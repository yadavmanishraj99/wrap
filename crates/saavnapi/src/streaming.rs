//! Signed streaming URLs — `song.generateAuthToken`.
//!
//! Live-verified 2026-11-25 (the call yt-dlp makes). This is the
//! server-side alternative to the local DES decryption in `links.rs`:
//! it takes an `encrypted_media_url` and a bitrate and returns a
//! **signed, short-lived** playable URL on `web.saavncdn.com`
//! (CloudFront `Expires`/`Signature`/`Key-Pair-Id`) plus a JWE token.
//! The raw DES-decrypted `aac.saavncdn.com` URL is unsigned —
//! this is the only route to a playable signed URL.
//! Works via GET and POST; this client uses GET.

use crate::client::{Context, SaavnClient};
use crate::error::{Error, Result};
use crate::models::AuthToken;
use crate::payload::auth_token_payload;

impl SaavnClient {
    /// A signed streaming URL for a song's encrypted media URL.
    /// `bitrate` is one of 16/32/64/96/128/160/320 (96 and 320
    /// verified live); the returned URL's `_96`/`_320` suffix follows
    /// the requested bitrate.
    pub async fn generate_auth_token(
        &self,
        encrypted_media_url: &str,
        bitrate: u32,
    ) -> Result<AuthToken> {
        if encrypted_media_url.trim().is_empty() {
            return Err(Error::InvalidInput(
                "encrypted_media_url is required".into(),
            ));
        }
        let data = self
            .fetch(
                "song.generateAuthToken",
                &[
                    ("url", encrypted_media_url.to_string()),
                    ("bitrate", bitrate.to_string()),
                ],
                Context::Web,
            )
            .await?;
        auth_token_payload(&data).ok_or_else(|| {
            Error::InvalidResponse("generateAuthToken did not return a signed URL".into())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn auth_token_payload_maps_live_shape() {
        let raw = json!({
            "auth_url": "https://web.saavncdn.com/665/x_96.mp4?Expires=1&Signature=2&Key-Pair-Id=3",
            "type": "mp4", "status": "success",
            "jwe_token": "eyJhbGciOiJSU0EtT0FFUCJ9"
        });
        let token = auth_token_payload(&raw).unwrap();
        assert!(token.auth_url.starts_with("https://web.saavncdn.com/"));
        assert_eq!(token.kind, "mp4");
        assert!(token.jwe_token.is_some());
        assert!(auth_token_payload(&json!({"status": "failure"})).is_none());
        assert!(auth_token_payload(&json!({})).is_none());
    }
}
