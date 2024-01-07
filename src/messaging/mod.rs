use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::http::spotify::client::TokenOption;
use crate::{http, APPLICATION_NAME};

mod socket;

pub fn setup_channel(token_option: Arc<Mutex<TokenOption>>) {
    let token_option = token_option.clone();
    std::thread::spawn(move || {
        let (tx, rx): (Sender<ClientMessage>, Receiver<ClientMessage>) = channel();
        std::thread::spawn(|| {
            if let Err(err) = socket::open_and_listen_unix_socket(tx) {
                error!("Unable to open unix socket: {:?}", err);
            }
        });
        process_incoming_messages(rx, token_option);
    });
}

fn process_incoming_messages(rx: Receiver<ClientMessage>, token_option: Arc<Mutex<TokenOption>>) {
    loop {
        match rx.recv() {
            Ok(msg) => match msg {
                ClientMessage::LoginToSpotify(back_channel) => {
                    match http::spotify::client::spotify_login_start(token_option.clone()) {
                        Ok(authorization_url) => {
                            let message = format!("{}\n", authorization_url);
                            if let Err(e) = back_channel.send(message) {
                                error!("Unable to send message via back_channel: {:?}", e);
                            }
                        }
                        Err(e) => {
                            let message = format!(
                                "Unable to start login process: {:?}. Maybe \
                                there's already a login process pending. You may try \
                                restarting {}.\n",
                                e, APPLICATION_NAME
                            );
                            if let Err(e) = back_channel.send(message) {
                                error!("Unable to send message via back_channel: {:?}", e);
                            }
                        }
                    }
                }
            },
            Err(e) => {
                error!("Error while receiving message on channel: {:?}", e);
                // Avoid spamming the logs in an infinite loop:
                break;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ClientMessage {
    /**
     * user requested to login to Spotify to give audiowarden the required authorizations
     * for fetching playlists.
     */
    LoginToSpotify(Sender<String>),
}
