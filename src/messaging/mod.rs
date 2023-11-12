use crate::config::add_to_config_file;
use crate::mpris;
use std::sync::mpsc::{channel, Receiver, Sender};

mod socket;

pub fn setup_channel() {
    std::thread::spawn(move || {
        let (tx, rx): (Sender<ClientMessage>, Receiver<ClientMessage>) = channel();
        std::thread::spawn(|| {
            if let Err(err) = socket::open_and_listen_unix_socket(tx) {
                error!("Unable to open unix socket: {:?}", err);
            }
        });
        process_incoming_messages(rx);
    });
}

fn process_incoming_messages(rx: Receiver<ClientMessage>) {
    loop {
        match rx.recv() {
            Ok(msg) => match msg {
                ClientMessage::BlockCurrentSong => {
                    match mpris::current_song() {
                        None => {
                            warn!("Unable to determine current song")
                        }
                        Some(song_attrs) => {
                            info!("Currently playing: {:?}", song_attrs);
                            let attributes = vec![
                                song_attrs
                                    .artist
                                    .map(|artist| format!("Artist: {}", artist)),
                                song_attrs.title.map(|title| format!("Title: {}", title)),
                            ];
                            let attributes: Vec<&str> = attributes
                                .iter()
                                .filter_map(|x| x.as_ref())
                                .map(|x| x.as_str())
                                .collect();
                            let comment = if attributes.is_empty() {
                                None
                            } else {
                                Some(format!("# {}", attributes.join(", ")))
                            };

                            let prefix = match comment {
                                Some(c) => format!("{}\n", c),
                                None => "".to_string(),
                            };

                            let config_entry = format!("\n{}{}\n", prefix, song_attrs.url);
                            if let Err(e) = add_to_config_file(&config_entry) {
                                warn!("Unable to add entry to config file: {:?}", e);
                            }
                        }
                    }
                    mpris::play_next();
                }
            },
            Err(e) => {
                error!("Error while receiving message on channel: {:?}", e);
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ClientMessage {
    BlockCurrentSong,
}
