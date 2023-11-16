use std::io;

use base64::engine::general_purpose;
use base64::Engine;
use rand::distributions::{Alphanumeric, DistString};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::AudioWardenError;
use crate::file_io::{cache, state};
use crate::http::server;
use crate::http::spotify::model::{
    SpotifyPagingObject, SpotifyPlaylist, SpotifyPlaylistSimplified, SpotifyPlaylistTracks,
    SpotifySimplifiedPlaylistObject, SpotifyTrackOrEpisodeObject,
};
use crate::model::BlockedSong;

/// Returns the URL to be visited by the user
pub fn spotify_login_start() -> io::Result<Url> {
    let code_verifier = generate_random_string(128);
    let code_challenge = sha256_base64_encoded(&code_verifier);
    let state = generate_random_string(16);
    let url = Url::parse_with_params(
        "https://accounts.spotify.com/authorize",
        &[
            ("response_type", "code"),
            ("client_id", CLIENT_ID),
            ("scope", SCOPE),
            ("state", &state),
            ("code_challenge_method", "S256"),
            ("code_challenge", &code_challenge),
            ("redirect_uri", REDIRECT_URI),
        ],
    )
    .unwrap();
    server::listen(&code_verifier, &state, &url)?;

    Ok(url)
}

pub fn get_token(code: &str, code_verifier: &str) -> Result<TokenResponse, ureq::Error> {
    let url = "https://accounts.spotify.com/api/token";
    let query_params = vec![
        ("client_id", CLIENT_ID),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", REDIRECT_URI),
        ("code_verifier", code_verifier),
    ];
    let token: TokenResponse = ureq::post(url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Content-Length", "0")
        .query_pairs(query_params)
        .call()?
        .into_json()?;

    Ok(token)
}

fn generate_random_string(length: usize) -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), length)
}

fn sha256_base64_encoded(plain: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plain.as_bytes());
    let result = hasher.finalize();
    let encoded: String = general_purpose::STANDARD_NO_PAD.encode(result);

    // Not sure why we need the replacements, but this is how it's done in the official example
    // from Spotify:
    encoded.replace('=', "").replace('+', "-").replace('/', "_")
}

fn request_with_auth<T>(
    request: ureq::Request,
    token_container: &mut TokenContainer,
    retry_after_auth_failure: bool,
) -> ClientConnectionResult<T>
where
    T: DeserializeOwned,
{
    let original_request = request.clone();
    let result = token_container
        .set_auth_header(request)
        .clone()
        .set("Content-Type", "application/json")
        .call();
    match result {
        Ok(response) => Ok(response.into_json::<T>()?),
        Err(e) => {
            match e {
                ureq::Error::Status(401, _) => {
                    if retry_after_auth_failure {
                        // If we already tried to refresh our token, no need to try again.
                        Err(ClientConnectionHandlingError::UreqError(e))
                    } else {
                        // Otherwise, the 401 may be because our token has expired, so we try a
                        // refresh and then try again.
                        info!("Spotify returned 401, token refresh may be required.");
                        match token_container.refresh() {
                            Ok(()) => {
                                info!("Token refreshed successfully.");
                                request_with_auth(original_request, token_container, true)
                            }
                            Err(e) => {
                                if let ClientConnectionHandlingError::RefreshSpotifyTokenFailed = e
                                {
                                    error!(
                                        "Unable to refresh spotify token. The user \
                                        must login again."
                                    );
                                }
                                Err(e)
                            }
                        }
                    }
                }
                _ => {
                    error!("Request error: {:?}", e);
                    Err(ClientConnectionHandlingError::UreqError(e))
                }
            }
        }
    }
}

fn fetch_all_pages<T, F>(
    token_container: &mut TokenContainer,
    initial_url: &str,
    mut get_page: F,
) -> ClientConnectionResult<Vec<SpotifyPagingObject<T>>>
where
    F: FnMut(&str, &mut TokenContainer) -> ClientConnectionResult<SpotifyPagingObject<T>>,
{
    let mut current_url: Option<String> = Some(initial_url.to_string());
    let mut pages: Vec<SpotifyPagingObject<T>> = vec![];
    while let Some(ref url) = current_url {
        let page = get_page(url, token_container)?;
        current_url = page.next.clone();
        pages.push(page);
    }

    Ok(pages)
}

pub fn get_relevant_playlists(
    token_container: &mut TokenContainer,
) -> ClientConnectionResult<Vec<SpotifySimplifiedPlaylistObject>> {
    let url = "https://api.spotify.com/v1/me/playlists";
    let query_params = vec![("limit", SPOTIFY_PLAYLISTS_MAX_PER_PAGE)];

    let single_page_request = |url: &str, token_container: &mut TokenContainer| {
        let request = ureq::get(url).query_pairs(query_params.clone());
        request_with_auth::<SpotifyPlaylistSimplified>(request, token_container, false)
    };

    let playlist_objects = fetch_all_pages(token_container, url, single_page_request)?;

    let playlists: Vec<SpotifySimplifiedPlaylistObject> = playlist_objects
        .into_iter()
        .flat_map(|page| page.items)
        .collect();

    let relevant_playlists: Vec<SpotifySimplifiedPlaylistObject> = playlists
        .into_iter()
        .filter(|p| {
            p.description
                .clone()
                .map(|d| d.contains(AUDIOWARDEN_BLOCK_SONGS_KEYWORD))
                .unwrap_or(false)
        })
        .collect();

    info!(
        "got the following playlist from /me: {:#?}",
        relevant_playlists
    );
    Ok(relevant_playlists)
}

pub fn playlist_tracks(
    token_container: &mut TokenContainer,
    playlist: &SpotifyPlaylist,
) -> ClientConnectionResult<Vec<String>> {
    let mut tracks = song_ids_from_playlist(&playlist.tracks);
    if let Some(next) = &playlist.tracks.next {
        let additional_tracks = parse_playlist_tracks(token_container, next)?;
        tracks.extend(additional_tracks)
    }

    Ok(tracks)
}

fn song_ids_from_playlist(playlist_tracks: &SpotifyPlaylistTracks) -> Vec<String> {
    playlist_tracks
        .items
        .iter()
        .filter_map(|track| match &track.track {
            SpotifyTrackOrEpisodeObject::SpotifyEpisodeObject { .. } => {
                // podcast episodes are ignored, we support only normal music tracks.
                None
            }
            SpotifyTrackOrEpisodeObject::SpotifyTrackObject {
                is_local,
                external_urls,
                ..
            } => {
                if *is_local {
                    // local tracks are not supported for now, because the Spotify Web API does not
                    // provide any URLs inside the external_urls property.
                    None
                } else {
                    external_urls.spotify.clone()
                }
            }
        })
        .collect()
}

fn parse_playlist_tracks(
    token_container: &mut TokenContainer,
    url: &str,
) -> ClientConnectionResult<Vec<String>> {
    let single_page_request = |url: &str, token_container: &mut TokenContainer| {
        let request = ureq::get(url);
        request_with_auth::<SpotifyPlaylistTracks>(request, token_container, false)
    };
    let pages = fetch_all_pages(token_container, url, single_page_request)?;
    let tracks: Vec<String> = pages
        .into_iter()
        .flat_map(|tracks_from_page| song_ids_from_playlist(&tracks_from_page))
        .collect();

    Ok(tracks)
}

pub fn playlist_from_id(
    token_container: &mut TokenContainer,
    playlist_id: &str,
) -> ClientConnectionResult<SpotifyPlaylist> {
    let url = format!("https://api.spotify.com/v1/playlists/{}", playlist_id);
    // Filter which fields we actually require, to keep the payload small, for simplicity and
    // performance.
    let fields = "id,uri,name,description,href,snapshot_id,tracks(next,offset,limit,total),tracks.items(is_local,track(uri,external_urls,is_local,type))";
    let query_params = vec![("fields", fields)];

    let spotify_playlist = token_container
        .set_auth_header(ureq::get(&url))
        .query_pairs(query_params)
        .set("Content-Type", "application/json")
        .call()?
        .into_json::<SpotifyPlaylist>()?;

    Ok(spotify_playlist)
}

pub fn update_blocked_songs_in_cache(
    token_container: &mut TokenContainer,
) -> Result<(), AudioWardenError> {
    let blocked_songs = get_blocked_songs(token_container)?;
    info!("blocked songs: {:#?}", blocked_songs);
    Ok(cache::store_blocked_songs(blocked_songs)?)
}

fn get_blocked_songs(
    token_container: &mut TokenContainer,
) -> ClientConnectionResult<Vec<BlockedSong>> {
    let relevant_playlists = get_relevant_playlists(token_container)?;
    let blocked_songs: Vec<BlockedSong> = relevant_playlists
        .iter()
        .flat_map(|playlist| blocked_songs_from_playlist(playlist, token_container))
        .collect();

    Ok(blocked_songs)
}

fn blocked_songs_from_playlist(
    playlist: &SpotifySimplifiedPlaylistObject,
    token_container: &mut TokenContainer,
) -> Vec<BlockedSong> {
    let playlist = match playlist_from_id(token_container, &playlist.id) {
        Ok(p) => p,
        Err(e) => {
            error!(
                "Cannot determine playlist id for {}: {:?}",
                playlist.name, e
            );
            return vec![];
        }
    };
    let playlist_tracks = match playlist_tracks(token_container, &playlist) {
        Ok(tracks) => tracks,
        Err(e) => {
            error!(
                "Cannot determine playlist tracks for {}: {:?}",
                playlist.name, e
            );
            return vec![];
        }
    };
    playlist_tracks
        .iter()
        .map(|track| BlockedSong {
            spotify_url: track.clone(),
            playlist_name: playlist.name.clone(),
        })
        .collect::<Vec<BlockedSong>>()
}

#[derive(Debug)]
pub enum ClientConnectionHandlingError {
    IoError(io::Error),
    UreqError(ureq::Error),
    HttpProtocolError(String),
    RefreshSpotifyTokenFailed,
}

impl From<io::Error> for ClientConnectionHandlingError {
    fn from(error: io::Error) -> Self {
        ClientConnectionHandlingError::IoError(error)
    }
}

impl From<ureq::Error> for ClientConnectionHandlingError {
    fn from(error: ureq::Error) -> Self {
        ClientConnectionHandlingError::UreqError(error)
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: usize,
    pub refresh_token: String,
}

// TokenContainer contains the most recent version of the token, i.e., after each refresh, the
// token inside is updated. In general, the following rules should be followed when using tokens:
// 1. Use TokenContainer instead of TokenResponse whenever possible. TokenResponse should only be
//    used when deserialized from a file or when a new token was created via the Spotify API.
// 2. Only one instance of TokenContainer should exist. After each token refresh, this instance
//    should be updated with the new token.
// We use many &mut TokenContainer references throughout the code. Therefore, following these rules
// ensures that, whenever the token is refreshed, each ref is now using the most up-to-date token.
pub struct TokenContainer {
    token: TokenResponse,
}

impl TokenContainer {
    pub fn new(token_response: TokenResponse) -> Self {
        Self {
            token: token_response,
        }
    }

    fn set_auth_header(&self, request: ureq::Request) -> ureq::Request {
        let auth_header_value = format!("Bearer {}", self.token.access_token);
        request.set("Authorization", &auth_header_value)
    }

    fn refresh(&mut self) -> ClientConnectionResult<()> {
        let url = "https://accounts.spotify.com/api/token";
        let query_params = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", &self.token.refresh_token),
            ("client_id", CLIENT_ID),
        ];
        let request = ureq::post(url)
            .query_pairs(query_params)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .set("Content-Length", "0")
            .call();
        let token_response: TokenResponse = match request {
            Ok(r) => r.into_json()?,
            Err(_) => {
                return Err(ClientConnectionHandlingError::RefreshSpotifyTokenFailed);
            }
        };
        if let Err(e) = state::store_spotify_token(&token_response) {
            error!("Unable to store token after refresh: {:?}", e);
        }
        self.token = token_response;

        Ok(())
    }
}

type ClientConnectionResult<T> = Result<T, ClientConnectionHandlingError>;

const SPOTIFY_PLAYLISTS_MAX_PER_PAGE: &str = "50";
const AUDIOWARDEN_BLOCK_SONGS_KEYWORD: &str = "audiowarden:block_songs";
const CLIENT_ID: &str = "a9cc0c11a3944da8a4f97ecfc92a972d";
const REDIRECT_URI: &str = "http://localhost:7185";
const SCOPE: &str = "playlist-read-private";
