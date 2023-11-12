#[macro_use]
extern crate log;

use crate::mpris::setup_mpris_connection;

mod config;
mod error;
mod messaging;
mod mpris;

fn main() {
    env_logger::init();

    messaging::setup_channel();

    match config::get_config_path() {
        Ok(path) => {
            // We're not doing anything with the directory, but it's still useful to display
            // the directory upon start for first-time users, so they know which file they
            // need to edit.
            info!("Configuration directory: {}", &path.display())
        }
        Err(e) => {
            panic!("Unable to fetch config directory: {}", e);
        }
    }
    let blocked_songs = config::get_blocked_songs();
    if let Ok(songs) = &blocked_songs {
        debug!("{} songs are blocked.", songs.len());
    }

    setup_mpris_connection();
}

pub const APPLICATION_NAME: &str = "audiowarden";
