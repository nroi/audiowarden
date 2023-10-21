use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, fs};
use url::Url;

use dbus::arg::messageitem::{MessageItem, MessageItemDict};
use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus::strings::Member;
use dbus::MessageType;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blocked_songs = get_blocked_songs();
    if let Ok(songs) = &blocked_songs {
        println!("{} songs are blocked.", songs.len());
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

fn get_blocked_songs() -> Result<HashSet<String>, Error> {
    let path = create_config_path_and_file();
    parse_config_file(&path)
}

fn create_config_path_and_file() -> PathBuf {
    match get_config_path() {
        Ok(config_path) => {
            let filepath = config_path.join("blocked_songs.conf");
            match fs::create_dir_all(&config_path) {
                Ok(_) => {
                    create_initial_config_file(&filepath);
                }
                Err(e) => {
                    if e.kind() == ErrorKind::AlreadyExists {
                        // config directory already exists: this is the expected case when
                        // the application is not running for the first time and the config dir
                        // was therefore already created previously.
                    } else {
                        panic!(
                            "Unable to create config directory at {:?}: {:?}",
                            &config_path, e
                        );
                    }
                }
            }
            filepath
        }
        Err(e) => {
            panic!("Unable to fetch config file: {}", e);
        }
    }
}

fn parse_config_file(path: &Path) -> Result<HashSet<String>, Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut valid_urls = HashSet::new();

    for (line_number, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        // The # char may be used for comments.
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Ok(mut url) = Url::parse(line) {
            // When we copy URLs from spotify (via "share" in the context menu), then the resulting
            // link usually has a query param attached to it, something like '?si=7764fc…'. But
            // the URLs we get via mpris/dbus do not contain this query param. Therefore, we need
            // to remove it so that songs are matched correctly.
            url.set_query(None);
            valid_urls.insert(url.to_string());
        } else {
            eprintln!(
                "Error in line {}: the following is not a valid URL: {}",
                line_number + 1,
                line
            );
        }
    }

    Ok(valid_urls)
}

fn get_config_path() -> Result<PathBuf, String> {
    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        Ok(Path::new(&xdg_config_home).join(APPLICATION_NAME).join(""))
    } else if let Ok(home) = env::var("HOME") {
        let config_path = Path::new(&home).join(".config").join(APPLICATION_NAME);
        Ok(config_path)
    } else {
        Err("Neither XDG_CONFIG_HOME nor HOME environment variables are set.".to_string())
    }
}

fn create_initial_config_file(path: &Path) {
    match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(mut file) => {
            // TODO maybe describe this in more detail in a markdown file. In particular,
            // also describe that we can copy a full playlist with Ctrl-C and then just paste
            // the URLs all-at-once with Ctrl-V.
            let explanation = b"# Enter all songs that you don't want to listen to anymore here.\
            \n# Make sure to enter valid spotify URLs only: You can get them from the Spotify app\
            \n# via the 'share' functionality. For example, if you use the desktop version of\
            \n# Spotify, right-click a song, click share, and then 'Copy Song Link'.\
            \n\n# The following line is included for testing and demonstration purposes: Feel free\
            \n# to remove this line (and everything else in this file) to replace it by your\
            \n# own song URLs.\
            \nhttps://open.spotify.com/track/6CE6xXEI29e6X0noaNugIW\n";
            if let Err(err) = file.write_all(explanation) {
                eprintln!("Error writing to file: {}", err);
            }
        }
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            println!("File {:?} already exists.", path);
            // File already exists, nothing to do.
        }
        Err(err) => {
            eprintln!("Error creating file at path {:?}: {}", path, err);
        }
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
                let blocked_songs = get_blocked_songs();
                if let Ok(songs) = &blocked_songs {
                    println!("{} songs are blocked.", songs.len());
                }
                let blocked = match &blocked_songs {
                    Ok(v) => v.contains(&attrs.url.to_string()),
                    Err(_) => false,
                };
                if blocked {
                    println!("Song is blocked: Will skip to next song.");
                    play_next()
                } else {
                    println!("Song is not blocked.");
                }
                println!("{}", attrs);
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
    println!("processing dict: {:?}", dict);
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
                                    println!("Unable to parse artists from {:?}", value);
                                }
                            }
                        }
                        MessageItem::Str(s) if s == "xesam:title" => {
                            match string_from_message_item(value) {
                                Some(t) => {
                                    title = Some(t.to_string());
                                }
                                None => {
                                    println!("Unable to parse title from {:?}", value);
                                }
                            }
                        }
                        MessageItem::Str(s) if s == "xesam:url" => {
                            match string_from_message_item(value) {
                                Some(u) => {
                                    url = Some(u.to_string());
                                }
                                None => {
                                    println!("Unable to parse URL from {:?}", value);
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
        None => {
            println!("Required attribute URL was not found");
            None
        }
        Some(url) => Some(SongAttributes { url, artist, title }),
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

const APPLICATION_NAME: &str = "audiowarden";
