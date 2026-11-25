//! Download / image link construction.
//!
//! Ported from `src/common/helpers/link.helper.ts` in the reference repo
//! (itself the only place that knows how JioSaavn encrypts audio URLs).
//!
//! Audio:
//! * `more_info.encrypted_media_url` is base64.
//! * It decrypts with **DES-ECB, key `"38346591"`** (8 ASCII bytes).
//!   The IV `"00000000"` in the reference is dead — ECB has no IV, and
//!   node-forge ignores it. Do not feed it into a CBC IV.
//! * The plaintext is a URL containing `_96` (the 96 kbps variant).
//!   The five qualities are produced by replacing the **first** `_96`
//!   with `_12` / `_48` / `_96` / `_160` / `_320`.
//! * Empty input returns `[]`, never an error.
//!
//! Images:
//! * The source URL contains a size token `150x150` or `50x50`.
//!   Replace the **first** occurrence with `50x50` / `150x150` / `500x500`.
//! * Force `http://` → `https://`.
//! * Quirk (preserved): if the source is already `500x500` the regex
//!   matches nothing, so all three entries are the same URL.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyInit};
use des::Des;
use ecb::Decryptor;

use crate::error::{Error, Result};

/// One link at one quality — used for both audio and images, exactly as
/// in the reference (`DownloadLinkModel`: `{ quality, url }`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DownloadLink {
    pub quality: String,
    pub url: String,
}

/// The five audio qualities, in the reference's order.
pub const AUDIO_QUALITIES: [(&str, &str); 5] = [
    ("_12", "12kbps"),
    ("_48", "48kbps"),
    ("_96", "96kbps"),
    ("_160", "160kbps"),
    ("_320", "320kbps"),
];

/// The three image qualities, in the reference's order.
pub const IMAGE_QUALITIES: [&str; 3] = ["50x50", "150x150", "500x500"];

/// Decrypt one `encrypted_media_url` value to its plaintext URL.
///
/// This is the low-level primitive; [`create_download_links`] is what
/// song payloads use. PKCS#7 padding is stripped (node-forge does the
/// same on `decipher.finish()`).
pub fn decrypt_media_url(encrypted_media_url: &str) -> Result<String> {
    let ciphertext = BASE64
        .decode(encrypted_media_url.trim())
        .map_err(|e| Error::Decrypt(e.to_string()))?;
    if ciphertext.is_empty() || ciphertext.len() % 8 != 0 {
        return Err(Error::Decrypt(
            "ciphertext length is not a multiple of 8".to_string(),
        ));
    }
    let decryptor = Decryptor::<Des>::new(b"38346591".into());
    let mut buffer = ciphertext;
    let plaintext = decryptor
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|e| Error::Decrypt(e.to_string()))?;
    String::from_utf8(plaintext.to_vec()).map_err(|e| Error::Decrypt(e.to_string()))
}

/// Build the five audio links for a song, or `[]` when there is no
/// encrypted URL (or it cannot be decrypted — a bad URL must not fail
/// the whole song, mirroring the reference's lenient payload mapping).
pub fn create_download_links(encrypted_media_url: Option<&str>) -> Vec<DownloadLink> {
    let Some(enc) = encrypted_media_url.filter(|s| !s.is_empty()) else {
        return Vec::new();
    };
    let Ok(url) = decrypt_media_url(enc) else {
        return Vec::new();
    };
    AUDIO_QUALITIES
        .iter()
        .map(|(suffix, bitrate)| DownloadLink {
            quality: bitrate.to_string(),
            url: url.replacen("_96", suffix, 1),
        })
        .collect()
}

/// Build the three image links for a raw image URL, or `[]` when empty.
pub fn create_image_links(link: Option<&str>) -> Vec<DownloadLink> {
    let Some(link) = link.filter(|s| !s.is_empty()) else {
        return Vec::new();
    };
    IMAGE_QUALITIES
        .iter()
        .map(|quality| {
            let mut url = link.to_string();
            // Replace the first size token only, exactly like /150x150|50x50/.
            if let Some(pos) = url.find("150x150").or_else(|| url.find("50x50")) {
                url.replace_range(pos..pos + 7, quality);
            }
            if let Some(rest) = url.strip_prefix("http://") {
                url = format!("https://{rest}");
            }
            DownloadLink {
                quality: quality.to_string(),
                url,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live vector, verified 2026-11-25 against the real API:
    /// `song.getDetails` for `3IoDK8qI` ("Levitating", Dua Lipa).
    /// Also verified with `openssl enc -d -des-ecb -K 3338333436353931`.
    const ENCRYPTED: &str = "ID2ieOjCrwfgWvL5sXl4B1ImC5QfbsDylsUbBKiBMWa4cKhEF4Xz5p975Hh3jSc+rXO0khV0lO1tzxLIEtHbjhw7tS9a8Gtq";
    const PLAINTEXT: &str = "https://aac.saavncdn.com/665/7790c3b9097592113008eaf1031d6e57_96.mp4";

    #[test]
    fn decrypts_known_vector() {
        assert_eq!(decrypt_media_url(ENCRYPTED).unwrap(), PLAINTEXT);
    }

    #[test]
    fn download_links_swap_quality() {
        let links = create_download_links(Some(ENCRYPTED));
        assert_eq!(links.len(), 5);
        assert_eq!(links[0].quality, "12kbps");
        assert!(links[0].url.ends_with("_12.mp4"));
        assert_eq!(links[2].quality, "96kbps");
        assert_eq!(links[2].url, PLAINTEXT);
        assert_eq!(links[4].quality, "320kbps");
        assert!(links[4].url.ends_with("_320.mp4"));
    }

    #[test]
    fn empty_encrypted_url_gives_no_links() {
        assert!(create_download_links(None).is_empty());
        assert!(create_download_links(Some("")).is_empty());
        assert!(create_download_links(Some("!!!not-base64!!!")).is_empty());
    }

    #[test]
    fn image_links_expand_sizes_and_force_https() {
        let links = create_image_links(Some(
            "http://c.saavncdn.com/665/Future-Nostalgia-English-2020-150x150.jpg",
        ));
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].quality, "50x50");
        assert!(links[0].url.starts_with("https://"));
        assert!(links[0].url.ends_with("-50x50.jpg"));
        assert!(links[1].url.ends_with("-150x150.jpg"));
        assert!(links[2].url.ends_with("-500x500.jpg"));
    }

    #[test]
    fn image_links_empty_gives_none() {
        assert!(create_image_links(None).is_empty());
    }
}
