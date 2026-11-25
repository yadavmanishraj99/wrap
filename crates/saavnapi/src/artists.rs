//! Artists — `artist.getArtistPageDetails`, `webapi.get`,
//! `artist.getArtistMoreSong`, `artist.getArtistMoreAlbum`.
//!
//! Reference: `src/modules/artists/` (4 use-cases).
//!
//! Wire mapping (all four): `sortBy` → `category`,
//! `sortOrder` → `sort_order`.
//! Controller defaults: `page=0`, `song_count=10`, `album_count=10`,
//! `sort_by=popularity`, `sort_order=asc` on the detail routes and
//! `desc` on the songs/albums sub-routes (an inconsistency in the
//! reference, preserved here — pass the order explicitly if it matters).

use crate::client::{Context, SaavnClient, SortBy, SortOrder};
use crate::error::{Error, Result};
use crate::models::{Artist, ArtistAlbums, ArtistSongs};
use crate::payload::{album_payload, artist_payload, as_array, song_payload};

/// Arguments for `get_artist_by_id` / `get_artist_by_link`.
#[derive(Debug, Clone)]
pub struct ArtistArgs {
    pub page: u32,
    pub song_count: u32,
    pub album_count: u32,
    pub sort_by: SortBy,
    pub sort_order: SortOrder,
}

impl Default for ArtistArgs {
    fn default() -> Self {
        Self {
            page: 0,
            song_count: 10,
            album_count: 10,
            sort_by: SortBy::Popularity,
            sort_order: SortOrder::Asc,
        }
    }
}

/// Arguments for `get_artist_songs` / `get_artist_albums`.
#[derive(Debug, Clone)]
pub struct ArtistListArgs {
    pub page: u32,
    pub sort_by: SortBy,
    pub sort_order: SortOrder,
}

impl Default for ArtistListArgs {
    fn default() -> Self {
        Self {
            page: 0,
            sort_by: SortBy::Popularity,
            // Reference quirk: detail routes default to `asc`, the
            // songs/albums sub-routes default to `desc`.
            sort_order: SortOrder::Desc,
        }
    }
}

impl SaavnClient {
    /// Get an artist by ID — `__call=artist.getArtistPageDetails`,
    /// `artistId`, `n_song`, `n_album`, `page`, `sort_order`, `category`.
    pub async fn get_artist_by_id(&self, artist_id: &str, args: ArtistArgs) -> Result<Artist> {
        let data = self
            .fetch(
                "artist.getArtistPageDetails",
                &[
                    ("artistId", artist_id.to_string()),
                    ("n_song", args.song_count.to_string()),
                    ("n_album", args.album_count.to_string()),
                    ("page", args.page.to_string()),
                    ("sort_order", args.sort_order.as_str().to_string()),
                    ("category", args.sort_by.as_str().to_string()),
                ],
                Context::Web,
            )
            .await?;
        if data.is_null() {
            return Err(Error::NotFound("artist not found".into()));
        }
        Ok(artist_payload(&data))
    }

    /// Get an artist by link — `__call=webapi.get`, `token`, `type=artist`
    /// plus the same `n_song` / `n_album` / `page` / `sort_order` / `category`.
    pub async fn get_artist_by_link(&self, link: &str, args: ArtistArgs) -> Result<Artist> {
        let token = Self::token_from_link(link)?;
        let data = self
            .fetch(
                "webapi.get",
                &[
                    ("token", token),
                    ("n_song", args.song_count.to_string()),
                    ("n_album", args.album_count.to_string()),
                    ("page", args.page.to_string()),
                    ("sort_order", args.sort_order.as_str().to_string()),
                    ("category", args.sort_by.as_str().to_string()),
                    ("type", "artist".to_string()),
                ],
                Context::Web,
            )
            .await?;
        if data.is_null() {
            return Err(Error::NotFound("artist not found".into()));
        }
        Ok(artist_payload(&data))
    }

    /// Artist songs — `__call=artist.getArtistMoreSong`, `artistId`,
    /// `page`, `sort_order`, `category` (no size param — server decides).
    /// Reads the nested `topSongs { total, songs }` object.
    pub async fn get_artist_songs(
        &self,
        artist_id: &str,
        args: ArtistListArgs,
    ) -> Result<ArtistSongs> {
        let data = self
            .fetch(
                "artist.getArtistMoreSong",
                &[
                    ("artistId", artist_id.to_string()),
                    ("page", args.page.to_string()),
                    ("sort_order", args.sort_order.as_str().to_string()),
                    ("category", args.sort_by.as_str().to_string()),
                ],
                Context::Web,
            )
            .await?;
        if data.is_null() {
            return Err(Error::NotFound("artist songs not found".into()));
        }
        let top = data
            .get("topSongs")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        Ok(ArtistSongs {
            total: top.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
            songs: as_array(&top, "songs").iter().map(song_payload).collect(),
        })
    }

    /// Artist albums — `__call=artist.getArtistMoreAlbum`, same params
    /// as songs. Reads the nested `topAlbums { total, albums }` object.
    pub async fn get_artist_albums(
        &self,
        artist_id: &str,
        args: ArtistListArgs,
    ) -> Result<ArtistAlbums> {
        let data = self
            .fetch(
                "artist.getArtistMoreAlbum",
                &[
                    ("artistId", artist_id.to_string()),
                    ("page", args.page.to_string()),
                    ("sort_order", args.sort_order.as_str().to_string()),
                    ("category", args.sort_by.as_str().to_string()),
                ],
                Context::Web,
            )
            .await?;
        if data.is_null() {
            return Err(Error::NotFound("artist albums not found".into()));
        }
        let top = data
            .get("topAlbums")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        Ok(ArtistAlbums {
            total: top.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
            albums: as_array(&top, "albums").iter().map(album_payload).collect(),
        })
    }
}
