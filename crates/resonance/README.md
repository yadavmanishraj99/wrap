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
- **Bottom player bar** — 90px persistent bar modeled on Spotify's
  Now Playing bar, three zones: track info left (56px art, title /
  artist, add-to-liked button), controls + progress stacked center
  (shuffle, previous, large filled circular play/pause, next,
  repeat; elapsed / seek slider / duration below), extras right
  (lyrics, queue, mute + volume slider).
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
- **Persistence** — liked songs and recents, plus settings, saved as
  JSON under `%LOCALAPPDATA%\Resonance`.
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
| Home | Live — launch data as clickable media cards: Trending now, Top playlists, New albums, Charts; each section is 1 row (≤ 10 items) or 2 rows (> 10), scrolling horizontally (scrollbar hidden); cards are 138px, top-aligned, title/subtitle max 2 lines, hover shows circular Play + more buttons, right-click/more opens a context menu (Play, Play next, Properties) |
| Details | Live — opened by clicking any Home card: artwork, title, subtitle, Play, song list, lyrics for songs when available; title-bar back button returns |
| Queue | Live — current playback queue from the player bar; playing a row starts the queue there |
| Search | Live — AutoSuggestBox suggestions + Top result, Songs, Albums, Artists, Playlists |
| Library | Live — liked songs, recently played, liked albums/playlists (persisted locally) |
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
  views.rs    shared bits: song row, media card, section header,
              formatting
  pages/      home.rs, details.rs, queue.rs, search.rs, library.rs,
              settings.rs
```

## Design

Follows the Windows app design guidelines and the Resonance Figma
design system (teal seed, dark-first): NavigationView shell, title-bar
search, persistent media bar, 4/8/12/24 spacing, 12/14/20/28 type
ramps. All network work runs off the UI thread via Reactor's
`spawn_background`; the UI never awaits.
