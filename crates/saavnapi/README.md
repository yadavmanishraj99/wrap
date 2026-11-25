# saavnapi

Unofficial Rust client for JioSaavn, built from scratch against the real
JioSaavn web API. It does **not** call the `sumitkolhe/jiosaavn-api`
hosted API (`saavn.dev`) or reuse its code — only the wire knowledge
learned by studying that repo (cloned at
`~/workspace/wrap-study/jiosaavn-api`) was carried over.

## How it works

Every call is one `GET` to `https://www.jiosaavn.com/api.php`:

```
__call=<endpoint>&_format=json&_marker=0&api_version=4&ctx=web6dot0
```

plus the call-specific parameters, with headers
`Content-Type: application/json` and a random browser `User-Agent`
per request. No auth, no cookies, no API key.

Audio URLs arrive encrypted. `more_info.encrypted_media_url` is
base64, decrypted with **DES-ECB, key `38346591`** (the `00000000`
IV in the reference is dead — ECB has no IV). The plaintext URL
contains `_96`; swapping it for `_12` / `_48` / `_96` / `_160` /
`_320` gives the 12/48/96/160/320 kbps variants. Images are the same
idea: replace the `150x150`/`50x50` token with `50x50`/`150x150`/`500x500`
and force `https://`.

Verified live 2026-11-25: `song.getDetails` for `3IoDK8qI`
("Levitating") decrypts (also checked with
`openssl enc -d -des-ecb -K 3338333436353931`) to
`https://aac.saavncdn.com/665/7790c3b9097592113008eaf1031d6e57_96.mp4`.

## Endpoints — all 42 known `__call` values (2026-11-25)

The whole-picture exploration (`~/workspace/wrap-study/ENDPOINT_INVENTORY.md`)
found 42 distinct `__call` values across every public client and live
probe. **All 42 are now implemented** (the 17 the reference wires up,
plus the 25 gaps found by the exploration).

| Method | `__call` | Params | `ctx` |
|---|---|---|---|
| `get_songs_by_ids` | `song.getDetails` | `pids=<comma ids>` | web6dot0 |
| `get_songs_by_link` | `webapi.get` | `token`, `type=song` | web6dot0 |
| `create_song_station` | `webradio.createEntityStation` | `entity_id=["<url-enc id>"]`, `entity_type=queue` | android |
| `get_song_suggestions` | `webradio.getSong` | `stationid`, `k=<limit>` | android |
| `get_album_by_id` | `content.getAlbumDetails` | `albumid` | web6dot0 |
| `get_album_by_link` | `webapi.get` | `token`, `type=album` | web6dot0 |
| `get_artist_by_id` | `artist.getArtistPageDetails` | `artistId`, `n_song`, `n_album`, `page`, `sort_order`, `category` | web6dot0 |
| `get_artist_by_link` | `webapi.get` | `token`, `type=artist` + artist params | web6dot0 |
| `get_artist_songs` | `artist.getArtistMoreSong` | `artistId`, `page`, `sort_order`, `category` | web6dot0 |
| `get_artist_albums` | `artist.getArtistMoreAlbum` | `artistId`, `page`, `sort_order`, `category` | web6dot0 |
| `get_playlist_by_id` | `playlist.getDetails` | `listid` (numeric), `n`, `p` | web6dot0 |
| `get_playlist_by_link` | `webapi.get` | `token`, `n`, `p`, `type=playlist` | web6dot0 |
| `search_all` | `autocomplete.get` | `query` (note: **not** `q`) | web6dot0 |
| `search_songs` | `search.getResults` | `q`, `p`, `n` | web6dot0 |
| `search_albums` | `search.getAlbumResults` | `q`, `p`, `n` | web6dot0 |
| `search_artists` | `search.getArtistResults` | `q`, `p`, `n` | web6dot0 |
| `search_playlists` | `search.getPlaylistResults` | `q`, `p`, `n` | web6dot0 |
| `get_launch_data` | `webapi.getLaunchData` | `language` (optional) | web6dot0 |
| `get_browse_modules` | `content.getBrowseModules` | — | web6dot0 |
| `get_trending` | `content.getTrending` | `p`, `n` | web6dot0 |
| `get_lyrics` | `lyrics.getLyrics` | `lyrics_id=<song pid>` | web6dot0 |
| `get_song_recommendations` | `reco.getreco` | `pid` | **android** (web6dot0 always returns `[]`) |
| `get_album_recommendations` | `reco.getAlbumReco` | `albumid` | web6dot0 |
| `get_playlist_recommendations` | `reco.getPlaylistReco` | `listid` | web6dot0 |
| `get_charts` | `content.getCharts` | — (no params; `p`/`n` behave oddly) | web6dot0 |
| `get_albums` | `content.getAlbums` | `p`, `n`, `language` (optional) | web6dot0 |
| `get_featured_playlists` | `content.getFeaturedPlaylists` | `p`, `n`, `languages` (optional) | web6dot0 |
| `get_top_shows` | `content.getTopShows` | `p`, `n` | web6dot0 |
| `get_top_artists` | `social.getTopArtists` | — | web6dot0 |
| `get_top_searches` | `content.getTopSearches` | `language` (optional) | web6dot0 |
| `get_featured_stations` | `webradio.getFeaturedStations` | `language` (optional) | web6dot0 |
| `get_top_albums_of_year` | `search.topAlbumsoftheYear` | `album_year`, `album_lang` (optional) | web6dot0 |
| `get_show_by_link` | `webapi.get` | `token`, `type=show` | web6dot0 |
| `get_episode_by_link` | `webapi.get` | `token`, `type=episode` | web6dot0 |
| `get_show_episodes` | `show.getAllEpisodes` | `show_id`, `season_number`, `p` (1-indexed) | web6dot0 |
| `create_featured_station` | `webradio.createFeaturedStation` | `name`, `language` (optional) | web6dot0 |
| `create_artist_station` | `webradio.createArtistStation` | `name`, `query`, `language` (optional) | web6dot0 |
| `generate_auth_token` | `song.generateAuthToken` | `url=<encrypted_media_url>`, `bitrate` | web6dot0 |
| `get_footer_details` | `webapi.getFooterDetails` | `language` | web6dot0 |
| `get_browse_hover_details` | `webapi.getBrowseHoverDetails` | — | web6dot0 |
| `get_mix_by_link` | `webapi.get` | `token`, `type=mix` | web6dot0 |
| `get_label_by_link` | `webapi.get` | `token`, `type=label` | web6dot0 |
| `search_podcasts` | `search.getMoreResults` | `query`, `p`, `n`, `params='{"type":"podcasts"}'` | web6dot0 |
| `get_artist_top_songs` | `search.artistOtherTopSongs` | `artist_ids`, `song_id`, `p`, `category`, `sort_order`, `language` | web6dot0 |
| `get_actor_top_songs` | `search.actorOtherTopSongs` | `actor_ids`, `song_id`, `language` | web6dot0 |

Launch data is the home page payload — `new_trending` (24 items),
`new_albums` (20), `top_playlists` (12), `charts` (9), `radio` (28),
`artist_recos` (20), `browse_discover` (28), plus dynamic
`promo:*` sections and a `modules` map describing each section
(including its "view more" API — `new_trending` points at
`content.getTrending`). It was **not** in the `sumitkolhe`
reference at all; the endpoint is documented by other clients
(`webapi.getLaunchData`) and verified live. Sections are mixed
types (album/song/playlist/channel/radio station/show), so they
are returned as `LaunchItem`s with the raw `more_info` preserved,
not forced into one entity type. `get_browse_modules` returns the
raw section map for the same reason (`radio` is
`{ featured_stations: […] }`, `top_shows` is
`{ badge, shows, last_page }`).

### Launch-data hardening (2026-11-25)

Launch data is the loosest payload in the client, so it gets the
strictest treatment:

- **Real-response fixtures** (`tests/fixtures/launch_data_english.json`,
  `launch_data_hindi.json`, `browse_modules.json`, `trending.json` —
  captured live 2026-11-25) are parsed in unit tests with exact
  section counts asserted (24 trending, 20 new albums, 12 top
  playlists, 9 charts, 28 radio, 20 artist recos, 28 browse-discover,
  promo 9/7/7/29, 193 items total, module positions 1–10 unique).
- **Wire quirks found in the fixtures and handled:** `more_info`
  is a list `[]` on show items (not an object); `view_more` is a
  list `[]` on 5 of 10 modules (parses as `None`); charts carry
  no `explicit_content`; `city_mod` appears only with some
  languages (hindi/tamil/bengali yes, english/punjabi no) and is
  now a typed field, not `extra`.
- **Lenient numbers everywhere:** `count`, module `position` and
  `view_more.default_size` accept string or number form, like the
  rest of the client.
- **Junk never becomes a phantom item:** array elements that parse
  to an empty id *and* empty title (`null`, numbers, strings) are
  dropped from sections.
- **Malformed-input tests:** `null`, `{}`, `[]`, wrong-typed
  sections/modules/`more_info`/`view_more` are all fed through
  the parsers and must not panic.
- **Serde roundtrip:** a parsed `LaunchData` serializes and
  deserializes back to an equal value.
- **Live:** `get_launch_data(None)` and `get_launch_data(Some("hindi"))`
  (asserts `city_mod` present), trending pagination (page 0 vs 1
  return different items), browse modules — all passing.
  `cargo fmt --check` and `cargo clippy --all-targets` clean.

Link tokens are the last path segment of a JioSaavn URL
(`…/song/houdini/OgwhbhtDRwM` → `OgwhbhtDRwM`); a bare token works too.

Defaults match the reference controllers: search/playlist
`page=0, limit=10`; artist `page=0, song_count=10, album_count=10,
sort_by=popularity, sort_order=asc` (detail) / `desc` (songs/albums
sub-routes — an inconsistency in the reference, preserved);
suggestions `limit=10`.

## Live status (2026-11-25)

32 unit + 16 live + 3 doc tests pass (`cargo test`, then
`cargo test -- --ignored`). One live-test caveat: the suite hits the
real API, and under full parallel load the server occasionally
flakes — the lyrics call once answered its generic failure message
for a song that has lyrics, and `city_mod` once went missing from a
Hindi launch-data response. Both pass reliably on re-run / solo;
`get_lyrics` deliberately maps the server's generic failure to
`Ok(None)` because "no lyrics" and a transient failure are the same
response body.

Song suggestions remain server-side broken: `webradio.createEntityStation`
works, but `webradio.getSong` answers
`{ stationid, error: "No new song found for current radio." }`
for every song/k tried. The client surfaces that as an
`InvalidResponse` error. Use `get_song_recommendations`
(`reco.getreco`, android ctx) instead — it works.

## Deliberately not implemented

- Mix/label live shapes: `get_mix_by_link` / `get_label_by_link`
  are implemented from the TS wrapper's payloads but not yet
  live-verified with a real token (mix/label links are rare in
  search results).
- `search_podcasts` is the least-verified call: the TS config says
  `params='{"type":"podcasts"}'`, one live probe with `{}` returned
  `[]`; the literal-JSON form is untested live.
- Single-source, unconfirmed names are not implemented:
  `reco.getArtistReco`, `content.getShowDetails`, `show.getDetails`,
  `search.getPodcastResults`, `lyrics.get` (almost certainly a
  README typo for `lyrics.getLyrics`).
- The client only returns decrypted/signed CDN URLs; it never
  downloads audio bytes.

## Gap implementation (2026-11-25)

All 25 gaps from the whole-picture exploration were implemented in
one pass (`src/{lyrics,reco,discovery,shows,radio,streaming,chrome,collections}.rs`
plus podcast/artist/actor search helpers in `src/search.rs` and a
new `Song.encrypted_media_url` field so `generate_auth_token` has
something to sign). Live-verified the same day: lyrics present
("Rula Ke Gaya Ishq") and absent ("Levitating" → `None`), reco
trio, all 8 discovery lists (charts exactly 9, top artists exactly
30, stations exactly 28, top albums of the year exactly 24),
shows + episode pagination, both station creations, auth token
(signed `web.saavncdn.com` URL with `_96.mp4`), footer + mega menu,
and artist top songs on a populated artist (Arijit Singh — Dua Lipa
returns `[]` live).

## Quirks found in the study (preserved or fixed)

- Playlist by link goes through `Endpoints.albums.link` in the
  reference — same `webapi.get` string, so the wire call is identical.
- Playlist `songCount` in the public response is **not** `list_count`;
  it is the length of the returned (sliced) song list.
- `playlist.getDetails` `listid` must be numeric (`82914609`);
  a token gives `INPUT_INVALID: Only numbers are allowed`.
- The reference's zod models mark many fields required, but the live
  API omits some (`lyrics_id` was absent on the test song) and returns
  numbers where the schema says string. This client parses the raw
  JSON leniently (string/number/null → the right type) instead.
- `search_all` in the reference reads `search.songs.position` without
  optional chaining; this client defaults a missing section to
  `position: 0, results: []`.

## Usage

```rust
use saavnapi::SaavnClient;

let client = SaavnClient::new();
let songs = client.get_songs_by_ids(&["3IoDK8qI"]).await?;
let results = client.search_songs("levitating", 0, 5).await?;
let album = client.get_album_by_id("23241654").await?;
let suggestions = client.get_song_suggestions("3IoDK8qI", 10).await?;
```

Run the example: `cargo run --example search -- levitating`

Tests: `cargo test` (unit tests, including the live DES vector —
no network). Live tests are `#[ignore]`d:
`cargo test -- --ignored` (hits `jiosaavn.com`).
