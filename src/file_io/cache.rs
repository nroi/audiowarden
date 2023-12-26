use std::fs::File;
use std::io::{BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use flate2::bufread::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use ureq::serde_json;

use crate::file_io::model::Versioned;
use crate::model::BlockedSong;
use crate::APPLICATION_NAME;

pub fn store_blocked_songs(blocked_songs: Vec<BlockedSong>) -> io::Result<()> {
    let filename = get_cache_filename();
    let blocked_songs_v1: Vec<BlockedSongV1> =
        blocked_songs.into_iter().map(BlockedSongV1::from).collect();
    let cache = AudiowardenCacheV1 {
        version: 1,
        blocked_songs: blocked_songs_v1,
    };
    let cache_as_json = serde_json::to_string(&cache)?;
    let file = match File::create(&filename) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            create_cache_directory()?;
            File::create(&filename)?
        }
        Err(e) => return Err(e),
    };
    let writer = BufWriter::new(file);
    let mut encoder = GzEncoder::new(writer, Compression::default());
    encoder.write_all(cache_as_json.as_bytes())?;

    Ok(())
}

pub fn get_blocked_songs() -> io::Result<Vec<BlockedSong>> {
    let filename = get_cache_filename();
    let file = match File::open(filename) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // This is not an error: If audiowarden starts for the first time, for example, then
            // the file does not exist yet.
            return Ok(vec![]);
        }
        Err(e) => return Err(e),
    };
    let reader = BufReader::new(file);
    let decoder = GzDecoder::new(reader);
    let cache: AudiowardenCacheV1 = serde_json::from_reader(decoder)?;
    let blocked_songs: Vec<BlockedSong> =
        cache.blocked_songs.into_iter().map(|b| b.into()).collect();

    Ok(blocked_songs)
}

fn get_cache_filename() -> PathBuf {
    get_cache_directory().join("blocked_songs.json.gz")
}

fn get_cache_directory() -> PathBuf {
    if let Ok(cache_dir) = env::var("CACHE_DIRECTORY") {
        // CACHE_DIRECTORY is set if this application runs via systemd: More details here:
        // https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#RuntimeDirectory=
        Path::new(&cache_dir).to_path_buf()
    } else if let Ok(xdg_cache_home) = env::var("XDG_CACHE_HOME") {
        Path::new(&xdg_cache_home).join(APPLICATION_NAME)
    } else if let Ok(home) = env::var("HOME") {
        Path::new(&home).join(".cache").join(APPLICATION_NAME)
    } else {
        // We try to avoid panic! in general, but this is one of those cases where audiowarden
        // is just not usable in any reasonable way.
        panic!("None of the environment vars CACHE_DIRECTORY, XDG_CACHE_HOME or HOME is set.");
    }
}

fn create_cache_directory() -> io::Result<()> {
    let directory = get_cache_directory();
    fs::create_dir_all(directory)
}

#[derive(Serialize, Deserialize)]
struct AudiowardenCacheV1 {
    version: u32,
    blocked_songs: Vec<BlockedSongV1>,
}

#[derive(Serialize, Deserialize)]
struct BlockedSongV1 {
    pub spotify_url: String,
    // The playlist where this song was found.
    pub playlist_name: String,
}

impl Versioned<BlockedSong> for BlockedSongV1 {}

impl From<BlockedSong> for BlockedSongV1 {
    fn from(value: BlockedSong) -> Self {
        Self {
            spotify_url: value.spotify_url,
            playlist_name: value.playlist_name,
        }
    }
}

impl From<BlockedSongV1> for BlockedSong {
    fn from(value: BlockedSongV1) -> Self {
        Self {
            spotify_url: value.spotify_url,
            playlist_name: value.playlist_name,
        }
    }
}
