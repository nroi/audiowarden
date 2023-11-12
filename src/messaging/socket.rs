use std::io::ErrorKind::NotFound;
use std::io::{ErrorKind, Read};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::{env, fs, io, thread};

use crate::error::AudioWardenError;
use crate::messaging::ClientMessage;
use crate::APPLICATION_NAME;

pub fn open_and_listen_unix_socket(tx: Sender<ClientMessage>) -> Result<(), AudioWardenError> {
    let path = get_and_create_socket_path()?;
    let path = path.join("audiowarden.sock");
    // If the socket file already exists, just remove it. If we open the existing file, we get
    // the error message "Address already in use".
    remove_socketfile(&path)?;
    let listener = UnixListener::bind(&path)?;

    let tx = Arc::new(tx);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tx = tx.clone();
                thread::spawn(move || {
                    handle_client(stream, tx);
                });
            }
            Err(err) => {
                error!("Error accepting connection on unix socket: {}", err);
            }
        }
    }

    Ok(())
}

fn get_and_create_socket_path() -> Result<PathBuf, AudioWardenError> {
    let path = get_socket_path()?;
    let result = match fs::create_dir_all(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            // socket directory already exists: this is the expected case when
            // the application is not running for the first time and the socket dir
            // was therefore already created previously.
            Ok(())
        }
        err => err,
    };

    result?;

    Ok(path)
}

fn get_socket_path() -> Result<PathBuf, String> {
    if let Ok(runtime_dir) = env::var("RUNTIME_DIRECTORY") {
        // RUNTIME_DIRECTORY is set if this application runs via systemd: More details here:
        // https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#RuntimeDirectory=
        Ok(Path::new(&runtime_dir).to_path_buf())
    } else if let Ok(xdg_runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        Ok(Path::new(&xdg_runtime_dir).join(APPLICATION_NAME))
    } else {
        Err(
            "Neither RUNTIME_DIRECTORY nor XDG_RUNTIME_DIR environment variables are set."
                .to_string(),
        )
    }
}

pub fn remove_socketfile(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(_) => {
            // File successfully removed.
            Ok(())
        }
        Err(e) if e.kind() == NotFound => {
            // No file to remove, because it didn't exist in the first place.
            Ok(())
        }
        err => err,
    }
}

pub fn handle_client(mut stream: UnixStream, tx: Arc<Sender<ClientMessage>>) {
    let message_result = read_string(&mut stream);
    match message_result {
        Ok(s) if s == "block_current_song\n" || s == "block_current_song" => {
            let message = ClientMessage::BlockCurrentSong;
            if let Err(e) = tx.send(message) {
                warn!("Unable to send message {:?}: {:?}", message, e);
            }
        }
        Ok(s) => {
            warn!("ClientMessage not recognized: {}", s);
        }
        Err(e) => {
            error!("Unable to read message from socket: {:?}", e);
        }
    };
}

fn read_string<R>(stream: &mut R) -> io::Result<String>
where
    R: Read,
{
    let mut buffer = String::new();
    stream.read_to_string(&mut buffer)?;
    Ok(buffer)
}
