use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::{io, thread};

use regex::Regex;
use url::{ParseError, Url};

use http::spotify::client;

use crate::file_io::state;
use crate::http;
use crate::http::spotify::client::TokenContainer;

pub fn listen(code_verifier: &str, state: &str, auth_url: &Url) -> io::Result<()> {
    let code_verifier = code_verifier.to_string();
    let state = state.to_string();
    let auth_url = auth_url.clone();

    let listener = TcpListener::bind(LISTEN_ADDRESS)?;
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let result = handle_connection(&mut stream, &code_verifier, &state);
            match result {
                Ok(HandleConnectionResult::BadRequest) => {
                    let response = "HTTP/1.1 400 Bad Request\r\n\
                        Content-Type: text/plain\r\n\
                        Content-Length: 12\r\n\r\n\
                        Bad Request\n";
                    stream.write_all(response.as_bytes()).unwrap();
                }
                Ok(HandleConnectionResult::InitiateAuth) => {
                    let response = format!("HTTP/1.1 302 Found\r\nLocation: {}\r\n\r\n", auth_url);
                    stream.write_all(response.as_bytes()).unwrap();
                }
                Ok(HandleConnectionResult::Redirect(true)) => {
                    let response = "HTTP/1.1 200 OK\r\n\
                        Content-Type: text/plain\r\n\
                        Content-Length: 3\r\n\r\n\
                        OK\n";
                    stream.write_all(response.as_bytes()).unwrap();
                    // If we got the code, return, in order to remove the listener and not leave the
                    // TCP socket open without any good reason.
                    return;
                }
                Ok(HandleConnectionResult::Redirect(false)) => {
                    // Keep listening, maybe the client accidentally sent the wrong request and
                    // will subsequently send a correct request.
                }
                Err(e) => {
                    error!("Something went wrong: {:?}", e);
                    return;
                }
            }
        }
    });

    Ok(())
}

// Returns true if we received the code from spotify, false otherwise.
fn handle_connection(
    stream: &mut TcpStream,
    code_verifier: &str,
    state: &str,
) -> Result<HandleConnectionResult, client::ClientConnectionHandlingError> {
    let request_target = request_target_from_stream(stream)?;
    if request_target == "/authorize_audiowarden" {
        Ok(HandleConnectionResult::InitiateAuth)
    } else {
        match get_query_params(&request_target) {
            Ok(Some(query_params)) => {
                if query_params.state == state {
                    let token = client::get_token(&query_params.code, code_verifier)?;
                    if let Err(e) = state::store_spotify_token(&token) {
                        error!("Unable to store spotify token: {:?}", e)
                    }
                    let mut token_container = TokenContainer::new(token);
                    if let Err(e) = client::update_blocked_songs_in_cache(&mut token_container) {
                        error!("Unable to update blocked songs: {:?}", e);
                    }
                    Ok(HandleConnectionResult::Redirect(true))
                } else {
                    // The state from the redirect URI does not match the state that we previously
                    // generated. OAuth uses the state param as a security measure against CSRF
                    // attacks, so we abort the auth process here.
                    Ok(HandleConnectionResult::Redirect(false))
                }
            }
            Ok(None) => {
                // No technical errors have occurred, but the HTTP request does not contain the code
                // and state query params that we require to complete the login process.
                Ok(HandleConnectionResult::BadRequest)
            }
            Err(e) => {
                // Some technical error has occurred, e.g., the client has sent rubbish over the TCP
                // connection.
                Err(e)
            }
        }
    }
}

enum HandleConnectionResult {
    /// The GET request was executed by the client after the client was redirected from Spotify's
    /// authorization flow. The boolean value is true for success and false for failure.
    Redirect(bool),
    /// The request cannot be processed by audiowarden.
    BadRequest,
    /// The client requested to initiate the authorization process at Spotify.
    InitiateAuth,
}

fn request_target_from_stream(
    mut stream: &mut TcpStream,
) -> Result<String, client::ClientConnectionHandlingError> {
    let buf_reader = BufReader::new(&mut stream);
    let http_request: Vec<String> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();

    match http_request.get(0) {
        Some(http_request_line) => match request_target(http_request_line) {
            Some(target) => Ok(target),
            None => {
                let message = "Unable to parse HTTP data: Probably, we've received \
                    invalid data over the TCP connection."
                    .to_string();
                Err(client::ClientConnectionHandlingError::HttpProtocolError(
                    message,
                ))
            }
        },
        None => Err(client::ClientConnectionHandlingError::HttpProtocolError(
            "Received TCP data that does not resemble a valid HTTP request".to_string(),
        )),
    }
}

fn get_query_params(
    request_target: &str,
) -> Result<Option<QueryParamsForSpotifyAuthorization>, client::ClientConnectionHandlingError> {
    let query_params = match query_params(request_target) {
        Ok(params) => params,
        Err(e) => {
            let message = format!(
                "Unable to parse query params: {:?}. Probably, \
                    we've received invalid data over the TCP connection.",
                e
            );
            return Err(client::ClientConnectionHandlingError::HttpProtocolError(
                message,
            ));
        }
    };
    let code = query_params.get("code");
    let state = query_params.get("state");
    match (code, state) {
        (None, None) => {
            warn!("Neither code nor state are present in the URL.");
            Ok(None)
        }
        (Some(_), None) => {
            warn!("state is not present in the URL.");
            Ok(None)
        }
        (None, Some(_)) => {
            warn!("code is not present in the URL.");
            Ok(None)
        }
        (Some(c), Some(s)) => Ok(Some(QueryParamsForSpotifyAuthorization {
            code: c.to_string(),
            state: s.to_string(),
        })),
    }
}

fn request_target(http_request_line: &str) -> Option<String> {
    // Check if input is a valid HTTP request line (RFC 7230):
    let pattern = r"^(?P<method>[A-Z]+) (?P<request_target>[^ ]+) (?P<version>HTTP/\d\.\d)$";
    let regex = Regex::new(pattern).unwrap();
    let captures = regex.captures(http_request_line)?;
    captures
        .name("request_target")
        .map(|target_match| target_match.as_str())
        .map(|s| s.to_string())
}

fn query_params(request_target: &str) -> Result<HashMap<String, String>, ParseError> {
    // We're using https://example.com as a dummy URL just to have a valid URL that we can then
    // use to parse the query params.
    let url_as_string = format!("https://example.com{}", request_target);
    let url = Url::parse(&url_as_string)?;
    let params = url
        .query_pairs()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();

    Ok(params)
}

struct QueryParamsForSpotifyAuthorization {
    code: String,
    state: String,
}

const LISTEN_ADDRESS: &str = "127.0.0.1:7185";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_target_get_request() {
        let result = request_target("GET /api/users/123 HTTP/1.1");
        assert_eq!(result, Some("/api/users/123".to_string()));
    }

    #[test]
    fn test_request_target_empty_string() {
        let result = request_target("");
        assert_eq!(result, None);
    }

    #[test]
    fn test_request_target_incomplete_string() {
        let result = request_target("GET ");
        assert_eq!(result, None);
    }

    #[test]
    fn test_query_params() {
        let result = query_params("/example/path?param1=value1&param2=value2");
        let expected = HashMap::from([
            ("param1".to_string(), "value1".to_string()),
            ("param2".to_string(), "value2".to_string()),
        ]);

        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_query_params_invalid_input() {
        let result = query_params(":foo");
        assert!(result.is_err());
    }
}
