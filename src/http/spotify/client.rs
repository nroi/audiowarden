use std::io;
use std::time::Duration;

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
    exponential_backoff: ExponentialBackoff,
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
                                request_with_auth(
                                    original_request,
                                    token_container,
                                    true,
                                    exponential_backoff,
                                )
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
                ureq::Error::Status(429, _) => {
                    match exponential_backoff.increase_after_limit_exceeded() {
                        Some((duration, new_backoff)) => {
                            std::thread::sleep(duration);
                            request_with_auth(
                                original_request,
                                token_container,
                                retry_after_auth_failure,
                                new_backoff,
                            )
                        }
                        None => {
                            error!("Max. number of retries reached after rate limit exceeded.");
                            Err(ClientConnectionHandlingError::UreqError(e))
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
        request_with_auth::<SpotifyPlaylistSimplified>(
            request,
            token_container,
            false,
            ExponentialBackoff::default(),
        )
    };

    let playlist_objects = fetch_all_pages(token_container, url, single_page_request)?;

    let playlists: Vec<SpotifySimplifiedPlaylistObject> = playlist_objects
        .into_iter()
        .flat_map(|page| page.items)
        .collect();

    let relevant_playlists: Vec<SpotifySimplifiedPlaylistObject> = playlists
        .into_iter()
        .filter(|playlist| {
            playlist
                .description
                .as_ref()
                .map(|description| description.contains(AUDIOWARDEN_BLOCK_SONGS_KEYWORD))
                .unwrap_or(false)
        })
        .collect();

    info!(
        "Retrieved {} playlist from /v1/me/playlists",
        relevant_playlists.len()
    );
    Ok(relevant_playlists)
}

pub fn fetch_track_urls(
    token_container: &mut TokenContainer,
    tracks: &SpotifyPlaylistTracks,
) -> ClientConnectionResult<Vec<String>> {
    let mut song_ids = extract_track_urls(tracks);
    if let Some(next) = &tracks.next {
        let additional_tracks = parse_playlist_tracks(token_container, next)?;
        song_ids.extend(additional_tracks)
    }

    Ok(song_ids)
}

fn extract_track_urls(playlist_tracks: &SpotifyPlaylistTracks) -> Vec<String> {
    playlist_tracks
        .items
        .iter()
        .filter_map(|track| match &track.track {
            SpotifyTrackOrEpisodeObject::SpotifyEpisodeObject { .. } => {
                // podcast episodes are ignored, we support only music tracks.
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
        request_with_auth::<SpotifyPlaylistTracks>(
            request,
            token_container,
            false,
            ExponentialBackoff::default(),
        )
    };
    let pages = fetch_all_pages(token_container, url, single_page_request)?;
    let tracks: Vec<String> = pages
        .into_iter()
        .flat_map(|tracks_from_page| extract_track_urls(&tracks_from_page))
        .collect();

    Ok(tracks)
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
    let playlist_tracks = match fetch_track_urls(token_container, &playlist.tracks) {
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

fn playlist_from_id(
    token_container: &mut TokenContainer,
    playlist_id: &str,
) -> ClientConnectionResult<SpotifyPlaylist> {
    let url = format!("https://api.spotify.com/v1/playlists/{}", playlist_id);
    // Filter which fields we actually require, to keep the payload small, for simplicity and
    // performance.
    let fields = "id,uri,name,description,href,snapshot_id,tracks(next,offset,limit,total),\
        tracks.items(is_local,track(uri,external_urls,is_local,type))";
    let query_params = vec![("fields", fields)];
    let spotify_playlist = token_container
        .set_auth_header(ureq::get(&url))
        .query_pairs(query_params)
        .set("Content-Type", "application/json")
        .call()?
        .into_json::<SpotifyPlaylist>()?;

    Ok(spotify_playlist)
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

#[derive(Debug, PartialEq)]
struct ExponentialBackoff {
    max_retries: u32,
    previous_retries: u32,
    backoff_duration: Duration,
}

impl ExponentialBackoff {
    /// Returns the duration to wait for after the rate limit was exceeded, and the updated
    /// ExponentialBackoff to be used if the rate limit is exceeded in subsequent requests.
    fn increase_after_limit_exceeded(&self) -> Option<(Duration, Self)> {
        if self.max_retries > self.previous_retries {
            let backoff = Self {
                max_retries: self.max_retries,
                previous_retries: self.previous_retries + 1,
                backoff_duration: self.backoff_duration * 2,
            };
            Some((self.backoff_duration, backoff))
        } else {
            None
        }
    }

    fn new(initial_backoff_duration: Duration, max_retries: u32) -> Self {
        Self {
            max_retries,
            backoff_duration: initial_backoff_duration,
            previous_retries: 0,
        }
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self::new(Duration::from_secs(1), 4)
    }
}

type ClientConnectionResult<T> = Result<T, ClientConnectionHandlingError>;

const SPOTIFY_PLAYLISTS_MAX_PER_PAGE: &str = "50";
const AUDIOWARDEN_BLOCK_SONGS_KEYWORD: &str = "audiowarden:block_songs";
const CLIENT_ID: &str = "a9cc0c11a3944da8a4f97ecfc92a972d";
const REDIRECT_URI: &str = "http://localhost:7185";
const SCOPE: &str = "playlist-read-private";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_backoff_test() {
        let initial_backoff = ExponentialBackoff::new(Duration::from_millis(200), 1);
        let (duration_to_wait, new_backoff) =
            initial_backoff.increase_after_limit_exceeded().unwrap();
        let expected_new_backoff = ExponentialBackoff {
            max_retries: 1,
            previous_retries: 1,
            backoff_duration: Duration::from_millis(400),
        };

        assert_eq!(duration_to_wait, Duration::from_millis(200));
        assert_eq!(new_backoff, expected_new_backoff);

        let next_backoff = new_backoff.increase_after_limit_exceeded();

        assert_eq!(next_backoff, None);
    }
}
