//! Discovery lists — charts, albums, featured playlists, top shows,
//! top artists, top searches, featured stations, top albums of the year.
//!
//! All live-verified 2026-11-25 (see `ENDPOINT_INVENTORY.md`).
//! Two envelope shapes exist: bare arrays (charts, top searches,
//! featured stations, top albums of the year) and
//! `{data, count, last_page}` (albums, featured playlists, top shows).
//! Most items map onto the existing [`LaunchItem`] shape.

use crate::client::{Context, SaavnClient};
use crate::error::Result;
use crate::models::{Album, LaunchItem, Page, TopArtist, TopShows};
use crate::payload::{
    album_payload, launch_items, page_payload, playlist_payload, top_artist_payload,
    top_shows_payload,
};

fn page_params(page: u32, limit: u32) -> [(&'static str, String); 2] {
    [("p", page.to_string()), ("n", limit.to_string())]
}

impl SaavnClient {
    /// Top charts — `__call=content.getCharts`. Call with no params:
    /// `p`/`n` behave oddly here (`p=0` truncates to 5, anything else
    /// returns all 9). This is the launch-data `charts` module's
    /// view-more target.
    pub async fn get_charts(&self) -> Result<Vec<LaunchItem>> {
        let data = self.fetch("content.getCharts", &[], Context::Web).await?;
        Ok(data.as_array().map(|a| launch_items(a)).unwrap_or_default())
    }

    /// New albums feed — `__call=content.getAlbums`, `p`, `n`,
    /// optional `language` (filters). Despite the name the items are
    /// mixed songs + albums. Default page size on the server is 31.
    pub async fn get_albums(
        &self,
        page: u32,
        limit: u32,
        language: Option<&str>,
    ) -> Result<Page<LaunchItem>> {
        let mut params: Vec<(&str, String)> = page_params(page, limit).into_iter().collect();
        if let Some(lang) = language {
            params.push(("language", lang.to_string()));
        }
        let data = self
            .fetch("content.getAlbums", &params, Context::Web)
            .await?;
        Ok(page_payload(&data, |x| {
            crate::payload::launch_item_payload(x)
        }))
    }

    /// Featured playlists — `__call=content.getFeaturedPlaylists`,
    /// `p`, `n`, optional `languages` (filters). Real pagination:
    /// page 0 and page 1 have zero overlap. This is the launch-data
    /// `top_playlists` module's view-more target.
    pub async fn get_featured_playlists(
        &self,
        page: u32,
        limit: u32,
        language: Option<&str>,
    ) -> Result<Page<crate::models::Playlist>> {
        let mut params: Vec<(&str, String)> = page_params(page, limit).into_iter().collect();
        if let Some(lang) = language {
            params.push(("languages", lang.to_string()));
        }
        let data = self
            .fetch("content.getFeaturedPlaylists", &params, Context::Web)
            .await?;
        Ok(page_payload(&data, playlist_payload))
    }

    /// Top shows / podcasts — `__call=content.getTopShows`, `p`, `n`.
    pub async fn get_top_shows(&self, page: u32, limit: u32) -> Result<TopShows> {
        let params = page_params(page, limit);
        let data = self
            .fetch("content.getTopShows", &params, Context::Web)
            .await?;
        Ok(top_shows_payload(&data))
    }

    /// Top artists — `__call=social.getTopArtists`. Returns the
    /// `top_artists` list (30 items); `following_artists` is not
    /// exposed (it is empty for anonymous callers).
    pub async fn get_top_artists(&self) -> Result<Vec<TopArtist>> {
        let data = self
            .fetch("social.getTopArtists", &[], Context::Web)
            .await?;
        Ok(data
            .get("top_artists")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(top_artist_payload).collect())
            .unwrap_or_default())
    }

    /// Top searches — `__call=content.getTopSearches`. Fixed top 10,
    /// mixed song/album/artist; `language` re-ranks, it does not filter.
    pub async fn get_top_searches(&self, language: Option<&str>) -> Result<Vec<LaunchItem>> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(lang) = language {
            params.push(("language", lang.to_string()));
        }
        let data = self
            .fetch("content.getTopSearches", &params, Context::Web)
            .await?;
        Ok(data.as_array().map(|a| launch_items(a)).unwrap_or_default())
    }

    /// Featured radio stations — `__call=webradio.getFeaturedStations`.
    /// Always 28 items; `p`/`n` are ignored; `language` re-selects the
    /// set. This is the launch-data `radio` module's view-more target.
    pub async fn get_featured_stations(&self, language: Option<&str>) -> Result<Vec<LaunchItem>> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(lang) = language {
            params.push(("language", lang.to_string()));
        }
        let data = self
            .fetch("webradio.getFeaturedStations", &params, Context::Web)
            .await?;
        Ok(data.as_array().map(|a| launch_items(a)).unwrap_or_default())
    }

    /// Top albums of a year — `__call=search.topAlbumsoftheYear`.
    /// The year parameter is named **`album_year`** (not `year` —
    /// `year=` errors `INPUT_MISSING`); `album_lang` is optional.
    /// Returns 24 albums, each with its songs embedded.
    pub async fn get_top_albums_of_year(
        &self,
        album_year: u32,
        album_lang: Option<&str>,
    ) -> Result<Vec<Album>> {
        let mut params: Vec<(&str, String)> = vec![("album_year", album_year.to_string())];
        if let Some(lang) = album_lang {
            params.push(("album_lang", lang.to_string()));
        }
        let data = self
            .fetch("search.topAlbumsoftheYear", &params, Context::Web)
            .await?;
        Ok(data
            .as_array()
            .map(|a| a.iter().map(album_payload).collect())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn page_payload_maps_envelope() {
        let raw = json!({"data": [
            {"id": "1", "title": "Punjab Se Bollywood", "type": "playlist",
             "perma_url": "u", "image": "https://c.saavncdn.com/x-150x150.jpg",
             "more_info": {"song_count": "22"}}],
            "count": 30, "last_page": false});
        let page = page_payload(&raw, playlist_payload);
        assert_eq!(page.count, 30);
        assert!(!page.last_page);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "Punjab Se Bollywood");
    }

    #[test]
    fn top_shows_payload_flattens_trending_podcasts() {
        let raw = json!({"data": [
            {"id": "1", "title": "Gita For Daily Living", "type": "show",
             "perma_url": "u", "image": "https://c.sop.saavncdn.com/x-500x500.jpg"}],
            "last_page": false,
            "trendingPodcasts": [{"items": [
                {"id": "63565", "title": "Shri Krishna Amritvani", "type": "show",
                 "perma_url": "u", "image": "https://c.sop.saavncdn.com/x-500x500.jpg",
                 "more_info": []}],
                "module": {"title": "Trending Podcasts", "subtitle": "Most heard in India"}}]});
        let shows = top_shows_payload(&raw);
        assert_eq!(shows.shows.len(), 1);
        assert_eq!(shows.trending_podcasts.len(), 1);
        assert_eq!(shows.trending_title.as_deref(), Some("Trending Podcasts"));
    }

    #[test]
    fn top_artist_payload_uses_artistid() {
        let raw = json!({"artistid": "459320", "name": "Arijit Singh",
            "image": "https://c.saavncdn.com/artists/x-150x150.jpg",
            "follower_count": 109903579, "is_followed": false,
            "perma_url": "https://www.jiosaavn.com/artist/arijit-singh-songs/LlRWpHzy3Hk_"});
        let artist = top_artist_payload(&raw);
        assert_eq!(artist.id, "459320");
        assert_eq!(artist.follower_count, Some(109_903_579));
        assert_eq!(artist.is_followed, Some(false));
    }
}
