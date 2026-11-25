//! The client itself: one `GET https://www.jiosaavn.com/api.php`
//! per call, exactly as the reference's `useFetch` does it.
//!
//! Fixed query parameters on every request:
//! `__call=<endpoint>`, `_format=json`, `_marker=0`, `api_version=4`,
//! `ctx=<web6dot0|android>`, then the call-specific parameters.
//!
//! Headers: `Content-Type: application/json` and a random
//! `User-Agent` per request. No auth, no cookies, no API key, no
//! `Referer` — that is the whole identity the reference uses.
//!
//! The reference's UA list has 100 entries; this port ships 12
//! representative ones (Chrome/Firefox/Safari/Edge, desktop + mobile).
//! Rotation, not a fixed UA, is the behaviour that matters.

use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Client as HttpClient;
use serde_json::Value;

use crate::error::{Error, Result};

/// Base URL for every Saavn call.
pub const BASE_URL: &str = "https://www.jiosaavn.com/api.php";

/// `ctx` values — `ApiContextEnum` in the reference.
/// Default `web6dot0`; `android` only for the two `webradio.*` calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Web,
    Android,
}

impl Context {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Context::Web => "web6dot0",
            Context::Android => "android",
        }
    }
}

/// Sort field for artist endpoints (`category` on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    #[default]
    Popularity,
    Latest,
    Alphabetical,
}

impl SortBy {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            SortBy::Popularity => "popularity",
            SortBy::Latest => "latest",
            SortBy::Alphabetical => "alphabetical",
        }
    }
}

/// Sort direction for artist endpoints (`sort_order` on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

impl SortOrder {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            SortOrder::Asc => "asc",
            SortOrder::Desc => "desc",
        }
    }
}

const USER_AGENTS: [&str; 12] = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36 Edg/134.0.0.0",
    "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Mobile Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_3_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3.1 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3.1 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Linux; Android 13; SM-G981B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Mobile Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 OPR/117.0.0.0",
];

fn random_user_agent() -> &'static str {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0);
    USER_AGENTS[nanos % USER_AGENTS.len()]
}

/// A JioSaavn API client.
///
/// ```no_run
/// use saavnapi::SaavnClient;
/// # #[tokio::main]
/// # async fn main() -> Result<(), saavnapi::Error> {
/// let client = SaavnClient::new();
/// let songs = client.get_songs_by_ids(&["3IoDK8qI"]).await?;
/// println!("{} — {}", songs[0].name, songs[0].download_url[4].url);
/// # Ok(())
/// # }
/// ```
///
/// All methods are `async` and hit `api.php` directly. This client
/// never talks to `saavn.dev` or any other hosted wrapper.
#[derive(Debug, Clone)]
pub struct SaavnClient {
    http: HttpClient,
    base_url: String,
}

impl Default for SaavnClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SaavnClient {
    pub fn new() -> Self {
        Self {
            http: HttpClient::new(),
            base_url: BASE_URL.to_string(),
        }
    }

    /// Override the base URL (tests / a future Saavn host change).
    /// The default is `https://www.jiosaavn.com/api.php`.
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            http: HttpClient::new(),
            base_url: base_url.into(),
        }
    }

    /// The `useFetch` equivalent: build the URL, GET it, parse JSON.
    /// Returns the whole parsed body as `Value` — callers map it.
    pub(crate) async fn fetch(
        &self,
        call: &str,
        params: &[(&str, String)],
        context: Context,
    ) -> Result<Value> {
        let mut url =
            reqwest::Url::parse(&self.base_url).map_err(|e| Error::InvalidInput(e.to_string()))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("__call", call);
            q.append_pair("_format", "json");
            q.append_pair("_marker", "0");
            q.append_pair("api_version", "4");
            q.append_pair("ctx", context.as_str());
            for (k, v) in params {
                q.append_pair(k, v);
            }
        }
        let response = self
            .http
            .get(url)
            .header("Content-Type", "application/json")
            .header("User-Agent", random_user_agent())
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::HttpStatus(status));
        }
        Ok(response.json::<Value>().await?)
    }

    /// Extract the token (last path segment) from a JioSaavn link.
    ///
    /// Reference regexes (song/album/artist):
    /// `/jiosaavn\.com\/(song|album|artist)\/[^/]+\/([^/]+)$/`
    /// Playlist also accepts `saavn.com` and `/featured/` or
    /// `/s/playlist/` paths — in every case the token is the last
    /// path segment, so one rule covers all four.
    /// A bare token (no `/`) is returned unchanged.
    pub(crate) fn token_from_link(link: &str) -> Result<String> {
        let trimmed = link.trim().trim_end_matches('/');
        // A URL with no path (e.g. https://www.jiosaavn.com/) has no token:
        // the last segment would just be the host.
        let path = if let Some(after_scheme) = trimmed.split("://").nth(1) {
            match after_scheme.find('/') {
                Some(pos) => &after_scheme[pos + 1..],
                None => "",
            }
        } else {
            trimmed
        };
        let token = path.rsplit('/').next().unwrap_or("").trim();
        if token.is_empty() {
            return Err(Error::InvalidInput(format!(
                "could not extract a token from link: {link}"
            )));
        }
        // Strip a query string if one was pasted with the link.
        let token = token.split(['?', '#']).next().unwrap_or(token);
        Ok(token.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_extraction_matches_reference_regexes() {
        assert_eq!(
            SaavnClient::token_from_link("https://www.jiosaavn.com/song/houdini/OgwhbhtDRwM")
                .unwrap(),
            "OgwhbhtDRwM"
        );
        assert_eq!(
            SaavnClient::token_from_link(
                "https://www.jiosaavn.com/album/future-nostalgia/ITIyo-GDr7A_"
            )
            .unwrap(),
            "ITIyo-GDr7A_"
        );
        assert_eq!(
            SaavnClient::token_from_link(
                "https://www.jiosaavn.com/artist/dua-lipa-songs/r-OWIKgpX2I_"
            )
            .unwrap(),
            "r-OWIKgpX2I_"
        );
        assert_eq!(
            SaavnClient::token_from_link(
                "https://www.jiosaavn.com/featured/its-indie-english/AMoxtXyKHoU_"
            )
            .unwrap(),
            "AMoxtXyKHoU_"
        );
        assert_eq!(
            SaavnClient::token_from_link(
                "https://www.saavn.com/s/playlist/cf3c2fb07449311f87f53670da0e3d20/gautham-menon-telugu-hits/4sylrSC21MjvZ3uUE6bUVw__"
            )
            .unwrap(),
            "4sylrSC21MjvZ3uUE6bUVw__"
        );
        assert_eq!(
            SaavnClient::token_from_link("OgwhbhtDRwM").unwrap(),
            "OgwhbhtDRwM"
        );
        assert!(SaavnClient::token_from_link("https://www.jiosaavn.com/").is_err());
    }
}
