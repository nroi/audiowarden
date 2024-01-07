use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dbus::arg::messageitem::{MessageItem, MessageItemDict};
use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus::strings::Member;
use dbus::{Message, MessageType};

use crate::cache;
use crate::http::spotify::client;
use crate::http::spotify::client::TokenOption;

pub fn setup_mpris_connection(token_option: Arc<Mutex<TokenOption>>) {
    let conn = Connection::new_session().expect("Unable to open D-Bus connection.");
    let proxy = conn.with_proxy(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        Duration::from_millis(5000),
    );

    let path = dbus::Path::new("/org/mpris/MediaPlayer2").expect("Invalid D-Bus path.");
    let member = Member::new("PropertiesChanged").expect("Invalid D-Bus member.");
    let rule = MatchRule::new()
        .with_path(path)
        .with_type(MessageType::Signal)
        .with_member(member);

    let result: Result<(), dbus::Error> = proxy.method_call(
        "org.freedesktop.DBus.Monitoring",
        "BecomeMonitor",
        (vec![rule.match_str()], 0u32),
    );
    result.expect("Unable to execute method against D-Bus.");

    conn.start_receive(
        rule,
        Box::new(move |msg, _| {
            handle_message(&msg, token_option.clone(), false);
            true
        }),
    );

    loop {
        conn.process(Duration::from_millis(1000))
            .expect("Unable to process D-Bus message.");
    }
}

pub fn play_next() {
    let conn =
        Connection::new_session().expect("Unable to open D-Bus connection to play next song.");
    let proxy = conn.with_proxy(
        "org.mpris.MediaPlayer2.spotify",
        "/org/mpris/MediaPlayer2",
        Duration::from_millis(5000),
    );

    let result: Result<(), dbus::Error> =
        proxy.method_call("org.mpris.MediaPlayer2.Player", "Next", ());
    if let Err(e) = result {
        error!(
            "Unable to execute method against D-Bus to play next song: {:?}",
            e
        );
    }
}

fn handle_message(message: &Message, token_option: Arc<Mutex<TokenOption>>, cache_updated: bool) {
    let blocked_songs = match cache::get_blocked_songs() {
        Ok(songs) => songs,
        Err(e) => {
            error!("Unable to determine blocked songs: {:?}", e);
            return;
        }
    };
    let song_attributes = song_attributes_from_message(message);
    if song_attributes.len() > 1 {
        error!(
            "Received more than one song via a single MPRIS message, this is unexpected. We \
            will only consider the first song. These are the songs we've \
            received: {:?}",
            song_attributes
        );
    }
    if let Some(song_attributes) = song_attributes.first() {
        let maybe_blocked_song = blocked_songs
            .iter()
            .find(|blocked_song| blocked_song.spotify_url == song_attributes.url);

        let suffix = match maybe_blocked_song {
            None => "[NOT BLOCKED]".to_string(),
            Some(blocked_song) => {
                play_next();
                format!("[BLOCKED] via playlist <{}>", blocked_song.playlist_name)
            }
        };

        info!("{} {}", song_attributes, suffix);

        // Update the cache, if it's not already up-to-date. Note that it might seem
        // counterintuitive to update the cache now (it would seem more reasonable to update the
        // cache before we decide if the song should be blocked or not). However, we're doing it
        // this way in order to avoid that any latency can be perceived by the user: If the
        // currently playing song should be blocked, this should happen immediately, and not just
        // after we have updated the cache.
        if !cache_updated {
            info!("The cache might be stale, so we update it.");
            let mut token_option_guard = token_option.lock().unwrap();
            let cache_update_successful = match token_option_guard.token_container.as_mut() {
                None => false,
                Some(token_container) => {
                    match client::update_blocked_songs_in_cache(token_container) {
                        Ok(()) => true,
                        Err(e) => {
                            error!("Unable to update blocked songs: {:?}", e);
                            false
                        }
                    }
                }
            };

            // If we had to update the cache, and we did not block the currently playing song,
            // then it's possible that we've made the wrong decision based on information in a
            // stale cache (i.e., if the song is not blocked in the stale cache, but is blocked
            // in the current cache). So, we re-run this function.
            if cache_update_successful && maybe_blocked_song.is_none() {
                handle_message(message, token_option.clone(), true)
            }
        }
    }
}

fn song_attributes_from_message(message: &Message) -> Vec<SongAttributes> {
    message
        .get_items()
        .iter()
        .flat_map(|message_item| match &message_item {
            MessageItem::Dict(d) => song_attributes_from_message_item(d),
            _ => None,
        })
        .collect()
}

fn vec_from_message_item(message_item: &MessageItem) -> Option<Vec<&str>> {
    let mut string_values = vec![];
    match message_item {
        MessageItem::Array(a) => {
            for v in a.iter() {
                match v.peel() {
                    MessageItem::Str(s) => string_values.push(s.as_str()),
                    _ => return None,
                }
            }
        }
        _ => return None,
    }

    Some(string_values)
}

fn song_attributes_from_message_item(dict: &MessageItemDict) -> Option<SongAttributes> {
    debug!("processing dict: {:?}", dict);
    let mut artist: Option<String> = None;
    let mut title: Option<String> = None;
    let mut url: Option<String> = None;

    let metadata_values = dict.iter().filter_map(|(key, value)| match key {
        MessageItem::Str(s) if s == "Metadata" => Some(value),
        _ => None,
    });

    for value in metadata_values {
        if let MessageItem::Variant(variant) = value {
            let variant = variant.peel();
            if let MessageItem::Dict(d) = variant {
                for (key, value) in d.iter() {
                    let value = value.peel();
                    match key {
                        MessageItem::Str(s) if s == "xesam:artist" => {
                            match vec_from_message_item(value) {
                                Some(a) => {
                                    artist = Some(a.join(", "));
                                }
                                None => {
                                    warn!("Unable to parse artists from {:?}", value);
                                }
                            }
                        }
                        MessageItem::Str(s) if s == "xesam:title" => {
                            match string_from_message_item(value) {
                                Some(t) => {
                                    title = Some(t.to_string());
                                }
                                None => {
                                    warn!("Unable to parse title from {:?}", value);
                                }
                            }
                        }
                        MessageItem::Str(s) if s == "xesam:url" => {
                            match string_from_message_item(value) {
                                Some(u) => {
                                    url = Some(u.to_string());
                                }
                                None => {
                                    warn!("Unable to parse URL from {:?}", value);
                                }
                            }
                        }
                        _ => {
                            // Nothing to do.
                        }
                    };
                }
            }
        };
    }

    match url {
        Some(url) if url.contains("open.spotify.com") => {
            Some(SongAttributes { url, artist, title })
        }
        _ => {
            // if no URL exists, or the URL does not contain the spotify host, then the event was
            // probably not emitted by Spotify and should be ignored.
            None
        }
    }
}

fn string_from_message_item(message_item: &MessageItem) -> Option<&str> {
    match message_item {
        MessageItem::Str(s) => Some(s),
        _ => None,
    }
}

#[derive(Debug)]
struct SongAttributes {
    url: String,
    artist: Option<String>,
    title: Option<String>,
}

impl Display for SongAttributes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let artist = match &self.artist {
            Some(a) => a.as_str(),
            None => "Unknown",
        };
        let title = match &self.title {
            Some(t) => t.as_str(),
            None => "Unknown",
        };
        write!(f, "Artist: {}, Title: {}, URL: {}", artist, title, self.url)
    }
}
