//! Raw `api.php` JSON (`serde_json::Value`) → public models.
//!
//! This is the Rust equivalent of the reference's `create*Payload`
//! helpers. Two rules, learned from the live API (2026-11-25):
//! 1. **Be lenient.** The reference's zod schemas mark many fields
//!    required, but the live API omits some (`lyrics_id` was absent on
//!    the `3IoDK8qI` test song) and returns numbers where the schema
//!    says string (`year`, `play_count`). Every accessor below accepts
//!    string / number / null and defaults to empty / `None`.
//! 2. **Copy the reference's conversions exactly** where they matter:
//!    `explicit_content == "1"`, `has_lyrics == "true"`,
//!    `Number(duration)` / `Number(play_count)` / `Number(year)`,
//!    `bio` / `languages` / `similar` are JSON-encoded *strings*.

use serde_json::Value;

use crate::links::{create_download_links, create_image_links};
use crate::models::*;

/// String-or-number → String. `""` when missing/null.
pub(crate) fn s(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// String-or-number → Option<String>. `None` when missing, null or empty.
pub(crate) fn opt_s(v: &Value, key: &str) -> Option<String> {
    match v.get(key) {
        Some(Value::String(x)) if !x.is_empty() => Some(x.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

/// String-or-number → Option<u64> (`Number(...)` in the reference).
pub(crate) fn opt_u64(v: &Value, key: &str) -> Option<u64> {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(x)) => x.trim().parse().ok(),
        _ => None,
    }
}

pub(crate) fn opt_i64(v: &Value, key: &str) -> Option<i64> {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(x)) => x.trim().parse().ok(),
        _ => None,
    }
}

pub(crate) fn as_array(v: &Value, key: &str) -> Vec<Value> {
    match v.get(key) {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    }
}

fn parse_json_string<T>(raw: Option<&str>) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let raw = raw?;
    serde_json::from_str(raw).ok()
}

pub(crate) fn artist_map_payload(v: &Value) -> ArtistMap {
    ArtistMap {
        id: s(v, "id"),
        name: s(v, "name"),
        role: s(v, "role"),
        kind: s(v, "type"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        url: s(v, "perma_url"),
    }
}

fn artist_groups(more_info: &Value) -> ArtistGroups {
    let map = more_info.get("artistMap").cloned().unwrap_or(Value::Null);
    ArtistGroups {
        primary: as_array(&map, "primary_artists")
            .iter()
            .map(artist_map_payload)
            .collect(),
        featured: as_array(&map, "featured_artists")
            .iter()
            .map(artist_map_payload)
            .collect(),
        all: as_array(&map, "artists")
            .iter()
            .map(artist_map_payload)
            .collect(),
    }
}

/// `createSongPayload` — `src/modules/songs/helpers/song.helper.ts`.
pub(crate) fn song_payload(v: &Value) -> Song {
    let more = v.get("more_info").cloned().unwrap_or(Value::Null);
    Song {
        id: s(v, "id"),
        name: s(v, "title"),
        kind: s(v, "type"),
        year: opt_s(v, "year"),
        release_date: opt_s(&more, "release_date"),
        duration: opt_u64(&more, "duration"),
        label: opt_s(&more, "label"),
        explicit_content: s(v, "explicit_content") == "1",
        play_count: opt_u64(v, "play_count"),
        language: s(v, "language"),
        has_lyrics: s(&more, "has_lyrics") == "true",
        lyrics_id: opt_s(&more, "lyrics_id"),
        url: s(v, "perma_url"),
        copyright: opt_s(&more, "copyright_text"),
        album: SongAlbum {
            id: opt_s(&more, "album_id"),
            name: opt_s(&more, "album"),
            url: opt_s(&more, "album_url"),
        },
        artists: artist_groups(&more),
        image: create_image_links(opt_s(v, "image").as_deref()),
        download_url: create_download_links(opt_s(&more, "encrypted_media_url").as_deref()),
        encrypted_media_url: opt_s(&more, "encrypted_media_url"),
    }
}

/// `createAlbumPayload` — `src/modules/albums/helpers/album.helper.ts`.
pub(crate) fn album_payload(v: &Value) -> Album {
    let more = v.get("more_info").cloned().unwrap_or(Value::Null);
    Album {
        id: s(v, "id"),
        name: s(v, "title"),
        description: opt_s(v, "header_desc"),
        year: opt_i64(v, "year"),
        kind: s(v, "type"),
        play_count: opt_u64(v, "play_count"),
        language: s(v, "language"),
        explicit_content: s(v, "explicit_content") == "1",
        artists: artist_groups(&more),
        song_count: opt_u64(&more, "song_count"),
        url: s(v, "perma_url"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        songs: match v.get("list") {
            Some(Value::Array(list)) => Some(list.iter().map(song_payload).collect()),
            _ => None,
        },
    }
}

/// `createPlaylistPayload` — `src/modules/playlists/helpers/playlist.helper.ts`.
pub(crate) fn playlist_payload(v: &Value) -> Playlist {
    let more = v.get("more_info").cloned().unwrap_or(Value::Null);
    Playlist {
        id: s(v, "id"),
        name: s(v, "title"),
        description: opt_s(v, "header_desc").or_else(|| opt_s(v, "description")),
        year: opt_i64(v, "year"),
        kind: s(v, "type"),
        play_count: opt_u64(v, "play_count"),
        language: s(v, "language"),
        explicit_content: s(v, "explicit_content") == "1",
        // NOTE: callers (playlist by id/link) overwrite this with
        // `songs.len()` after slicing, exactly like the reference.
        song_count: opt_u64(v, "list_count"),
        url: s(v, "perma_url"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        songs: match v.get("list") {
            Some(Value::Array(list)) => Some(list.iter().map(song_payload).collect()),
            _ => None,
        },
        artists: match more.get("artists") {
            Some(Value::Array(a)) => Some(a.iter().map(artist_map_payload).collect()),
            _ => None,
        },
    }
}

/// `createArtistPayload` — `src/modules/artists/helpers/artist.helper.ts`.
pub(crate) fn artist_payload(v: &Value) -> Artist {
    let urls = v.get("urls").cloned().unwrap_or(Value::Null);
    Artist {
        id: opt_s(v, "artistId").unwrap_or_else(|| s(v, "id")),
        name: s(v, "name"),
        url: opt_s(&urls, "overview").unwrap_or_else(|| s(v, "perma_url")),
        kind: s(v, "type"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        follower_count: opt_u64(v, "follower_count"),
        fan_count: opt_s(v, "fan_count"),
        is_verified: v.get("isVerified").and_then(Value::as_bool),
        dominant_language: opt_s(v, "dominantLanguage"),
        dominant_type: opt_s(v, "dominantType"),
        bio: parse_json_string(opt_s(v, "bio").as_deref()),
        dob: opt_s(v, "dob"),
        fb: opt_s(v, "fb"),
        twitter: opt_s(v, "twitter"),
        wiki: opt_s(v, "wiki"),
        available_languages: as_array(v, "availableLanguages")
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
        is_radio_present: v.get("isRadioPresent").and_then(Value::as_bool),
        top_songs: match v.get("topSongs") {
            Some(Value::Array(a)) => Some(a.iter().map(song_payload).collect()),
            _ => None,
        },
        top_albums: match v.get("topAlbums") {
            Some(Value::Array(a)) => Some(a.iter().map(album_payload).collect()),
            _ => None,
        },
        singles: match v.get("singles") {
            Some(Value::Array(a)) => Some(a.iter().map(song_payload).collect()),
            _ => None,
        },
        similar_artists: match v.get("similarArtists") {
            Some(Value::Array(a)) => Some(
                a.iter()
                    .map(|x| SimilarArtist {
                        id: s(x, "id"),
                        name: s(x, "name"),
                        url: s(x, "perma_url"),
                        image: create_image_links(opt_s(x, "image_url").as_deref()),
                        languages: parse_json_string(opt_s(x, "languages").as_deref()),
                        wiki: s(x, "wiki"),
                        dob: s(x, "dob"),
                        fb: s(x, "fb"),
                        twitter: s(x, "twitter"),
                        is_radio_present: x
                            .get("isRadioPresent")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        kind: s(x, "type"),
                        dominant_type: s(x, "dominantType"),
                        aka: s(x, "aka"),
                        bio: match parse_json_string::<Value>(opt_s(x, "bio").as_deref()) {
                            Some(Value::String(t)) => Some(t),
                            Some(other) => Some(other.to_string()),
                            None => opt_s(x, "bio"),
                        },
                        similar_artists: parse_json_string(opt_s(x, "similar").as_deref()),
                    })
                    .collect(),
            ),
            _ => None,
        },
    }
}

// --- search helpers -------------------------------------------------------

fn section<T, F>(v: &Value, key: &str, map: F) -> SearchSection<T>
where
    F: Fn(&Value) -> T,
{
    let sec = v.get(key).cloned().unwrap_or(Value::Null);
    SearchSection {
        results: as_array(&sec, "data").iter().map(map).collect(),
        position: sec.get("position").and_then(Value::as_i64).unwrap_or(0),
    }
}

fn search_hit(v: &Value) -> SearchHit {
    let more = v.get("more_info").cloned().unwrap_or(Value::Null);
    SearchHit {
        id: s(v, "id"),
        title: s(v, "title"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        album: opt_s(&more, "album"),
        url: opt_s(v, "perma_url"),
        kind: opt_s(v, "type"),
        language: opt_s(&more, "language"),
        description: opt_s(v, "description"),
        primary_artists: opt_s(&more, "primary_artists"),
        singers: opt_s(&more, "singers"),
    }
}

/// `createSearchPayload` — `src/modules/search/helpers/search.helper.ts`.
pub(crate) fn search_all_payload(v: &Value) -> SearchAll {
    SearchAll {
        top_query: section(v, "topquery", search_hit),
        songs: section(v, "songs", search_hit),
        albums: section(v, "albums", |x| {
            let more = x.get("more_info").cloned().unwrap_or(Value::Null);
            SearchAlbumHit {
                id: s(x, "id"),
                title: s(x, "title"),
                image: create_image_links(opt_s(x, "image").as_deref()),
                artist: opt_s(&more, "music"),
                url: opt_s(x, "perma_url"),
                kind: opt_s(x, "type"),
                description: opt_s(x, "description"),
                year: opt_s(&more, "year"),
                song_ids: opt_s(&more, "song_pids"),
                language: opt_s(&more, "language"),
            }
        }),
        artists: section(v, "artists", |x| SearchArtistHit {
            id: s(x, "id"),
            title: s(x, "title"),
            image: create_image_links(opt_s(x, "image").as_deref()),
            kind: opt_s(x, "type"),
            description: opt_s(x, "description"),
            position: x.get("position").and_then(Value::as_i64),
        }),
        playlists: section(v, "playlists", |x| {
            let more = x.get("more_info").cloned().unwrap_or(Value::Null);
            SearchPlaylistHit {
                id: s(x, "id"),
                title: s(x, "title"),
                image: create_image_links(opt_s(x, "image").as_deref()),
                url: opt_s(x, "perma_url"),
                kind: opt_s(x, "type"),
                language: opt_s(&more, "language"),
                description: opt_s(x, "description"),
            }
        }),
    }
}

/// `createSearchAlbumPayload`.
pub(crate) fn search_album_payload(v: &Value) -> SearchResults<SearchAlbumResult> {
    SearchResults {
        total: v.get("total").and_then(Value::as_u64).unwrap_or(0),
        start: v.get("start").and_then(Value::as_i64).unwrap_or(0),
        results: as_array(v, "results")
            .iter()
            .map(|x| {
                let more = x.get("more_info").cloned().unwrap_or(Value::Null);
                SearchAlbumResult {
                    id: s(x, "id"),
                    name: s(x, "title"),
                    description: opt_s(x, "header_desc"),
                    url: s(x, "perma_url"),
                    year: opt_i64(x, "year"),
                    kind: s(x, "type"),
                    play_count: opt_u64(x, "play_count"),
                    language: s(x, "language"),
                    explicit_content: s(x, "explicit_content") == "1",
                    artists: artist_groups(&more),
                    image: create_image_links(opt_s(x, "image").as_deref()),
                }
            })
            .collect(),
    }
}

/// `createSearchPlaylistPayload`.
pub(crate) fn search_playlist_payload(v: &Value) -> SearchResults<SearchPlaylistResult> {
    SearchResults {
        total: v.get("total").and_then(Value::as_u64).unwrap_or(0),
        start: v.get("start").and_then(Value::as_i64).unwrap_or(0),
        results: as_array(v, "results")
            .iter()
            .map(|x| {
                let more = x.get("more_info").cloned().unwrap_or(Value::Null);
                SearchPlaylistResult {
                    id: s(x, "id"),
                    name: s(x, "title"),
                    kind: s(x, "type"),
                    image: create_image_links(opt_s(x, "image").as_deref()),
                    url: s(x, "perma_url"),
                    song_count: opt_u64(&more, "song_count"),
                    language: s(&more, "language"),
                    explicit_content: s(x, "explicit_content") == "1",
                }
            })
            .collect(),
    }
}

/// One home / launch-data item — common fields + raw `more_info`.
pub(crate) fn launch_item_payload(v: &Value) -> LaunchItem {
    let more = v.get("more_info").cloned().unwrap_or(Value::Null);
    LaunchItem {
        id: s(v, "id"),
        title: s(v, "title"),
        subtitle: opt_s(v, "subtitle"),
        kind: s(v, "type"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        url: opt_s(v, "perma_url"),
        language: opt_s(v, "language").or_else(|| opt_s(&more, "language")),
        year: opt_s(v, "year"),
        play_count: opt_u64(v, "play_count"),
        explicit_content: s(v, "explicit_content") == "1",
        song_count: opt_u64(&more, "song_count"),
        // `count` is a JSON number on charts, but accept the string
        // form too — same lenient rule as every other number here.
        count: opt_u64(v, "count"),
        artists: artist_groups(&more),
        more_info: more,
    }
}

/// Map raw items to [`LaunchItem`]s, dropping entries that parsed to
/// nothing at all (empty id *and* empty title). Junk array elements —
/// `null`, numbers, strings — must not become phantom items in a
/// section; real Saavn items always carry an id and a title.
pub(crate) fn launch_items(values: &[Value]) -> Vec<LaunchItem> {
    values
        .iter()
        .map(launch_item_payload)
        .filter(|item| !(item.id.is_empty() && item.title.is_empty()))
        .collect()
}

/// `lyrics.getLyrics` — `None` when the response carries no `lyrics`
/// (the server signals "no lyrics" as `{"status":"failure",...}`).
pub(crate) fn lyrics_payload(v: &Value) -> Option<Lyrics> {
    let text = opt_s(v, "lyrics")?;
    Some(Lyrics {
        text,
        copyright: opt_s(v, "lyrics_copyright"),
        snippet: opt_s(v, "snippet"),
        script_tracking_url: opt_s(v, "script_tracking_url"),
    })
}

/// `song.generateAuthToken` — `None` when `status` is not `success`.
pub(crate) fn auth_token_payload(v: &Value) -> Option<AuthToken> {
    if s(v, "status") != "success" {
        return None;
    }
    Some(AuthToken {
        auth_url: s(v, "auth_url"),
        kind: s(v, "type"),
        jwe_token: opt_s(v, "jwe_token"),
    })
}

/// The `{data, count, last_page}` envelope.
pub(crate) fn page_payload<T, F>(v: &Value, map: F) -> Page<T>
where
    F: Fn(&Value) -> T,
{
    Page {
        count: opt_u64(v, "count").unwrap_or(0),
        last_page: v.get("last_page").and_then(Value::as_bool).unwrap_or(false),
        items: as_array(v, "data").iter().map(map).collect(),
    }
}

/// One `social.getTopArtists` item — note the id key is `artistid`.
pub(crate) fn top_artist_payload(v: &Value) -> TopArtist {
    TopArtist {
        id: opt_s(v, "artistid").unwrap_or_else(|| s(v, "id")),
        name: s(v, "name"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        url: opt_s(v, "perma_url"),
        follower_count: opt_u64(v, "follower_count"),
        is_followed: v.get("is_followed").and_then(Value::as_bool),
    }
}

/// `content.getTopShows` — `trendingPodcasts` is a list of modules,
/// each `{items: [...], module: {title, subtitle}}`.
pub(crate) fn top_shows_payload(v: &Value) -> TopShows {
    let mut trending: Vec<LaunchItem> = Vec::new();
    let mut title = None;
    let mut subtitle = None;
    for module in as_array(v, "trendingPodcasts") {
        if title.is_none() {
            if let Some(m) = module.get("module") {
                title = opt_s(m, "title");
                subtitle = opt_s(m, "subtitle");
            }
        }
        trending.extend(launch_items(&as_array(&module, "items")));
    }
    TopShows {
        count: opt_u64(v, "count").unwrap_or(0),
        last_page: v.get("last_page").and_then(Value::as_bool).unwrap_or(false),
        shows: launch_items(&as_array(v, "data")),
        trending_podcasts: trending,
        trending_title: title,
        trending_subtitle: subtitle,
    }
}

/// One show episode.
pub(crate) fn episode_payload(v: &Value) -> Episode {
    let more = v.get("more_info").cloned().unwrap_or(Value::Null);
    Episode {
        id: s(v, "id"),
        title: s(v, "title"),
        subtitle: opt_s(v, "subtitle"),
        kind: s(v, "type"),
        url: opt_s(v, "perma_url"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        language: opt_s(v, "language"),
        year: opt_s(v, "year"),
        play_count: opt_u64(v, "play_count"),
        explicit_content: s(v, "explicit_content") == "1",
        duration: opt_u64(&more, "duration"),
        release_date: opt_s(&more, "release_date").or_else(|| opt_s(&more, "release_time")),
        season_no: opt_u64(&more, "season_no"),
        episode_number: opt_u64(&more, "episode_number"),
        show_id: opt_s(&more, "show_id"),
        season_id: opt_s(&more, "season_id"),
        show_title: opt_s(&more, "show_title"),
        download_url: create_download_links(opt_s(&more, "encrypted_media_url").as_deref()),
        artists: artist_groups(&more),
        more_info: more,
    }
}

/// A show (`webapi.get`, `type=show`) — `{show_details, seasons,
/// episodes, modules}`.
pub(crate) fn show_payload(v: &Value) -> Show {
    let details = v.get("show_details").cloned().unwrap_or(Value::Null);
    Show {
        id: s(&details, "id"),
        title: s(&details, "title"),
        subtitle: opt_s(&details, "subtitle"),
        header_desc: opt_s(&details, "header_desc"),
        kind: s(&details, "type"),
        url: opt_s(&details, "perma_url"),
        image: create_image_links(opt_s(&details, "image").as_deref()),
        language: opt_s(&details, "language"),
        year: opt_s(&details, "year"),
        play_count: opt_u64(&details, "play_count"),
        explicit_content: s(&details, "explicit_content") == "1",
        more_info: details.get("more_info").cloned().unwrap_or(Value::Null),
        seasons: as_array(v, "seasons")
            .iter()
            .map(|x| ShowSeason {
                id: s(x, "id"),
                title: s(x, "title"),
            })
            .collect(),
        episodes: as_array(v, "episodes")
            .iter()
            .map(episode_payload)
            .collect(),
        modules: v.get("modules").cloned().unwrap_or(Value::Null),
    }
}

/// `webapi.getFooterDetails` — sections of `{id, title, action}`.
pub(crate) fn footer_payload(v: &Value) -> FooterDetails {
    let links = |key: &str| -> Vec<FooterLink> {
        as_array(v, key)
            .iter()
            .map(|x| FooterLink {
                id: s(x, "id"),
                title: s(x, "title"),
                action: opt_s(x, "action"),
            })
            .collect()
    };
    FooterDetails {
        playlists: links("playlist"),
        artists: links("artist"),
        albums: links("album"),
        actors: links("actor"),
    }
}

/// `webapi.getBrowseHoverDetails` — accepts either the bare object
/// or the `{mega_menu: {...}}` wrapper the live API returns.
pub(crate) fn mega_menu_payload(v: &Value) -> MegaMenu {
    let inner = v.get("mega_menu").unwrap_or(v);
    let items = |key: &str| -> Vec<MegaMenuItem> {
        as_array(inner, key)
            .iter()
            .map(|x| MegaMenuItem {
                name: s(x, "name"),
                url: s(x, "url"),
            })
            .collect()
    };
    MegaMenu {
        top_artists: items("top_artists"),
        top_playlists: items("top_playlists"),
        new_releases: items("new_releases"),
    }
}

/// A mix (`webapi.get`, `type=mix`).
pub(crate) fn mix_payload(v: &Value) -> Mix {
    let more = v.get("more_info").cloned().unwrap_or(Value::Null);
    Mix {
        id: s(v, "id"),
        title: s(v, "title"),
        subtitle: opt_s(v, "subtitle"),
        header_desc: opt_s(v, "header_desc"),
        kind: s(v, "type"),
        url: opt_s(v, "perma_url"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        language: opt_s(v, "language"),
        year: opt_s(v, "year"),
        play_count: opt_u64(v, "play_count"),
        explicit_content: s(v, "explicit_content") == "1",
        list_count: opt_u64(v, "list_count"),
        songs: match v.get("list") {
            Some(Value::Array(list)) => list.iter().map(song_payload).collect(),
            _ => Vec::new(),
        },
        user_id: opt_s(&more, "uid"),
        last_updated: opt_s(&more, "last_updated"),
        username: opt_s(&more, "username"),
        firstname: opt_s(&more, "firstname"),
        lastname: opt_s(&more, "lastname"),
        is_followed: v.get("is_followed").and_then(Value::as_bool),
    }
}

/// A label (`webapi.get`, `type=label`).
pub(crate) fn label_payload(v: &Value) -> Label {
    let songs_section = v.get("top_songs").cloned().unwrap_or(Value::Null);
    let albums_section = v.get("top_albums").cloned().unwrap_or(Value::Null);
    let list_of = |section: &Value, key: &str, map: fn(&Value) -> Song| -> Vec<Song> {
        match section.get(key) {
            Some(Value::Array(a)) => a.iter().map(map).collect(),
            _ => match section {
                Value::Array(a) => a.iter().map(map).collect(),
                _ => Vec::new(),
            },
        }
    };
    let albums_of = |section: &Value| -> Vec<Album> {
        match section.get("albums") {
            Some(Value::Array(a)) => a.iter().map(album_payload).collect(),
            _ => match section {
                Value::Array(a) => a.iter().map(album_payload).collect(),
                _ => Vec::new(),
            },
        }
    };
    Label {
        id: opt_s(v, "labelId").unwrap_or_else(|| s(v, "id")),
        name: s(v, "name"),
        kind: s(v, "type"),
        url: opt_s(v, "perma_url"),
        image: create_image_links(opt_s(v, "image").as_deref()),
        top_songs: list_of(&songs_section, "songs", song_payload),
        top_songs_total: opt_u64(&songs_section, "total"),
        top_albums: albums_of(&albums_section),
        top_albums_total: opt_u64(&albums_section, "total"),
        available_languages: as_array(v, "availableLanguages")
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
    }
}

/// One `modules` entry in launch data.
pub(crate) fn module_config_payload(v: &Value) -> ModuleConfig {
    let view_more = match v.get("view_more") {
        Some(vm) if vm.is_object() => Some(ViewMore {
            api: s(vm, "api"),
            page_param: opt_s(vm, "page_param"),
            size_param: opt_s(vm, "size_param"),
            default_size: opt_u64(vm, "default_size"),
        }),
        _ => None,
    };
    ModuleConfig {
        source: opt_s(v, "source"),
        position: opt_i64(v, "position").unwrap_or(0),
        title: opt_s(v, "title"),
        subtitle: opt_s(v, "subtitle"),
        scroll_type: opt_s(v, "scroll_type"),
        view_more,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn song_payload_maps_live_shape() {
        // Shape taken from the live `song.getDetails` response for 3IoDK8qI.
        let raw = json!({
            "id": "3IoDK8qI", "title": "Levitating", "type": "song",
            "year": "2020", "play_count": "21633093", "explicit_content": "0",
            "perma_url": "https://www.jiosaavn.com/song/levitating/3IoDK8qI",
            "image": "https://c.saavncdn.com/665/x-150x150.jpg", "language": "english",
            "more_info": {
                "album": "Future Nostalgia", "album_id": "23241654",
                "album_url": "https://www.jiosaavn.com/album/future-nostalgia/ITIyo-GDr7A_",
                "duration": "203", "label": "Warner Records",
                "has_lyrics": "true", "release_date": "2020-03-27",
                "copyright_text": "© 2020 Warner Records",
                "encrypted_media_url": "ID2ieOjCrwfgWvL5sXl4B1ImC5QfbsDylsUbBKiBMWa4cKhEF4Xz5p975Hh3jSc+rXO0khV0lO1tzxLIEtHbjhw7tS9a8Gtq",
                "artistMap": {
                    "primary_artists": [{"id":"1274170","name":"Dua Lipa","role":"primary_artists","type":"artist","image":"https://c.saavncdn.com/artists/x-150x150.jpg","perma_url":"https://www.jiosaavn.com/artist/dua-lipa-songs/r-OWIKgpX2I_"}],
                    "featured_artists": [], "artists": []
                }
            }
        });
        // lyrics_id is absent in the live response — must not fail.
        let song = song_payload(&raw);
        assert_eq!(song.name, "Levitating");
        assert_eq!(song.duration, Some(203));
        assert_eq!(song.play_count, Some(21_633_093));
        assert_eq!(song.lyrics_id, None);
        assert!(song.has_lyrics);
        assert_eq!(song.artists.primary.len(), 1);
        assert_eq!(song.artists.primary[0].name, "Dua Lipa");
        assert_eq!(song.download_url.len(), 5);
        assert_eq!(song.image.len(), 3);
    }

    #[test]
    fn lenient_numbers_accept_ints_too() {
        let raw = json!({"id":"x","title":"t","type":"song","year":2020,"play_count":5,
            "explicit_content":"1","perma_url":"u","image":"","language":"en",
            "more_info":{"duration": 10, "artistMap":{"primary_artists":[],"featured_artists":[],"artists":[]}}});
        let song = song_payload(&raw);
        assert_eq!(song.year.as_deref(), Some("2020"));
        assert_eq!(song.play_count, Some(5));
        assert!(song.explicit_content);
    }
}
