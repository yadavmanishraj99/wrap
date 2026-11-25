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
- **Bottom player bar** — 76px persistent bar: artwork, track
  title/artist, previous / play-pause / next, seek slider with
  time labels, volume slider.
- **Playback** — `MediaPlayer` plays saavnapi stream URLs at the
  configured quality (96/160/320 kbps); queue with auto-advance;
  position polled every 500ms while a song is loaded.
- **Persistence** — liked songs and recents, plus settings, saved as
  JSON under `%LOCALAPPDATA%\Resonance`.
- **Theme** — Dark / Light / System toggle in the title bar
  (dark-first, matching the Resonance design system).

## Pages (in progress)

| Page | Status |
| --- | --- |
| Home | Placeholder — launch data (trending, new albums, charts) next |
| Search | Placeholder — typed search, suggestions, all result types next |
| Library | Placeholder — liked songs/albums/playlists, recents next |
| Settings | Placeholder — theme, quality, volume, autoplay next |

## Layout

```
crates/resonance/src/
  main.rs     entry — App::run_component::<Shell>
  app.rs      Shell component: state, messages, TitleBar +
              NavigationView + player bar, player polling
  data.rs     shared tokio runtime + saavnapi client
  player.rs   MediaPlayer wrapper: queue, play/pause/seek/volume
  store.rs    Library + Settings persistence (%LOCALAPPDATA%)
  views.rs    shared bits: song row, section header, formatting
  pages/      home.rs, search.rs, library.rs, settings.rs
```

## Design

Follows the Windows app design guidelines and the Resonance Figma
design system (teal seed, dark-first): NavigationView shell, title-bar
search, persistent media bar, 4/8/12/24 spacing, 12/14/20/28 type
ramps. All network work runs off the UI thread via Reactor's
`spawn_background`; the UI never awaits.
