//! Unofficial Rust client for JioSaavn.
//!
//! Built from scratch against the real JioSaavn web API —
//! `GET https://www.jiosaavn.com/api.php?__call=…` — as reverse-engineered
//! from the `sumitkolhe/jiosaavn-api` TypeScript reference (cloned and
//! studied in `~/workspace/wrap-study/jiosaavn-api`). It does **not**
//! use that project's hosted API (`saavn.dev`), its client, or any of
//! its code; only the wire knowledge was carried over (see `README.md`
//! for the full endpoint table and the study notes).
//!
//! # Quick start
//!
//! ```no_run
//! use saavnapi::SaavnClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), saavnapi::Error> {
//!     let client = SaavnClient::new();
//!     let songs = client.get_songs_by_ids(&["3IoDK8qI"]).await?;
//!     let song = &songs[0];
//!     println!("{} ({})", song.name, song.year.as_deref().unwrap_or(""));
//!     for link in &song.download_url {
//!         println!("{}: {}", link.quality, link.url);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # What is (and isn't) here
//!
//! * All 17 use-cases the reference actually wires up: songs by
//!   id/link, song suggestions (two-step station flow), albums by
//!   id/link, artists by id/link + their songs/albums, playlists by
//!   id/link, and all five searches.
//! * Audio URLs are decrypted in-process (DES-ECB, key `38346591`)
//!   and expanded to 12/48/96/160/320 kbps; images to 50/150/500 px.
//! * Home / launch data is implemented too (`get_launch_data`,
//!   `get_browse_modules`, `get_trending`), even though the reference
//!   never wired it up — `webapi.getLaunchData` is a real endpoint,
//!   verified live (see `src/home.rs`).
//! * **Not implemented, deliberately:** `lyrics.getLyrics` exists as
//!   a constant in the reference but has no use-case, service or route
//!   there — so there is no verified wire behaviour to port. The
//!   client also never downloads audio bytes itself; it only returns
//!   the decrypted CDN URLs, like the reference.
//! * Unofficial and unauthenticated: JioSaavn may change or block
//!   this at any time. No auth, no cookies, no API key.

pub mod albums;
pub mod artists;
pub mod chrome;
pub mod client;
pub mod collections;
pub mod discovery;
pub mod error;
pub mod home;
pub mod links;
pub mod lyrics;
pub mod models;
pub mod payload;
pub mod playlists;
pub mod radio;
pub mod reco;
pub mod search;
pub mod shows;
pub mod songs;
pub mod streaming;

pub use artists::{ArtistArgs, ArtistListArgs};
pub use client::{Context, SaavnClient, SortBy, SortOrder, BASE_URL};
pub use error::{Error, Result};
pub use links::{create_download_links, create_image_links, decrypt_media_url, DownloadLink};
pub use models::*;
