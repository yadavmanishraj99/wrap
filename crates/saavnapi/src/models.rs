//! Public (transformed) models.
//!
//! These mirror the reference's `*Model` zod schemas — the shapes a
//! caller sees after the raw `api.php` JSON has been mapped — with two
//! deliberate differences:
//! * `Song.year` is a `String` in the reference but an `Option<String>`
//!   here (the raw value is sometimes missing / numeric).
//! * Fields the reference marks required but the live API sometimes
//!   omits (e.g. `lyrics_id`, `description`) are `Option` here, so a
//!   single missing field cannot fail a whole response.

use crate::links::DownloadLink;
use serde::{Deserialize, Serialize};

/// An artist as it appears inside a song/album (`ArtistMapModel`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistMap {
    pub id: String,
    pub name: String,
    pub role: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub image: Vec<DownloadLink>,
    pub url: String,
}

/// The three artist lists a song/album carries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistGroups {
    #[serde(default)]
    pub primary: Vec<ArtistMap>,
    #[serde(default)]
    pub featured: Vec<ArtistMap>,
    #[serde(default)]
    pub all: Vec<ArtistMap>,
}

/// A song (`SongModel` in the reference).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Song {
    pub id: String,
    /// Raw `title`.
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub year: Option<String>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    pub duration: Option<u64>,
    pub label: Option<String>,
    #[serde(rename = "explicitContent")]
    pub explicit_content: bool,
    #[serde(rename = "playCount")]
    pub play_count: Option<u64>,
    pub language: String,
    #[serde(rename = "hasLyrics")]
    pub has_lyrics: bool,
    #[serde(rename = "lyricsId")]
    pub lyrics_id: Option<String>,
    /// Raw `perma_url`.
    pub url: String,
    pub copyright: Option<String>,
    pub album: SongAlbum,
    pub artists: ArtistGroups,
    pub image: Vec<DownloadLink>,
    #[serde(rename = "downloadUrl")]
    pub download_url: Vec<DownloadLink>,
    /// Raw `more_info.encrypted_media_url` — needed for
    /// `generate_auth_token` (signed URL); `download_url` is the
    /// locally decrypted unsigned URL.
    #[serde(rename = "encryptedMediaUrl")]
    pub encrypted_media_url: Option<String>,
}

/// The album stub carried on a song.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongAlbum {
    pub id: Option<String>,
    pub name: Option<String>,
    pub url: Option<String>,
}

/// An album (`AlbumModel`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub year: Option<i64>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "playCount")]
    pub play_count: Option<u64>,
    pub language: String,
    #[serde(rename = "explicitContent")]
    pub explicit_content: bool,
    pub artists: ArtistGroups,
    #[serde(rename = "songCount")]
    pub song_count: Option<u64>,
    pub url: String,
    pub image: Vec<DownloadLink>,
    pub songs: Option<Vec<Song>>,
}

/// A playlist (`PlaylistModel`).
///
/// Note: the reference overwrites the helper's `songCount` (from
/// `list_count`) with `songs.len()` after slicing to `limit` — this
/// struct carries that overwritten value, not `list_count`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub year: Option<i64>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "playCount")]
    pub play_count: Option<u64>,
    pub language: String,
    #[serde(rename = "explicitContent")]
    pub explicit_content: bool,
    #[serde(rename = "songCount")]
    pub song_count: Option<u64>,
    pub url: String,
    pub image: Vec<DownloadLink>,
    pub songs: Option<Vec<Song>>,
    pub artists: Option<Vec<ArtistMap>>,
}

/// One entry of an artist's `bio` (a JSON-encoded string in the raw API).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioEntry {
    pub text: Option<String>,
    pub title: Option<String>,
    pub sequence: Option<i64>,
}

/// A similar artist, as returned inside `Artist.similar_artists`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimilarArtist {
    pub id: String,
    pub name: String,
    pub url: String,
    pub image: Vec<DownloadLink>,
    pub languages: Option<std::collections::HashMap<String, String>>,
    pub wiki: String,
    pub dob: String,
    pub fb: String,
    pub twitter: String,
    #[serde(rename = "isRadioPresent")]
    pub is_radio_present: bool,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "dominantType")]
    pub dominant_type: String,
    pub aka: String,
    pub bio: Option<String>,
    #[serde(rename = "similarArtists")]
    pub similar_artists: Option<Vec<IdName>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdName {
    pub id: String,
    pub name: String,
}

/// An artist (`ArtistModel`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub image: Vec<DownloadLink>,
    #[serde(rename = "followerCount")]
    pub follower_count: Option<u64>,
    #[serde(rename = "fanCount")]
    pub fan_count: Option<String>,
    #[serde(rename = "isVerified")]
    pub is_verified: Option<bool>,
    #[serde(rename = "dominantLanguage")]
    pub dominant_language: Option<String>,
    #[serde(rename = "dominantType")]
    pub dominant_type: Option<String>,
    pub bio: Option<Vec<BioEntry>>,
    pub dob: Option<String>,
    pub fb: Option<String>,
    pub twitter: Option<String>,
    pub wiki: Option<String>,
    #[serde(rename = "availableLanguages", default)]
    pub available_languages: Vec<String>,
    #[serde(rename = "isRadioPresent")]
    pub is_radio_present: Option<bool>,
    #[serde(rename = "topSongs")]
    pub top_songs: Option<Vec<Song>>,
    #[serde(rename = "topAlbums")]
    pub top_albums: Option<Vec<Album>>,
    pub singles: Option<Vec<Song>>,
    #[serde(rename = "similarArtists")]
    pub similar_artists: Option<Vec<SimilarArtist>>,
}

/// `{ total, songs }` — artist-songs endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistSongs {
    pub total: u64,
    pub songs: Vec<Song>,
}

/// `{ total, albums }` — artist-albums endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistAlbums {
    pub total: u64,
    pub albums: Vec<Album>,
}

/// `{ total, start, results }` — all typed search endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults<T> {
    pub total: u64,
    pub start: i64,
    pub results: Vec<T>,
}

/// One section of `search_all` (`autocomplete.get`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchSection<T> {
    pub results: Vec<T>,
    pub position: i64,
}

/// A lightweight hit in `search_all` — songs / top-query share a shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    pub image: Vec<DownloadLink>,
    pub album: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "primaryArtists")]
    pub primary_artists: Option<String>,
    pub singers: Option<String>,
}

/// A lightweight album hit in `search_all`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchAlbumHit {
    pub id: String,
    pub title: String,
    pub image: Vec<DownloadLink>,
    pub artist: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub description: Option<String>,
    pub year: Option<String>,
    #[serde(rename = "songIds")]
    pub song_ids: Option<String>,
    pub language: Option<String>,
}

/// A lightweight artist hit in `search_all`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchArtistHit {
    pub id: String,
    pub title: String,
    pub image: Vec<DownloadLink>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub description: Option<String>,
    pub position: Option<i64>,
}

/// A lightweight playlist hit in `search_all`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchPlaylistHit {
    pub id: String,
    pub title: String,
    pub image: Vec<DownloadLink>,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
}

/// The full `search_all` result (`SearchModel`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchAll {
    #[serde(rename = "topQuery")]
    pub top_query: SearchSection<SearchHit>,
    pub songs: SearchSection<SearchHit>,
    pub albums: SearchSection<SearchAlbumHit>,
    pub artists: SearchSection<SearchArtistHit>,
    pub playlists: SearchSection<SearchPlaylistHit>,
}

/// One album hit from typed album search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchAlbumResult {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    pub year: Option<i64>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "playCount")]
    pub play_count: Option<u64>,
    pub language: String,
    #[serde(rename = "explicitContent")]
    pub explicit_content: bool,
    pub artists: ArtistGroups,
    pub image: Vec<DownloadLink>,
}

/// One item in a home / launch-data section.
///
/// Sections are heterogeneous (albums, songs, playlists, channels,
/// radio stations, shows), so this carries the fields they share and
/// keeps the rest in `more_info` (raw JSON) instead of forcing a
/// single entity type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub image: Vec<DownloadLink>,
    pub url: Option<String>,
    pub language: Option<String>,
    pub year: Option<String>,
    #[serde(rename = "playCount")]
    pub play_count: Option<u64>,
    #[serde(rename = "explicitContent")]
    pub explicit_content: bool,
    /// `more_info.song_count` (albums) — `None` for other types.
    #[serde(rename = "songCount")]
    pub song_count: Option<u64>,
    /// Top-level `count` (charts) — `None` for other types.
    pub count: Option<u64>,
    /// `more_info.artistMap`, when the item carries one.
    pub artists: ArtistGroups,
    /// The raw `more_info` object, preserved verbatim (radio station
    /// colour/description, channel info, …).
    #[serde(rename = "moreInfo")]
    pub more_info: serde_json::Value,
}

/// A section's `view_more` link inside `LaunchData.modules`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewMore {
    pub api: String,
    #[serde(rename = "pageParam")]
    pub page_param: Option<String>,
    #[serde(rename = "sizeParam")]
    pub size_param: Option<String>,
    #[serde(rename = "defaultSize")]
    pub default_size: Option<u64>,
}

/// One entry of `LaunchData.modules`: how a section is presented and
/// which API backs its "view more".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub source: Option<String>,
    pub position: i64,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    #[serde(rename = "scrollType")]
    pub scroll_type: Option<String>,
    #[serde(rename = "viewMore")]
    pub view_more: Option<ViewMore>,
}

/// The home / launch-data payload (`webapi.getLaunchData`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchData {
    pub history: Vec<LaunchItem>,
    #[serde(rename = "newTrending")]
    pub new_trending: Vec<LaunchItem>,
    #[serde(rename = "topPlaylists")]
    pub top_playlists: Vec<LaunchItem>,
    #[serde(rename = "newAlbums")]
    pub new_albums: Vec<LaunchItem>,
    #[serde(rename = "browseDiscover")]
    pub browse_discover: Vec<LaunchItem>,
    pub charts: Vec<LaunchItem>,
    pub radio: Vec<LaunchItem>,
    #[serde(rename = "artistRecos")]
    pub artist_recos: Vec<LaunchItem>,
    /// `city_mod` — only present when a `language` is passed to
    /// `get_launch_data` (hindi/tamil/bengali returned it; english
    /// and punjabi did not, 2026-11-25). Empty otherwise.
    #[serde(rename = "cityMod")]
    pub city_mod: Vec<LaunchItem>,
    /// Dynamic `promo:*` sections, keyed by their raw key
    /// (e.g. `promo:vx:data:68`).
    #[serde(rename = "promoSections")]
    pub promo_sections: std::collections::HashMap<String, Vec<LaunchItem>>,
    /// Section presentation config, keyed by section name.
    pub modules: std::collections::HashMap<String, ModuleConfig>,
    /// Raw `global_config` (`weekly_top_songs_listid`,
    /// `random_songs_listid`, … — language-keyed, kept verbatim).
    #[serde(rename = "globalConfig")]
    pub global_config: serde_json::Value,
    /// Any other top-level keys Saavn returns (e.g. `city_mod` when
    /// a language is passed), kept verbatim.
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Lyrics for one song (`lyrics.getLyrics`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lyrics {
    /// The lyrics text, lines separated by `<br>` (`<br><br>` = blank line).
    pub text: String,
    pub copyright: Option<String>,
    pub snippet: Option<String>,
    #[serde(rename = "scriptTrackingUrl")]
    pub script_tracking_url: Option<String>,
}

/// A signed streaming URL (`song.generateAuthToken`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthToken {
    /// Signed CloudFront URL on `web.saavncdn.com` — short-lived
    /// (carries `Expires`/`Signature`/`Key-Pair-Id`).
    #[serde(rename = "authUrl")]
    pub auth_url: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "jweToken")]
    pub jwe_token: Option<String>,
}

/// A paginated discovery list — the `{data, count, last_page}`
/// envelope used by `content.getAlbums`, `content.getFeaturedPlaylists`,
/// `content.getTopShows` (and others).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    /// Total items available (`count` on the wire).
    pub count: u64,
    #[serde(rename = "lastPage")]
    pub last_page: bool,
    #[serde(rename = "data")]
    pub items: Vec<T>,
}

/// One top artist (`social.getTopArtists`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopArtist {
    pub id: String,
    pub name: String,
    pub image: Vec<DownloadLink>,
    pub url: Option<String>,
    #[serde(rename = "followerCount")]
    pub follower_count: Option<u64>,
    #[serde(rename = "isFollowed")]
    pub is_followed: Option<bool>,
}

/// Top shows (`content.getTopShows`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopShows {
    pub count: u64,
    #[serde(rename = "lastPage")]
    pub last_page: bool,
    pub shows: Vec<LaunchItem>,
    /// Flattened items of the `trendingPodcasts` modules.
    #[serde(rename = "trendingPodcasts")]
    pub trending_podcasts: Vec<LaunchItem>,
    #[serde(rename = "trendingTitle")]
    pub trending_title: Option<String>,
    #[serde(rename = "trendingSubtitle")]
    pub trending_subtitle: Option<String>,
}

/// One season of a show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowSeason {
    pub id: String,
    pub title: String,
}

/// One podcast/show episode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: Option<String>,
    pub image: Vec<DownloadLink>,
    pub language: Option<String>,
    pub year: Option<String>,
    #[serde(rename = "playCount")]
    pub play_count: Option<u64>,
    #[serde(rename = "explicitContent")]
    pub explicit_content: bool,
    pub duration: Option<u64>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(rename = "seasonNo")]
    pub season_no: Option<u64>,
    #[serde(rename = "episodeNumber")]
    pub episode_number: Option<u64>,
    #[serde(rename = "showId")]
    pub show_id: Option<String>,
    #[serde(rename = "seasonId")]
    pub season_id: Option<String>,
    #[serde(rename = "showTitle")]
    pub show_title: Option<String>,
    #[serde(rename = "downloadUrl")]
    pub download_url: Vec<DownloadLink>,
    pub artists: ArtistGroups,
    /// Raw `more_info`, preserved verbatim.
    #[serde(rename = "moreInfo")]
    pub more_info: serde_json::Value,
}

/// A show / podcast (`webapi.get`, `type=show`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    #[serde(rename = "headerDesc")]
    pub header_desc: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: Option<String>,
    pub image: Vec<DownloadLink>,
    pub language: Option<String>,
    pub year: Option<String>,
    #[serde(rename = "playCount")]
    pub play_count: Option<u64>,
    #[serde(rename = "explicitContent")]
    pub explicit_content: bool,
    /// Raw `more_info` of `show_details` — note it is a
    /// **stringified Python-style dict** (single quotes) on the
    /// wire, not a JSON object; kept verbatim as a string/value.
    #[serde(rename = "moreInfo")]
    pub more_info: serde_json::Value,
    pub seasons: Vec<ShowSeason>,
    pub episodes: Vec<Episode>,
    /// Raw `modules` object, preserved verbatim.
    pub modules: serde_json::Value,
}

/// One footer link (`webapi.getFooterDetails`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FooterLink {
    pub id: String,
    pub title: String,
    pub action: Option<String>,
}

/// Footer details (`webapi.getFooterDetails`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FooterDetails {
    pub playlists: Vec<FooterLink>,
    pub artists: Vec<FooterLink>,
    pub albums: Vec<FooterLink>,
    pub actors: Vec<FooterLink>,
}

/// One mega-menu item (`webapi.getBrowseHoverDetails`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MegaMenuItem {
    pub name: String,
    pub url: String,
}

/// Mega menu (`webapi.getBrowseHoverDetails`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MegaMenu {
    #[serde(rename = "topArtists")]
    pub top_artists: Vec<MegaMenuItem>,
    #[serde(rename = "topPlaylists")]
    pub top_playlists: Vec<MegaMenuItem>,
    #[serde(rename = "newReleases")]
    pub new_releases: Vec<MegaMenuItem>,
}

/// A mix (`webapi.get`, `type=mix`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mix {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    #[serde(rename = "headerDesc")]
    pub header_desc: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: Option<String>,
    pub image: Vec<DownloadLink>,
    pub language: Option<String>,
    pub year: Option<String>,
    #[serde(rename = "playCount")]
    pub play_count: Option<u64>,
    #[serde(rename = "explicitContent")]
    pub explicit_content: bool,
    #[serde(rename = "listCount")]
    pub list_count: Option<u64>,
    pub songs: Vec<Song>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
    pub username: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    #[serde(rename = "isFollowed")]
    pub is_followed: Option<bool>,
}

/// A label (`webapi.get`, `type=label`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: Option<String>,
    pub image: Vec<DownloadLink>,
    #[serde(rename = "topSongs")]
    pub top_songs: Vec<Song>,
    #[serde(rename = "topSongsTotal")]
    pub top_songs_total: Option<u64>,
    #[serde(rename = "topAlbums")]
    pub top_albums: Vec<Album>,
    #[serde(rename = "topAlbumsTotal")]
    pub top_albums_total: Option<u64>,
    #[serde(rename = "availableLanguages")]
    pub available_languages: Vec<String>,
}

/// One playlist hit from typed playlist search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchPlaylistResult {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub image: Vec<DownloadLink>,
    pub url: String,
    #[serde(rename = "songCount")]
    pub song_count: Option<u64>,
    pub language: String,
    #[serde(rename = "explicitContent")]
    pub explicit_content: bool,
}
