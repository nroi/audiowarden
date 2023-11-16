use crate::http::spotify::client::ClientConnectionHandlingError;
use std::io;

#[derive(Debug)]
pub enum AudioWardenError {
    ClientConnectionHandling(ClientConnectionHandlingError),
    Io(io::Error),
    Generic(String),
}

impl From<io::Error> for AudioWardenError {
    fn from(error: io::Error) -> Self {
        AudioWardenError::Io(error)
    }
}

impl From<String> for AudioWardenError {
    fn from(error: String) -> Self {
        AudioWardenError::Generic(error)
    }
}

impl From<ClientConnectionHandlingError> for AudioWardenError {
    fn from(error: ClientConnectionHandlingError) -> Self {
        AudioWardenError::ClientConnectionHandling(error)
    }
}
