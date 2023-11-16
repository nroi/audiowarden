#[derive(Debug)]
pub struct BlockedSong {
    pub spotify_url: String,
    // The playlist where this song was found.
    pub playlist_name: String,
}
