//! wam-poc — Windows Animation Manager from Rust (windows-rs).
//!
//! Proves the full creation path end to end:
//!   1. Create the animation manager and the transition library
//!      (COM coclasses, via the `windows` crate's Win32_UI_Animation
//!      bindings).
//!   2. Create two animation variables (a fade 0→1 and a slide
//!      0→100).
//!   3. Build a storyboard per variable with a library transition
//!      (accelerate–decelerate ease for the fade, linear for the
//!      slide) and schedule both.
//!   4. Drive the manager with a ~60 fps loop: Update(now), read
//!      the current values back, print them as a text progress
//!      bar — the same "ask for the value, apply it yourself"
//!      loop a real app would use to move UI properties.
//!
//! Windows Animation Manager never draws anything; this demo
//! "applies" the values to the console. Run on Windows:
//!   cargo run -p wam-poc

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::io::Write;
    use std::time::{Duration, Instant};

    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Animation::{
        IUIAnimationManager2, IUIAnimationTransitionLibrary2, UIAnimationManager2,
        UIAnimationTransitionLibrary2,
    };

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

        let manager: IUIAnimationManager2 =
            CoCreateInstance(&UIAnimationManager2, None, CLSCTX_INPROC_SERVER)?;
        let library: IUIAnimationTransitionLibrary2 =
            CoCreateInstance(&UIAnimationTransitionLibrary2, None, CLSCTX_INPROC_SERVER)?;

        // The values being animated: a fade (0→1) and a slide
        // (0→100), both starting at 0.
        let fade = manager.CreateAnimationVariable(0.0)?;
        let slide = manager.CreateAnimationVariable(0.0)?;

        // One storyboard per variable. The fade eases in and out
        // (30% accelerate, 30% decelerate); the slide is linear.
        // Both run for 2 seconds.
        let fade_storyboard = manager.CreateStoryboard()?;
        fade_storyboard.AddTransition(
            &fade,
            &library.CreateAccelerateDecelerateTransition(2.0, 1.0, 0.3, 0.3)?,
        )?;
        let slide_storyboard = manager.CreateStoryboard()?;
        slide_storyboard.AddTransition(&slide, &library.CreateLinearTransition(2.0, 100.0)?)?;

        // Schedule at animation time 0 and drive the manager with
        // wall-clock elapsed time — Update computes the values,
        // GetValue reads them back.
        fade_storyboard.Schedule(0.0, None)?;
        slide_storyboard.Schedule(0.0, None)?;

        let start = Instant::now();
        loop {
            let t = start.elapsed().as_secs_f64();
            manager.Update(t, None)?;
            let fade_value = fade.GetValue()?;
            let slide_value = slide.GetValue()?;

            let filled = (fade_value * 30.0).round() as usize;
            let bar: String = std::iter::repeat_n('#', filled)
                .chain(std::iter::repeat_n('-', 30 - filled))
                .collect();
            print!("\r[{bar}] fade={fade_value:.3} slide={slide_value:6.2} t={t:.2}s");
            std::io::stdout().flush().ok();

            if t >= 2.05 {
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        println!(
            "\nDone: fade={:.3} slide={:.2}",
            fade.GetValue()?,
            slide.GetValue()?
        );
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("wam-poc runs on Windows only.");
}
