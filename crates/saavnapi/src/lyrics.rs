//! Lyrics — `lyrics.getLyrics`.
//!
//! Live-verified 2026-11-25 (was a dead constant in the sumitkolhe
//! reference, real in the TS/Python/Dart wrappers):
//! `__call=lyrics.getLyrics`, `lyrics_id=<song pid>`, `ctx=web6dot0`.
//! The `lyrics_id` is just the song's pid — but `song.getDetails`
//! never reports one, and `has_lyrics` is `"false"` even for songs
//! that do have lyrics, so there is no way to know in advance.
//! The server signals "no lyrics" as
//! `{"status":"failure","error":{"msg":"Something went wrong please
//! try again"}}`, which this method maps to `Ok(None)`.
//! `lyrics` is HTML-ish text: lines separated by `<br>`,
//! blank lines `<br><br>`.

use crate::client::{Context, SaavnClient};
use crate::error::{Error, Result};
use crate::models::Lyrics;
use crate::payload::lyrics_payload;

impl SaavnClient {
    /// Lyrics for one song, or `None` when the song has none.
    ///
    /// ```no_run
    /// use saavnapi::SaavnClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), saavnapi::Error> {
    /// let client = SaavnClient::new();
    /// // "Rula Ke Gaya Ishq" has lyrics; "Levitating" does not.
    /// let lyrics = client.get_lyrics("bPBAx5IK").await?;
    /// println!("{:?}", lyrics.map(|l| l.text.len()));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_lyrics(&self, song_id: &str) -> Result<Option<Lyrics>> {
        if song_id.trim().is_empty() {
            return Err(Error::InvalidInput("song id is required".into()));
        }
        let data = self
            .fetch(
                "lyrics.getLyrics",
                &[("lyrics_id", song_id.to_string())],
                Context::Web,
            )
            .await?;
        // "No lyrics" and a transient server failure are the same
        // response (`{"status":"failure","error":{"msg":"Something
        // went wrong please try again"}}`) — the server does not
        // distinguish them, so neither does this method: both are
        // `Ok(None)`, i.e. "no lyrics available right now".
        Ok(lyrics_payload(&data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lyrics_payload_maps_live_shape() {
        let raw = json!({
            "lyrics": "Ho O Oo…<br><br>Kaash Tu Mere Haq Mein Hota",
            "script_tracking_url": "https://www.jiosaavn.com/stats.php?ev=lyrics.track",
            "lyrics_copyright": "Lyrics powered by JioSaavn",
            "snippet": "Lyrics"
        });
        let lyrics = lyrics_payload(&raw).unwrap();
        assert!(lyrics.text.contains("<br><br>"));
        assert_eq!(
            lyrics.copyright.as_deref(),
            Some("Lyrics powered by JioSaavn")
        );
    }

    #[test]
    fn lyrics_payload_none_on_failure_shape() {
        let raw =
            json!({"status": "failure", "error": {"msg": "Something went wrong please try again"}});
        assert!(lyrics_payload(&raw).is_none());
        assert!(lyrics_payload(&json!({})).is_none());
        assert!(lyrics_payload(&json!({"lyrics": ""})).is_none());
    }
}
