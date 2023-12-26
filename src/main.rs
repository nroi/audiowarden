#[macro_use]
extern crate log;

use crate::file_io::state;
use crate::http::spotify::client;
use crate::http::spotify::client::{spotify_login_start, TokenContainer};
use file_io::cache;

use crate::mpris::setup_mpris_connection;

mod error;
mod file_io;
mod http;
mod messaging;
mod model;
mod mpris;

fn main() {
    env_logger::builder().format_timestamp_millis().init();

    messaging::setup_channel();

    match state::get_spotify_token() {
        Ok(Some(token)) => {
            let mut token_container = TokenContainer::new(token);
            if let Err(e) = client::update_blocked_songs_in_cache(&mut token_container) {
                error!("Unable to update blocked songs: {:?}", e);
            }
        }
        Ok(None) => {
            info!("No token exists yet – the user must login first.");
            match spotify_login_start() {
                Ok(url) => {
                    info!("Please visit the following URL in your browser: {}", url)
                }
                Err(e) => {
                    error!("Unable to start the login process: {:?}", e);
                }
            }
        }
        Err(e) => {
            error!("Unable to update blocked songs: {:?}", e);
        }
    }

    match cache::get_blocked_songs() {
        Ok(songs) => {
            debug!("{} songs are blocked.", songs.len());
        }
        Err(e) => {
            error!("Unable to get blocked songs: {:?}", e);
        }
    }

    setup_mpris_connection();
}

pub const APPLICATION_NAME: &str = "audiowarden";
