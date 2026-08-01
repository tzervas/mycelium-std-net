//! Never-silent network error carrier (G2).

use std::fmt;

/// Explicit failure from a network op. Never a silent empty/zero body on transport failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// Method string was empty or not valid UTF-8 when decoded from host encoding.
    InvalidMethod { why: String },
    /// URL string was empty, not absolute, or not valid UTF-8.
    InvalidUrl { why: String },
    /// Header name/value rejected (empty name, embedded newline, etc.).
    InvalidHeader { why: String },
    /// DNS / connect / TLS / protocol failure from the underlying client.
    Transport { why: String },
    /// Response body could not be fully read (I/O or size limit).
    BodyRead { why: String },
    /// Status line / response framing could not be interpreted.
    Protocol { why: String },
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::InvalidMethod { why } => write!(f, "invalid HTTP method: {why}"),
            NetError::InvalidUrl { why } => write!(f, "invalid URL: {why}"),
            NetError::InvalidHeader { why } => write!(f, "invalid header: {why}"),
            NetError::Transport { why } => write!(f, "transport error: {why}"),
            NetError::BodyRead { why } => write!(f, "body read error: {why}"),
            NetError::Protocol { why } => write!(f, "protocol error: {why}"),
        }
    }
}

impl std::error::Error for NetError {}

impl From<ureq::Error> for NetError {
    fn from(e: ureq::Error) -> Self {
        // ureq 3: status codes are not errors when http_status_as_error is false;
        // remaining errors are transport/protocol.
        NetError::Transport { why: e.to_string() }
    }
}
