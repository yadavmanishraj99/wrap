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
- **Bottom player bar** — 96px persistent bar modeled on Spotify's
  Now Playing bar, three zones: track info left (56px art, title on
  one line with an ellipsis, up to two lines of clickable artist
  names — each opens that artist's page — add-to-liked button),
  controls + progress stacked center (shuffle, previous, large
  filled circular play/pause, next, repeat; elapsed / seek slider /
  duration below), extras right (lyrics, queue, mute + volume
  slider). Nothing in the bar clips: the bar is tall enough for the
  two-line artist area and the progress row.
- **Search** — typing in the header search field searches whatever
  is typed, live: one `search_all` call per keystroke feeds both the
  autocomplete suggestions and the full results, and typing takes
  you to the Search page. Picking a suggestion searches it too.
  Results render as Home-style media-card sections (Top result,
  Songs, Albums, Artists, Playlists).
- **Sidebar** — Home, Search, Library at the top; Settings pinned
  to the very bottom (pane footer — 0.100 has no FooterMenuItems
  slot, so it is a Border with a pointer-press handler in the
  PaneFooter slot; a Button there never answered its click).
  Selected state is a card-background fill. No pane title,
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
- **Queue page** — the player bar's queue button opens the current
  queue ("Up next"); playing a row starts the queue there.
- **Lyrics** — the player bar's lyrics button opens the current
  song's details page, which shows its lyrics when the API has them.
  No connect-to-device, miniplayer, or fullscreen: windows-reactor
  0.100 exposes no API for them.
- **Persistence** — liked songs, albums and playlists, plus
  recents and settings, saved as JSON under
  `%LOCALAPPDATA%\Resonance`.
- **Theme** — Dark / Light / System toggle in the title bar
  (dark-first, matching the Resonance design system).
- **Window material** — Mica (default), Mica Alt, Acrylic, or Solid,
  switchable in Settings → Window material; persisted in settings.
- **Context menu** — right-click (or the hover more button) on any
  Home card: Play, Play next (queues right after the current song,
  or starts playing when nothing is), Properties (title, type,
  duration/year/album or song count in a dialog). Pointer events
  exist only on `Border` in windows-reactor 0.100, so every
  press/hover handler sits on a Border and the menu's click-away
  layer is a sibling of the menu, never its parent.

## Pages

| Page | Status |
| --- | --- |
| Home | Live — launch data as clickable media cards: Trending now, Top playlists, New albums, Charts; each section is 1 row (≤ 10 items) or 2 rows (> 10, 20px gap between rows), scrolling horizontally (scrollbar hidden); cards are 168px, 12px apart, top-aligned, art corners rounded 4px, title/subtitle max 2 lines, hover shows circular Play + more buttons, right-click/more opens a context menu (Play, Play next, Properties). Clicking a song card plays it; any other card opens its page |
| Details | Live — one page per kind, each with its own layout: Artist (circular art, followers, Play, Top songs, Top albums, Similar artists), Album (art, clickable artist names, year • songs, Play, Like), Playlist / Chart (art, description, Play, Like), Song (art, clickable artists, Play, Like, lyrics when available); title-bar back button returns. Song cards never open this page — they just play |
| Queue | Live — current playback queue from the player bar; playing a row starts the queue there; song titles wrap |
| Search | Live — typing searches the typed text live (results + autocomplete from one call); results are Home-style media-card sections: Top result, Songs, Albums, Artists, Playlists |
| Library | Live — liked songs, recently played, liked albums/playlists (persisted locally), all as Home-style media-card sections; albums and playlists land here via the Like button on their pages |
| Settings | Live — theme, window material (Mica default), stream quality, volume, autoplay, about |

## Layout

```
crates/resonance/src/
  main.rs     entry — App::run_component::<Shell>
  app.rs      Shell component: state, messages, TitleBar +
              NavigationView + player bar, player polling
  data.rs     shared tokio runtime + saavnapi client
  player.rs   MediaPlayer wrapper: queue, play/pause/seek/volume,
              shuffle-upcoming, repeat wrap/restart
  store.rs    Library + Settings persistence (%LOCALAPPDATA%),
              incl. shuffle + repeat mode
  views.rs    shared bits: song row, media card, card section
              (Home/Search/Library layout), clickable artist links,
              section header, formatting
  pages/      home.rs, details.rs, queue.rs, search.rs, library.rs,
              settings.rs
```

## Design

Follows the Windows app design guidelines and the Resonance Figma
design system (teal seed, dark-first): NavigationView shell, title-bar
search, persistent media bar, 4/8/12/24 spacing, 12/14/20/28 type
ramps. All network work runs off the UI thread via Reactor's
`spawn_background`; the UI never awaits.
