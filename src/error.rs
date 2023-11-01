use std::io;

#[derive(Debug)]
pub enum AudioWardenError {
    IoError(io::Error),
    GenericError(String),
}

impl From<io::Error> for AudioWardenError {
    fn from(error: io::Error) -> Self {
        AudioWardenError::IoError(error)
    }
}

impl From<String> for AudioWardenError {
    fn from(error: String) -> Self {
        AudioWardenError::GenericError(error)
    }
}
