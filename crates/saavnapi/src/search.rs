//! Search — `autocomplete.get`, `search.getResults`,
//! `search.getAlbumResults`, `search.getArtistResults`,
//! `search.getPlaylistResults`, plus the helpers found in the
//! whole-picture exploration: podcast search
//! (`search.getMoreResults`), artist top songs
//! (`search.artistOtherTopSongs`) and actor top songs
//! (`search.actorOtherTopSongs`).
//!
//! Reference: `src/modules/search/` (5 use-cases).
//!
//! Pagination: `p` = 0-based page, `n` = page size. Controller
//! defaults are `page=0`, `limit=10`.

use crate::client::{Context, SaavnClient, SortBy, SortOrder};
use crate::error::{Error, Result};
use crate::models::{
    ArtistMap, SearchAlbumResult, SearchAll, SearchPlaylistResult, SearchResults, Song,
};
use crate::payload::{
    artist_map_payload, as_array, search_album_payload, search_all_payload,
    search_playlist_payload, song_payload,
};

impl SaavnClient {
    /// Search everything — `__call=autocomplete.get`, param name is
    /// **`query`** (not `q`, unlike the typed searches). No pagination.
    pub async fn search_all(&self, query: &str) -> Result<SearchAll> {
        let data = self
            .fetch(
                "autocomplete.get",
                &[("query", query.to_string())],
                Context::Web,
            )
            .await?;
        if data.is_null() {
            return Err(Error::NotFound(format!("no results found for {query}")));
        }
        Ok(search_all_payload(&data))
    }

    /// Search songs — `__call=search.getResults`, `q`, `p`, `n`.
    /// Results are sliced to `limit` client-side, like the reference.
    pub async fn search_songs(
        &self,
        query: &str,
        page: u32,
        limit: u32,
    ) -> Result<SearchResults<Song>> {
        let data = self
            .fetch(
                "search.getResults",
                &[
                    ("q", query.to_string()),
                    ("p", page.to_string()),
                    ("n", limit.to_string()),
                ],
                Context::Web,
            )
            .await?;
        let mut results: Vec<Song> = as_array(&data, "results")
            .iter()
            .map(song_payload)
            .collect();
        results.truncate(limit as usize);
        Ok(SearchResults {
            total: data.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
            start: data.get("start").and_then(|v| v.as_i64()).unwrap_or(0),
            results,
        })
    }

    /// Search albums — `__call=search.getAlbumResults`, `q`, `p`, `n`.
    pub async fn search_albums(
        &self,
        query: &str,
        page: u32,
        limit: u32,
    ) -> Result<SearchResults<SearchAlbumResult>> {
        let data = self
            .fetch(
                "search.getAlbumResults",
                &[
                    ("q", query.to_string()),
                    ("p", page.to_string()),
                    ("n", limit.to_string()),
                ],
                Context::Web,
            )
            .await?;
        Ok(search_album_payload(&data))
    }

    /// Search artists — `__call=search.getArtistResults`, `q`, `p`, `n`.
    /// Results are sliced to `limit` client-side.
    pub async fn search_artists(
        &self,
        query: &str,
        page: u32,
        limit: u32,
    ) -> Result<SearchResults<ArtistMap>> {
        let data = self
            .fetch(
                "search.getArtistResults",
                &[
                    ("q", query.to_string()),
                    ("p", page.to_string()),
                    ("n", limit.to_string()),
                ],
                Context::Web,
            )
            .await?;
        if data.is_null() {
            return Err(Error::NotFound("artist not found".into()));
        }
        let mut results: Vec<ArtistMap> = as_array(&data, "results")
            .iter()
            .map(artist_map_payload)
            .collect();
        results.truncate(limit as usize);
        Ok(SearchResults {
            total: data.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
            start: data.get("start").and_then(|v| v.as_i64()).unwrap_or(0),
            results,
        })
    }

    /// Search playlists — `__call=search.getPlaylistResults`, `q`, `p`, `n`.
    pub async fn search_playlists(
        &self,
        query: &str,
        page: u32,
        limit: u32,
    ) -> Result<SearchResults<SearchPlaylistResult>> {
        let data = self
            .fetch(
                "search.getPlaylistResults",
                &[
                    ("q", query.to_string()),
                    ("p", page.to_string()),
                    ("n", limit.to_string()),
                ],
                Context::Web,
            )
            .await?;
        if data.is_null() {
            return Err(Error::NotFound("playlist not found".into()));
        }
        Ok(search_playlist_payload(&data))
    }

    /// Search podcasts — `__call=search.getMoreResults`, `query`,
    /// `p`, `n`, plus `params` = `'{"type": "podcasts"}'` (a literal
    /// JSON string, per the TS wrapper config). Returns shows as
    /// `LaunchItem`s. NOTE: this is the least-verified call in the
    /// client — one live probe with `{}` params returned `[]`;
    /// the literal-JSON form is what the config documents.
    pub async fn search_podcasts(
        &self,
        query: &str,
        page: u32,
        limit: u32,
    ) -> Result<SearchResults<crate::models::LaunchItem>> {
        let data = self
            .fetch(
                "search.getMoreResults",
                &[
                    ("query", query.to_string()),
                    ("p", page.to_string()),
                    ("n", limit.to_string()),
                    ("params", "{\"type\": \"podcasts\"}".to_string()),
                ],
                Context::Web,
            )
            .await?;
        let results: Vec<crate::models::LaunchItem> = as_array(&data, "results")
            .iter()
            .map(crate::payload::launch_item_payload)
            .collect();
        Ok(SearchResults {
            total: data.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
            start: data.get("start").and_then(|v| v.as_i64()).unwrap_or(0),
            results,
        })
    }

    /// An artist's other top songs — `__call=search.artistOtherTopSongs`.
    /// Both `artist_id` and `song_id` are required. Returns `[]` for
    /// some artists (Dua Lipa returned empty live) — that is the
    /// server's answer, not an error.
    pub async fn get_artist_top_songs(
        &self,
        artist_id: &str,
        song_id: &str,
        page: u32,
        category: Option<SortBy>,
        sort_order: Option<SortOrder>,
        language: Option<&str>,
    ) -> Result<Vec<Song>> {
        let mut params: Vec<(&str, String)> = vec![
            ("artist_ids", artist_id.to_string()),
            ("song_id", song_id.to_string()),
            ("p", page.to_string()),
        ];
        if let Some(cat) = category {
            params.push(("category", cat.as_str().to_string()));
        }
        if let Some(order) = sort_order {
            params.push(("sort_order", order.as_str().to_string()));
        }
        if let Some(lang) = language {
            params.push(("language", lang.to_string()));
        }
        let data = self
            .fetch("search.artistOtherTopSongs", &params, Context::Web)
            .await?;
        Ok(data
            .as_array()
            .map(|a| a.iter().map(song_payload).collect())
            .unwrap_or_default())
    }

    /// An actor's other top songs — `__call=search.actorOtherTopSongs`.
    pub async fn get_actor_top_songs(
        &self,
        actor_id: &str,
        song_id: &str,
        language: Option<&str>,
    ) -> Result<Vec<Song>> {
        let mut params: Vec<(&str, String)> = vec![
            ("actor_ids", actor_id.to_string()),
            ("song_id", song_id.to_string()),
        ];
        if let Some(lang) = language {
            params.push(("language", lang.to_string()));
        }
        let data = self
            .fetch("search.actorOtherTopSongs", &params, Context::Web)
            .await?;
        Ok(data
            .as_array()
            .map(|a| a.iter().map(song_payload).collect())
            .unwrap_or_default())
    }
}
