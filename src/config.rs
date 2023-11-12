use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use crate::APPLICATION_NAME;
use url::Url;

pub fn get_blocked_songs() -> Result<HashSet<String>, Error> {
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
            panic!("Unable to fetch config directory: {}", e);
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
            error!(
                "Error in line {}: the following is not a valid URL: {}",
                line_number + 1,
                line
            );
        }
    }

    Ok(valid_urls)
}

pub fn get_config_path() -> Result<PathBuf, String> {
    if let Ok(config_dir) = env::var("CONFIGURATION_DIRECTORY") {
        // CONFIGURATION_DIRECTORY is set if this application runs via systemd: More details here:
        // https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#RuntimeDirectory=
        Ok(Path::new(&config_dir).to_path_buf())
    } else if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        Ok(Path::new(&xdg_config_home).join(APPLICATION_NAME))
    } else if let Ok(home) = env::var("HOME") {
        let config_path = Path::new(&home).join(".config").join(APPLICATION_NAME);
        Ok(config_path)
    } else {
        Err(
            "None of the environment vars CONFIGURATION_DIRECTORY, XDG_CONFIG_HOME or HOME is set."
                .to_string(),
        )
    }
}

fn create_initial_config_file(path: &Path) {
    match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(mut file) => {
            let explanation = b"# Enter all songs that you don't want to listen to anymore here.\
            \n# Make sure to enter valid spotify URLs only: You can get them from the Spotify app\
            \n# via the 'share' functionality. For example, if you use the desktop version of\
            \n# Spotify, right-click a song, click share, and then 'Copy Song Link'.\
            \n# You can also select multiple songs and copy them with Ctrl + c to have multiple\
            \n# URLs in your clipboard.\
            \n\n# The following line is included for testing and demonstration purposes: Feel free\
            \n# to remove this line (and everything else in this file) to replace it by your\
            \n# own song URLs.\
            \nhttps://open.spotify.com/track/6CE6xXEI29e6X0noaNugIW\n";
            if let Err(err) = file.write_all(explanation) {
                error!("Error writing to file: {}", err);
            }
        }
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            debug!("File {:?} already exists.", path);
            // File already exists, nothing to do.
        }
        Err(err) => {
            warn!("Error creating file at path {:?}: {}", path, err);
        }
    }
}

pub fn add_to_config_file(content: &str) -> io::Result<()> {
    let path = create_config_path_and_file();
    let file = OpenOptions::new().append(true).open(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(content.as_bytes())?;
    Ok(())
}
