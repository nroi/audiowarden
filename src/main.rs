use std::fmt::{Display, Formatter};
use std::time::Duration;

use dbus::arg::messageitem::{MessageItem, MessageItemDict};
use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus::strings::Member;
use dbus::MessageType;

#[macro_use]
extern crate log;

mod config;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let blocked_songs = config::get_blocked_songs();
    if let Ok(songs) = &blocked_songs {
        debug!("{} songs are blocked.", songs.len());
    }

    let conn = Connection::new_session()?;
    let proxy = conn.with_proxy(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        Duration::from_millis(5000),
    );

    let rule = MatchRule::new()
        .with_path(dbus::Path::new("/org/mpris/MediaPlayer2").unwrap())
        .with_type(MessageType::Signal)
        .with_member(Member::new("PropertiesChanged").unwrap());

    let result: Result<(), dbus::Error> = proxy.method_call(
        "org.freedesktop.DBus.Monitoring",
        "BecomeMonitor",
        (vec![rule.match_str()], 0u32),
    );
    result.unwrap();

    conn.start_receive(
        rule,
        Box::new(|msg, _| {
            handle_message(&msg);
            true
        }),
    );

    // Loop and print out all messages received (using handle_message()) as they come.
    // Some can be quite large, e.g. if they contain embedded images..
    loop {
        conn.process(Duration::from_millis(1000)).unwrap();
    }
}

fn play_next() {
    // TODO it would be nice if we could just re-use an existing connection here instead of
    //   creating a new one, but Rust's ownership semantics makes this a bit difficult.
    let conn = Connection::new_session().unwrap();
    let proxy = conn.with_proxy(
        "org.mpris.MediaPlayer2.spotify",
        "/org/mpris/MediaPlayer2",
        Duration::from_millis(5000),
    );

    let result: Result<(), dbus::Error> =
        proxy.method_call("org.mpris.MediaPlayer2.Player", "Next", ());
    result.unwrap();
}

fn handle_message(message: &dbus::Message) {
    for message_item in message.get_items() {
        if let MessageItem::Dict(d) = &message_item {
            if let Some(attrs) = get_attrs(d) {
                let blocked_songs = config::get_blocked_songs();
                if let Ok(songs) = &blocked_songs {
                    debug!("{} songs are blocked.", songs.len());
                }
                let blocked = match &blocked_songs {
                    Ok(v) => v.contains(&attrs.url.to_string()),
                    Err(_) => false,
                };
                let suffix = if blocked {
                    play_next();
                    "[BLOCKED]"
                } else {
                    "[NOT BLOCKED]"
                };
                info!("{} {}", attrs, suffix);
            }
        }
    }
}

fn string_from_message_item(message_item: &MessageItem) -> Option<&str> {
    match message_item {
        MessageItem::Str(s) => Some(s),
        _ => None,
    }
}

fn vec_from_message_item(message_item: &MessageItem) -> Option<Vec<&str>> {
    let mut values = vec![];
    match message_item {
        MessageItem::Array(a) => {
            for v in a.iter() {
                match v.peel() {
                    MessageItem::Str(s) => values.push(s.as_str()),
                    _ => return None,
                }
            }
        }
        _ => return None,
    }

    Some(values)
}

fn get_attrs(dict: &MessageItemDict) -> Option<SongAttributes> {
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
            // if no URL exists, or the URL does not contain the spotify host, then the event was probably not emitted
            // by spotify and should be ignored.
            None
        }
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
