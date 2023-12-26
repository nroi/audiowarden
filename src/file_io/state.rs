use crate::file_io::model::Versioned;
use crate::http::spotify::client::TokenResponse;
use crate::APPLICATION_NAME;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::{env, fs, io};
use ureq::serde_json;

pub fn store_spotify_token(token: &TokenResponse) -> io::Result<()> {
    let filename = match get_spotify_token_filename() {
        Ok(f) => f,
        Err(reason) => {
            panic!("Unable to store spotify token: {}", reason);
        }
    };
    let token = TokenResponseV1 {
        access_token: token.access_token.clone(),
        token_type: token.token_type.clone(),
        expires_in: token.expires_in,
        refresh_token: token.refresh_token.clone(),
    };
    let token_as_json = serde_json::to_string(&token)?;
    let file = match File::create(&filename) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            create_state_directory()?;
            File::create(&filename)?
        }
        Err(e) => return Err(e),
    };
    let mut writer = BufWriter::new(file);
    writer.write_all(token_as_json.as_bytes())?;

    Ok(())
}

pub fn get_spotify_token() -> io::Result<Option<TokenResponse>> {
    let filename = match get_spotify_token_filename() {
        Ok(f) => f,
        Err(reason) => {
            panic!("Unable to retrieve spotify token: {}", reason);
        }
    };
    let file = match File::open(filename) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // This is not an error: If audiowarden starts for the first time, for example, then
            // the file does not exist yet.
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    let reader = BufReader::new(file);
    let token = serde_json::from_reader(reader)?;

    Ok(token)
}

fn get_spotify_token_filename() -> Result<PathBuf, String> {
    let cache_directory = get_state_directory();
    Ok(cache_directory.join("spotify_token.json"))
}

fn create_state_directory() -> io::Result<()> {
    let directory = get_state_directory();
    fs::create_dir_all(directory)
}

fn get_state_directory() -> PathBuf {
    if let Ok(state_dir) = env::var("STATE_DIRECTORY") {
        // STATE_DIRECTORY is set if this application runs via systemd: More details here:
        // https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#RuntimeDirectory=
        Path::new(&state_dir).to_path_buf()
    } else if let Ok(xdg_state_home) = env::var("XDG_STATE_HOME") {
        Path::new(&xdg_state_home).join(APPLICATION_NAME)
    } else if let Ok(home) = env::var("HOME") {
        let state_dir = Path::new(&home)
            .join(".local")
            .join("state")
            .join(APPLICATION_NAME);
        state_dir
    } else {
        // We try to avoid panic! in general, but this is one of those cases where audiowarden
        // is just not usable in any reasonable way.
        panic!("None of the environment vars STATE_DIRECTORY, XDG_STATE_HOME or HOME is set.");
    }
}

#[derive(Serialize, Deserialize)]
struct TokenResponseV1 {
    access_token: String,
    token_type: String,
    expires_in: usize,
    refresh_token: String,
}

impl Versioned<TokenResponse> for TokenResponseV1 {}

impl From<TokenResponse> for TokenResponseV1 {
    fn from(value: TokenResponse) -> Self {
        Self {
            access_token: value.access_token,
            token_type: value.token_type,
            expires_in: value.expires_in,
            refresh_token: value.refresh_token,
        }
    }
}

impl From<TokenResponseV1> for TokenResponse {
    fn from(value: TokenResponseV1) -> Self {
        Self {
            access_token: value.access_token,
            token_type: value.token_type,
            expires_in: value.expires_in,
            refresh_token: value.refresh_token,
        }
    }
}
