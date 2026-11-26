# wam-poc — Windows Animation Manager via windows-rs

A small proof of concept that **creates and runs a Windows Animation
Manager animation from Rust**, using only the `windows` crate
(windows-rs) bindings — no C++.

## Run it (Windows)

```
cargo run -p wam-poc
```

It animates two values over 2 seconds and prints them as a live text
progress bar, ~60 times a second:

- **fade** 0 → 1, accelerate–decelerate ease (30% / 30%)
- **slide** 0 → 100, linear

## What it proves

- The animation manager and transition library coclasses
  (`UIAnimationManager2`, `UIAnimationTransitionLibrary2`) can be
  created from Rust via `CoCreateInstance`.
- Animation variables, storyboards, and library transitions
  (linear, accelerate–decelerate, sinusoidal, cubic, smooth-stop, …)
  all work through the `Win32_UI_Animation` bindings.
- The per-frame loop an application drives itself: `Update(now)`,
  then `GetValue` on each variable.

## What it does not do

Windows Animation Manager never renders — it only computes values;
the application applies them to its UI and redraws. This demo applies
them to the console. Animating Resonance's screens would need the
values applied to Reactor's views every frame, and windows-reactor
0.100 exposes neither the native elements nor a frame callback —
see `~/workspace/wrap-study/windows-animation/STUDY.md` (kept with the
project notes) for the full analysis.
