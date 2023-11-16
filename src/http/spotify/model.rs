use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
pub struct SpotifyPlaylist {
    pub id: String,
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub href: String,
    pub snapshot_id: String,
    pub tracks: SpotifyPlaylistTracks,
}

pub type SpotifyPlaylistTracks = SpotifyPagingObject<SpotifyPlaylistTrackObject>;
pub type SpotifyPlaylistSimplified = SpotifyPagingObject<SpotifySimplifiedPlaylistObject>;

#[derive(Debug, Deserialize, PartialEq)]
pub struct SpotifyPagingObject<T> {
    pub limit: u32,
    pub next: Option<String>,
    pub offset: u32,
    pub total: u32,
    pub items: Vec<T>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct SpotifyPlaylistTrackObject {
    pub is_local: bool,
    pub track: SpotifyTrackOrEpisodeObject,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum SpotifyTrackOrEpisodeObject {
    #[serde(rename(deserialize = "episode"))]
    SpotifyEpisodeObject {
        is_local: bool,
        uri: Option<String>,
        external_urls: SpotifyExternalUrl,
    },
    #[serde(rename(deserialize = "track"))]
    SpotifyTrackObject {
        is_local: bool,
        uri: Option<String>,
        external_urls: SpotifyExternalUrl,
    },
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct SpotifyExternalUrl {
    pub spotify: Option<String>, // Can be null if song is local
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct SpotifySimplifiedPlaylistObject {
    pub name: String,
    pub description: Option<String>,
    pub href: String,
    pub tracks: SpotifySimplifiedPlaylistObjectTracks,
    pub id: String,
    pub uri: String,
    pub snapshot_id: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct SpotifySimplifiedPlaylistObjectTracks {
    pub href: String,
    pub total: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::spotify::model::SpotifyTrackOrEpisodeObject::{
        SpotifyEpisodeObject, SpotifyTrackObject,
    };
    use ureq::serde_json;

    #[test]
    fn deserialize_playlist() {
        let json_data = include_str!("test_artefacts/my_playlist.json");
        let playlist = serde_json::from_str::<SpotifyPlaylist>(json_data);
        let playlist = playlist.expect("Unable to deserialize from JSON");
        let expected = SpotifyPlaylist {
            id: "6i7r07KuYaY6X2C4wdHza7".to_string(),
            uri: "spotify:playlist:6i7r07KuYaY6X2C4wdHza7".to_string(),
            name: "my playlist".to_string(),
            description: Some("test playlist for audiowarden".to_string()),
            href: "https://api.spotify.com/v1/playlists/6i7r07KuYaY6X2C4wdHza7?\
                fields=id,uri,name,description,href,snapshot_id,tracks(next,offset,limit,total),\
                tracks.items(is_local,%20track(uri,external_urls,is_local,type))"
                .to_string(),
            snapshot_id: "NixmM2YzYTdmNmE4ODM4ZTdiZDQ2N2ZkYjg4NDVlOGI2ZGMyYjgyMmRj".to_string(),
            tracks: SpotifyPagingObject {
                limit: 100,
                next: None,
                offset: 0,
                total: 3,
                items: vec![
                    SpotifyPlaylistTrackObject {
                        is_local: false,
                        track: SpotifyTrackObject {
                            is_local: false,
                            uri: Some("spotify:track:1BncfTJAWxrsxyT9culBrj".to_string()),
                            external_urls: SpotifyExternalUrl {
                                spotify: Some(
                                    "https://open.spotify.com/track/1BncfTJAWxrsxyT9culBrj"
                                        .to_string(),
                                ),
                            },
                        },
                    },
                    SpotifyPlaylistTrackObject {
                        is_local: false,
                        track: SpotifyTrackObject {
                            is_local: false,
                            uri: Some("spotify:track:7xEX406hnVXC7mDfkts2jc".to_string()),
                            external_urls: SpotifyExternalUrl {
                                spotify: Some(
                                    "https://open.spotify.com/track/7xEX406hnVXC7mDfkts2jc"
                                        .to_string(),
                                ),
                            },
                        },
                    },
                    SpotifyPlaylistTrackObject {
                        is_local: false,
                        track: SpotifyTrackObject {
                            is_local: false,
                            uri: Some("spotify:track:56oReVXIfUO9xkX7pHmEU0".to_string()),
                            external_urls: SpotifyExternalUrl {
                                spotify: Some(
                                    "https://open.spotify.com/track/56oReVXIfUO9xkX7pHmEU0"
                                        .to_string(),
                                ),
                            },
                        },
                    },
                ],
            },
        };
        assert_eq!(playlist, expected);
    }

    #[test]
    fn deserialize_playlist_with_podcast() {
        let json_data = include_str!("test_artefacts/playlist_with_podcast.json");
        let playlist = serde_json::from_str::<SpotifyPlaylist>(json_data);
        let playlist = playlist.expect("Unable to deserialize from JSON");
        let expected = SpotifyPlaylist {
            id: "3jtq2m90g20x3JSdTjnDdZ".to_string(),
            uri: "spotify:playlist:3jtq2m90g20x3JSdTjnDdZ".to_string(),
            name: "playlist with podcast".to_string(),
            description: Some("test playlist for audiowarden".to_string()),
            href: "https://api.spotify.com/v1/playlists/3jtq2m90g20x3JSdTjnDdZ?fields=id,uri,name,\
                description,href,snapshot_id,tracks(next,offset,limit,total),\
                tracks.items(is_local,track(uri,external_urls,is_local,type))"
                .to_string(),
            snapshot_id: "NCxlMTA5MzhkMDA4MjU1MjNkNjdhNzg2MmM0N2I5OGQwMjU0NDQ2Mzc1".to_string(),
            tracks: SpotifyPagingObject {
                limit: 100,
                next: None,
                offset: 0,
                total: 1,
                items: vec![SpotifyPlaylistTrackObject {
                    is_local: false,
                    track: SpotifyEpisodeObject {
                        is_local: false,
                        uri: Some("spotify:episode:2hfRg2xGfokD333h69QQt8".to_string()),
                        external_urls: SpotifyExternalUrl {
                            spotify: Some(
                                "https://open.spotify.com/episode/2hfRg2xGfokD333h69QQt8"
                                    .to_string(),
                            ),
                        },
                    },
                }],
            },
        };
        assert_eq!(playlist, expected);
    }

    #[test]
    fn deserialize_playlist_with_local_track() {
        let json_data = include_str!("test_artefacts/playlist_with_local_track.json");
        let playlist = serde_json::from_str::<SpotifyPlaylist>(json_data);
        let playlist = playlist.expect("Unable to deserialize from JSON");
        let expected = SpotifyPlaylist {
            id: "2aj6oxgwTOFoynFcnU2U6T".to_string(),
            uri: "spotify:playlist:2aj6oxgwTOFoynFcnU2U6T".to_string(),
            name: "playlist with local track".to_string(),
            description: Some("test playlist for audiowarden".to_string()),
            href: "https://api.spotify.com/v1/playlists/2aj6oxgwTOFoynFcnU2U6T?fields=id,uri,\
                name,description,href,snapshot_id,tracks(next,offset,limit,total),\
                tracks.items(is_local,track(uri,external_urls,is_local,type))"
                .to_string(),
            snapshot_id: "NCxhODZjNGQzOGM1ZDNlMDBmNWEzNjRlMzE0ZjBhOTZlZmZkNmExMmQ3".to_string(),
            tracks: SpotifyPagingObject {
                limit: 100,
                next: None,
                offset: 0,
                total: 1,
                items: vec![SpotifyPlaylistTrackObject {
                    is_local: true,
                    track: SpotifyTrackObject {
                        is_local: true,
                        uri: Some(
                            "spotify:local:Geety:Into+The+Moonlight+EP:Geety+-+Envision:394"
                                .to_string(),
                        ),
                        external_urls: SpotifyExternalUrl { spotify: None },
                    },
                }],
            },
        };
        assert_eq!(playlist, expected);
    }

    #[test]
    fn deserialize_simplified_playlist() {
        let json_data = include_str!("test_artefacts/playlists_simplified.json");
        let playlist = serde_json::from_str::<SpotifyPlaylistSimplified>(json_data);
        let playlist = playlist.expect("Unable to deserialize from JSON");
        let expected = SpotifyPagingObject {
            limit: 3,
            next: Some(
                "https://api.spotify.com/v1/users/john_doe/playlists?offset=3&limit=3".to_string(),
            ),
            offset: 0,
            total: 81,
            items: vec![
                SpotifySimplifiedPlaylistObject {
                    name: "playlist with local track".to_string(),
                    description: Some("test playlist for audiowarden".to_string()),
                    href: "https://api.spotify.com/v1/playlists/2aj6oxgwTOFoynFcnU2U6T".to_string(),
                    tracks: SpotifySimplifiedPlaylistObjectTracks {
                        href: "https://api.spotify.com/v1/playlists/2aj6oxgwTOFoynFcnU2U6T/tracks"
                            .to_string(),
                        total: 1,
                    },
                    id: "2aj6oxgwTOFoynFcnU2U6T".to_string(),
                    uri: "spotify:playlist:2aj6oxgwTOFoynFcnU2U6T".to_string(),
                    snapshot_id: "NCxhODZjNGQzOGM1ZDNlMDBmNWEzNjRlMzE0ZjBhOTZlZmZkNmExMmQ3"
                        .to_string(),
                },
                SpotifySimplifiedPlaylistObject {
                    name: "playlist with podcast".to_string(),
                    description: Some("test playlist for audiowarden".to_string()),
                    href: "https://api.spotify.com/v1/playlists/3jtq2m90g20x3JSdTjnDdZ".to_string(),
                    tracks: SpotifySimplifiedPlaylistObjectTracks {
                        href: "https://api.spotify.com/v1/playlists/3jtq2m90g20x3JSdTjnDdZ/tracks"
                            .to_string(),
                        total: 1,
                    },
                    id: "3jtq2m90g20x3JSdTjnDdZ".to_string(),
                    uri: "spotify:playlist:3jtq2m90g20x3JSdTjnDdZ".to_string(),
                    snapshot_id: "NCxlMTA5MzhkMDA4MjU1MjNkNjdhNzg2MmM0N2I5OGQwMjU0NDQ2Mzc1"
                        .to_string(),
                },
                SpotifySimplifiedPlaylistObject {
                    name: "Dislike".to_string(),
                    description: Some(
                        "songs I don&#x27;t like to hear. audiowarden:block_songs.".to_string(),
                    ),
                    href: "https://api.spotify.com/v1/playlists/54MXIlypOpyez6JSGNhgVH".to_string(),
                    tracks: SpotifySimplifiedPlaylistObjectTracks {
                        href: "https://api.spotify.com/v1/playlists/54MXIlypOpyez6JSGNhgVH/tracks"
                            .to_string(),
                        total: 2,
                    },
                    id: "54MXIlypOpyez6JSGNhgVH".to_string(),
                    uri: "spotify:playlist:54MXIlypOpyez6JSGNhgVH".to_string(),
                    snapshot_id: "NCwwNzE4MTA2MjZmYWE2MDY5MmNlYTQ4ZTAxN2RmZWQ0YzVlYWZmZTli"
                        .to_string(),
                },
            ],
        };
        assert_eq!(playlist, expected);
    }
}
