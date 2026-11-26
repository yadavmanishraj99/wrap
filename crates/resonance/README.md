# Resonance

A native Windows music streaming app. WinUI 3 interface built with
[`windows-reactor`](https://crates.io/crates/windows-reactor) (declarative
Rust components over WinUI), data from the `saavnapi` crate in this
workspace (JioSaavn), audio through WinRT `MediaPlayer`.

> Windows-only. Build and run on a Windows machine — this crate does
> not compile for Linux/macOS targets.

## Run

```powershell
cargo run -p resonance
```

Requires the **Windows App Runtime** installed (Reactor uses the
installed runtime by default). If the app fails to start with a
runtime error, install the Windows App Runtime from the Microsoft
Store / Windows App SDK installer. A self-contained build that ships
the runtime is possible later via `windows-reactor-setup`.

Checking the Windows code from Linux (no run, type-check only):

```sh
rustup target add x86_64-pc-windows-msvc
cargo check -p resonance --target x86_64-pc-windows-msvc
```

## What works (push 1 — the shell)

- **Sidebar navigation** — WinUI `NavigationView`, Left pane:
  Home, Search, Library, Settings, with icons and pane toggle.
- **Header search** — search box in the custom `TitleBar`; Enter
  (magnifier button) runs a song search and opens the Search page.
- **Bottom player bar** — 112px persistent bar modeled on Spotify's
  Now Playing bar, three zones: track info left (56px art, title on
  one line with an ellipsis, up to two lines of clickable artist
  names — each opens that artist's page — a small circular
  add-to-liked button, 24px circle with a ~16px icon, greyish until
  hovered),
  controls + progress stacked center (shuffle, previous, large
  filled circular play/pause, next, repeat — the four around the
  play button are borderless icon buttons; every button in the bar
  has a tooltip), extras right (icon-only Now playing toggle, a
  volume icon button that opens a native flyout with a vertical
  volume slider right above it, and a More button). The three zones
  are equal-width outer columns with the controls in an Auto middle
  column, so the controls are truly centered in the window. The
  progress row is inset from the sides and each timer has a gap to
  the slider, so the times never stick to it. Nothing in the bar
  clips: the bar is tall enough for the two-line artist area and
  the progress row. (The volume Flyout and the More buttons' native
  menu sit on plain default Buttons — no Subtle style, no tooltip —
  because the Subtle + tooltip + Flyout/Menu combination crashed
  the app at launch, 0xc000027b. The buttons look like regular
  buttons for now, not borderless icons.)
- **Media session (SMTC)** — the app owns the OS media session
  instead of letting MediaPlayer's automatic integration run it
  (that one only offered Play/Pause and showed "Unknown app" +
  the stream path). The session shows the track's title, artist,
  album and artwork, Play/Pause/Next/Previous all work (media keys
  and the OS controls route into the same handlers the player bar
  uses), and the status follows playback. Next/Previous are enabled
  or disabled by queue position (no Previous on the first song, no
  Next on the last one — unless repeat-all is on, when Next wraps),
  and the flyout's seek bar shows position/duration (timeline
  properties, updated on the Tick). The app identity is
  `YadavManishraj.Resonance` (set as the process AppUserModelID at
  startup); the name fully resolves once the app is installed/
  packaged with a matching registered Start-menu shortcut — until
  then it may still show "Unknown app" even with the track info
  correct.
- **Skeleton loading** — Home, Search and Details show skeleton
  placeholders while they load. The skeleton is built from each page's own layout, not a generic
  stack of bars: Home shows its first four real sections (Trending
  Now, Top Charts, New Releases, Editorial Picks) with their headings and a row
  of skeleton cards each; Search shows its five result sections
  (Top result, Songs, Albums, Artists, Playlists); a details page
  shows that kind's header (circular art for an artist, square
  otherwise; eyebrow / title / subtitle lines; the action row:
  64px circular Play, shuffle, circular like, circular download,
  horizontal more) and its sections (Top songs + Top albums +
  Similar artists for an artist; Songs for an album/playlist/chart;
  Lyrics for a song). The fill is a solid gray with real contrast
  against the page background in both themes (checked with the WCAG
  formula: dark `#4A4A4A` (1.84:1), light `#BCBCBC` (1.71:1)). The
  skeletons are static — no shimmer pulse, no fade: motion is not
  supported, so the placeholders just sit at full opacity until the
  real content replaces them. Navigation is instant too — no page
  transition, no drill-in zoom, no overlay fade (the queue overlay
  and the Now Playing sidebar appear at once).
- **Search** — typing searches the typed text live: the search is debounced ~300ms, so a burst of typing
  sends one `search_all` call (not one per keystroke) once typing
  pauses, and that one call feeds both the autocomplete
  suggestions and the full results; typing takes you to the Search
  page. Picking a suggestion searches it at once (no debounce).
  Results render as Home-style media-card sections (Top result,
  Songs, Albums, Artists, Playlists).
- **Sidebar** — Home, Search, Library at the top; Settings pinned
  to the very bottom (pane footer). It is a real NavigationViewItem
  (same hover, selection indicator, icon + label as the other nav
  items), but a footer item never fires the NavigationView's
  selection callback, so its label is wrapped in a transparent
  Border with a pointer handler that navigates to Settings; the
  selection indicator is set from the current page. Do NOT wrap the
  Icon in a Border — the Icon slot expects an IconElement, and a
  Border there crashed the app at launch (STATUS_STACK_BUFFER_OVERRUN).
  0.100 has no FooterMenuItems slot, so the PaneFooter is the only
  bottom-pinned slot. No pane title,
  and the title bar shows no current-page text: just the app title,
  the search field, and the theme toggle (no search icon button).
- **Playback** — `MediaPlayer` plays saavnapi stream URLs at the
  configured quality (96/160/320 kbps); queue with auto-advance;
  position polled every 500ms while a song is loaded.
- **Shuffle & repeat** — both work and persist in settings. Shuffle
  shuffles the upcoming queue (current song stays put); repeat cycles
  off → all → one — repeat-one restarts the song, repeat-all wraps to
  the start of the queue, and repeat wins over the autoplay setting
  at the end of a track.
- **Queue** — the Now Playing sidebar's Show full queue button opens
  a queue panel over the whole sidebar (Queue + close button, the
  queue's song rows); playing a row starts the queue there. The
  full Queue page still exists in code but has no entry point since
  the player bar's queue button was removed.
- **Song rows** — in a details page (and the queue panel), hovering a
  song row shows a circular play icon over its 48px art; clicking it
  plays that song in its queue. The title and the artist line each
  wrap to at most 2 lines, the same rule as the media cards. The row
  gets a navigation-like hover background (a card fill while hovered).
  In a details page, the favorite button sits before the duration,
  with a gap between them, and only shows while the row is hovered.
  There is no permanent Play button — the hover icon is the play
  affordance. In the Now Playing queue, the row is compact: no
  favorite button and no duration, just art, title/artist, and the
  hover background. The favorite/like button (player bar and song
  rows) is a small 24px circle with a ~16px icon: not liked, a plus
  inside a 1px circle outline, greyish until hovered; already liked,
  a check inside a filled circle in the framework's primary color
  (Accent), always full contrast. Both read in dark and light mode.
  The circle is 32px with a ~16px icon, so there is padding between
  the icon and the circle, and the circle stroke is 2px — the same
  color and the same weight as the icon's strokes.
- **Details action row** — under a details header, left to right: a
  64x64 circular Play button (filled primary, Play icon centered),
  a shuffle icon button (icon only, no container), a circular like
  button (plus in a circle outline, or a check in a filled primary
  circle when already liked), a download button (icon only, in a
  circle — a placeholder, the download feature is still under
  discussion), and a horizontal more icon button (icon only, no
  container) that opens the item's context menu. The icons in the
  circular buttons are centered, with padding between the icon and
  the circle, and the icon and the circle stroke are the same color
  and the same weight (the theme's primary text color, 2px). Artists
  can be liked too — the artist page has the same circular like
  button, and liked artists show in the Library under "Liked
  artists". Shuffle plays the item's songs in a shuffled order.
- **Lyrics** — the song details page shows a song's lyrics when the
  API has them. It currently has no entry point since the player
  bar's lyrics button was removed. No connect-to-device, miniplayer,
  or fullscreen: windows-reactor
  0.100 exposes no API for them.
- **Persistence** — liked songs, albums and playlists, plus
  recents and settings, saved as JSON under
  `%LOCALAPPDATA%\Resonance`.
- **Theme** — Dark / Light / System toggle in the title bar
  (dark-first, matching the Resonance design system).
- **Window material** — Acrylic (default, frosted glass), Mica, Mica Alt, or Solid,
  switchable in Settings → Window material; persisted in settings.
- **Context menu** — right-click (or the hover more button) on any
  Home card, any song row, or any pinned sidebar row: Play, Play
  next (queues right after the current song, or starts playing when
  nothing is), Properties (title, type, duration/year/album or song
  count in a dialog), Pin / Unpin (pins the item to the sidebar, or
  removes it). The bottom player bar has a More button at its far
  right that opens the same menu for the song now playing (inert
  while nothing is playing). The More buttons on details pages and
  the player bar open a real native WinUI MenuFlyout (a Button with
  a `Menu` attached), and Properties is a real native WinUI
  ContentDialog (the item title is the dialog title, the details
  are its content, Close is its close button). The MenuFlyout
  buttons are plain default Buttons — no Subtle style, no tooltip —
  because the Subtle + tooltip + Menu combination crashed the app
  at launch (0xc000027b). Right-click and the
  media card's hover More button still use a custom overlay (a
  Border in the shell's overlay layer): Reactor 0.100's native menu
  only opens on Button click, never on right-click, and it has no
  programmatic show, so a right-click menu cannot be native; and a
  native menu on a hover-revealed button risks the button unmounting
  mid-press and closing the menu. The custom menu and dialog are
  opaque (a solid fill), never translucent. Pointer events exist
  only on `Border` in windows-reactor 0.100, so every press/hover
  handler
  sits on a Border and the menu's click-away layer is a sibling of
  the menu, never its parent.
- **Pinning** — any song, artist, album, playlist, or chart can be
  pinned from its card's context menu (Pin / Unpin) or the Pin
  button in a details page's action row. Pinned items
  appear in the sidebar below a divider after Library, most
  recently pinned first,
  each as a row with its art (48px, the same size as the song-row art
  in the details views) + title (14px, the same size as the nav
  items) + subtitle (12px, dimmed, below the title with a small gap
  between them). The 48px art makes the pinned rows taller than a
  plain nav item, and there is a little vertical breathing room
  between rows. On hover, a circular play icon appears over the
  pinned art; clicking it plays the item — a song plays itself, an
  artist plays their top songs, an album/playlist/chart plays its
  songs. (The icon is a sibling of the row, not a child, so its
  press never also triggers the row's click handler.)
  click.) Every square art in
  the app has 4px rounded corners (pinned art, song-row art, details
  header art, media-card art, Now Playing art, player-bar art);
  artist art is a full circle everywhere (media cards, pinned rows,
  details header, Now Playing brief). Pins persist in the SQLite
  catalog (`library_refs`). A pinned row is a shortcut, not a page: clicking
  it never changes the nav selection. A pinned song plays on click
  (its full Song is fetched by id first); any other pinned item opens
  its details page. The divider shows only while pinned items exist.
  Reactor 0.100 has no NavigationViewItemSeparator, so the divider is
  a NavigationViewItem with a 1px line as its content and
  `selects_on_invoked(false)`; the line runs end to end, left to
  right (no horizontal margin).
- **Playback disposal** — when a track starts, the previous track's
  `MediaSource` is closed explicitly (`Close()` releases its stream
  and network resources at once) instead of leaving it to COM
  refcounting. Rust's ownership frees the rest: popped details,
  replaced search/home data, and the capped recents are
  dropped when no longer needed.

## Pages

| Page | Status |
| --- | --- |
| Home | Live — launch data as clickable media cards: every section except History, in Saavn's own module order with Saavn's titles (Trending Now, Top Charts, New Releases, Editorial Picks, Trending Podcasts, Radio Stations, Recommended Artist Stations, the promo sections, Discover); each section is 1 row (≤ 10 items) or 2 rows (> 10, 20px gap between rows), scrolling horizontally (scrollbar hidden); cards are 168px, 12px apart, top-aligned, art corners rounded 4px (artist cards are full circles), title/subtitle max 2 lines, hover shows a card-fill background plus circular Play + more buttons, right-click/more opens a context menu (Play, Play next, Properties, Pin/Unpin). Cards with no art show a placeholder tile (music-note icon on a solid fill). Clicking a song card plays it; artist-station cards open the artist page; any other card opens its page — except channels, radio stations and shows, which show a plain "Not available yet" notice (Saavn's station-songs call is down on their side; channel and show pages aren't built yet) |
| Details | Live — one page per kind, each with its own layout: Artist (circular art, followers, Play, Top songs, Top albums, Similar artists), Album (art, clickable artist names, year • songs, Play, Like), Playlist / Chart (art, description, Play, Like), Song (art, clickable artists, Play, Like, lyrics when available); title-bar back button returns. Song cards never open this page — they just play. Details are an inner-route stack: opening a details page from inside another details page pushes a new entry, and Back walks back through each one in reverse order (Home → album → album → artist → back → back → back → Home), instead of dropping straight to the page. Tapping the current section's nav item (e.g. Home while deep in Home's details) returns straight to that section's first page, popping every detail layer at once — the nav label carries a press handler for this, since the nav only reports selection *changes* and tapping an already-selected item would otherwise do nothing |
| Queue | Live — the queue panel over the Now Playing sidebar (from its Show full queue button); playing a row starts the queue there; song titles wrap |
| Now Playing | Live — an opaque 340px sidebar on the right (a solid fill, so the page never bleeds through it). It only opens and closes from the player bar's Now playing button — a track starting never opens it on its own (covers the right of the page, never the title bar or player bar). Top to bottom: the current song's details (big art, title, clickable artist names, album (clickable), Like), then the main artist's brief (circular picture, name + followers, fetched on track change; tap opens the artist page), then a Next-in-queue card (clicking it plays that song) and a Show full queue button (opens a queue panel over the whole sidebar — Queue + close button, the queue's song rows — which appears at once; hiding the sidebar closes it too). Hidden with the close button or the player bar's Now playing toggle; it stays hidden until the toggle is pressed again |
| Search | Debounced ~300ms — typing searches the typed text (results + autocomplete from one call); results are Home-style media-card sections: Top result, Songs, Albums, Artists, Playlists. Results are cached per search (see Offline-first); with no connection and no cached result, search falls back to the local catalog — downloads and library — so downloaded music is still findable offline |
| Library | Live — liked songs, recently played, liked albums/playlists/artists (persisted locally), all as Home-style media-card sections; albums, playlists and artists land here via the Like button on their pages. **Downloads** (auto-playlist of individually downloaded songs) and **Downloaded artists** (one playlist per downloaded artist, holding their top songs) also live here |
| Settings | Live — theme, window material (Acrylic default), stream quality, autoplay, downloads (auto-download switches, download quality, image cache size, recently-played cap), about |

## Offline-first

Pages open from local data first and refresh in the background:

- **Page cache:** Home, every details page, and search results are cached as JSON files in `%LOCALAPPDATA%\Resonance\cache\` (one file per entry, stamped with its fetch time). Opening a page shows the cache instantly — no skeleton. A fresh entry (Home / search: 30 minutes, details: 24 hours) is served with no network call at all; a stale entry is shown and refreshed in the background, and the refresh replaces it only when it succeeds. **Nothing is evicted while offline** — there is no eviction; an entry lives until a fresh copy replaces it, so whatever was cached is what the user can browse with no connection.
- **Playback is local-first too:** a downloaded song (pinned, in the Library, or on any card) plays straight from the catalog and its local file — no fetch. A downloaded collection plays its downloaded songs. Only an item the app has never stored is fetched, and if that fetch fails the downloaded part plays instead; when nothing playable is stored, a dialog says so in plain words ("This song isn't downloaded, and you're offline…"). No error anywhere in the app shows technical text — page errors are plain sentences with a Retry / Back action.

## Downloads

Live — a song, album, playlist or artist can be downloaded for offline listening:

- **Where the button is:** album / playlist / artist / song pages have a Download button in the action row; everywhere else (media cards, song rows, pinned rows) Download lives in the context menu (right-click, or the More button). The details-page button also shows the download state (a filled check once downloaded, dimmed while downloading).
- **What downloads:** a single song goes into the auto **Downloads** playlist in the Library. A full album / playlist downloads every song. An artist downloads their top songs and creates a playlist for them under the artist's name (**Downloaded artists** in the Library). Downloads are saved at the download quality set in Settings (separate from the stream quality).
- **Storage model:** everything lives in one SQLite catalog (`%LOCALAPPDATA%\Resonance\resonance.db`) with four tables — `media` (one row per song: metadata + download state), `collections` (one row per album / playlist / chart / artist, with everything its details view needs), `collection_media` (the relationship: which songs belong to which collection, in track order — one song can belong to many collections) and `library_refs` (liked / pinned / recent entries, each just pointing at a media or collection row). On disk there are only two folders under `%USERPROFILE%\Music\Resonance`: `music\<song_id>.mp3` — the one real copy of each song, named by ID — and `covers\<cover_id>.jpg` — one cover per id: every song caches its own art under its own id, and a downloaded album / playlist / artist also downloads its OWN cover (the collection's art, not a member song's) under the collection id, so collection pages and cards show the right picture offline. A song is downloaded **once**, however many collections contain it; asking for it again from another playlist just touches its `updated_at` timestamp (every row carries `created_at` / `updated_at`, and pinned / recent / downloads order by `updated_at`, newest first). Liking or pinning also creates the item's catalog row (not downloaded), so the library, pins and recents all live in the same catalog — there is no `library.json` any more (an old one is migrated on first run, as are the pre-catalog download tables and their per-collection folders).
- **Progress:** a downloading song shows a circular progress ring with a download arrow in the middle, centered on its art (song rows and media cards). A downloading collection shows its overall progress ("Downloading X of Y songs" + a bar) above the song list on its page, while each song row shows its own progress on its art. A song downloaded on its own shows progress only on itself — the collection page shows nothing. A downloaded song gets a filled circle check at the start of its subtitle, **everywhere it appears** — song rows, media cards, pinned rows in the sidebar, the player bar, and the Now Playing sidebar.
- **Resilience:** downloads write to a `.part` file and resume from it (HTTP `Range`) after a failure or an app restart, retrying up to 3 times with a backoff. The queue lives in the catalog, so an interrupted download resumes on the next launch. A startup sweep deletes orphaned files (audio whose song is not downloaded, stale partials, unreferenced covers).
- **Deleting downloads:** the context menus (and the details More menu) offer **Delete download**. Deleting one song removes its file even if it was downloaded as part of an album / playlist — the song stays in the collection's tracklist, just not downloaded, and the collection's progress drops accordingly (re-downloading it fetches only the missing songs). Deleting a whole collection's download removes its memberships, and a song's file is only deleted when that was its last membership — a song also in another downloaded collection, or downloaded on its own, keeps its file. Deleting wins over auto-download: nothing re-downloads until the item is liked / pinned / played / downloaded again.
- **Offline playback:** a downloaded song plays from its local file instead of streaming, so it works with no network. The catalog holds the song metadata, the covers are on disk, and the collection → songs relationships are stored, so a downloaded album / playlist / artist opens from the stored data offline — no network needed. (A partially downloaded collection opens from the network when online, and falls back to its downloaded songs when the fetch fails.) The Library's Downloads and Downloaded artists sections work offline too.
- **Auto-download (Settings → Downloads, all on by default):** liking a song / album / playlist / artist downloads it; pinning an item downloads it; playing a song downloads it (the recent list's size is configurable in Settings — 50 / 100 / 200 / 500, default 100 — and recent songs are downloaded too). Each can be switched off separately.
- **Image cache (Settings → Downloads):** album art is cached on disk (`%LOCALAPPDATA%\Resonance\image_cache\`), so Home and details pages do not re-download the same image on every visit. The size limit is configurable (100 MB / 250 MB / 500 MB / 1 GB, default 500 MB); past the limit the least-recently-used images are evicted.
- **Not done yet:** minimizing to the system tray when the app is closed while a download is running. Reactor exposes no window-close event, so a proper minimize-to-tray needs Win32 window subclassing that cannot be verified from the Linux build machine. Closing the app mid-download is safe regardless — the partial file and queue are persisted, and the download resumes on the next launch.

## Layout

```
crates/resonance/src/
  main.rs     entry — App::run_component::<Shell>
  app.rs      Shell component: state, messages, TitleBar +
              NavigationView + player bar, player polling
  data.rs     shared tokio runtime + saavnapi client
  cache.rs    file-based page cache (Home / details / search),
              offline-first: instant opens, background refresh,
              nothing evicted while offline
  catalog.rs the SQLite catalog (media / collections /
              collection_media / library_refs) + the download
              engine: resume (.part + Range) + retry, dedup by
              id, offline data, covers; download.rs is a tombstone
  image_cache.rs disk cache for album art (Home/details), with a
              size limit + least-recently-used eviction
  player.rs   MediaPlayer wrapper: queue, play/pause/seek/volume,
              shuffle-upcoming, repeat wrap/restart
  store.rs    Settings persistence (settings.json,
              %LOCALAPPDATA%), incl. shuffle + repeat mode
  views.rs    shared bits: song row, media card, card section
              (Home/Search/Library layout), clickable artist links,
              section header, formatting
  pages/      home.rs, details.rs, queue.rs, search.rs, library.rs,
              settings.rs
```

**Build note (SQLite):** downloads use `rusqlite` with the `bundled` SQLite C library. That C code needs an MSVC toolchain, which the Linux build machine does not have — so on Linux, verify with the MinGW cross-compiler against the GNU target (`CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar cargo check -p resonance --target x86_64-pc-windows-gnu`; the target needs `rustup target add x86_64-pc-windows-gnu` first). The user's Windows `cargo run` (MSVC) compiles the bundled SQLite normally.

## Design

Follows the Windows app design guidelines and the Resonance Figma
design system (teal seed, dark-first): NavigationView shell, title-bar
search, persistent media bar, 4/8/12/24 spacing, 12/14/20/28 type
ramps. All network work runs off the UI thread via Reactor's
`spawn_background`; the UI never awaits.
