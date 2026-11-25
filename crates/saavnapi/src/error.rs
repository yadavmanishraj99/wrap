//! Error type for the Saavn client.

use thiserror::Error;

/// Every failure mode the client can report.
///
/// The reference implementation mostly throws a bare 404/500 with a string
/// message; here each case is a typed variant so callers can match on it.
#[derive(Debug, Error)]
pub enum Error {
    /// Transport failure (DNS, TLS, connection, timeout, …).
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// The server returned a non-2xx status.
    #[error("saavn returned HTTP {0}")]
    HttpStatus(reqwest::StatusCode),

    /// The response body was not valid JSON, or did not have the shape
    /// a method expected (e.g. an empty `songs` array where one was required).
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// JSON parsing failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The requested entity does not exist (mirrors the reference 404s).
    #[error("not found: {0}")]
    NotFound(String),

    /// Caller input was missing or malformed (bad link, empty id list, …).
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// DES-ECB decryption of `encrypted_media_url` failed, or the
    /// base64 payload was malformed.
    #[error("could not decrypt download url: {0}")]
    Decrypt(String),

    /// `webradio.createEntityStation` returned no `stationid`.
    #[error("could not create station")]
    Station,
}

pub type Result<T> = std::result::Result<T, Error>;
