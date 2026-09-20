//! Probes a video file through the decoder, both paces — a quick way to know
//! whether GStreamer can play a file before putting it in a patch.
//!
//!   cargo run -p vybe-video --example probe -- clip.mov

use std::path::Path;
use std::time::{Duration, Instant};

use vybe::clip::{Decoder, Pace};
use vybe_video::GStreamer;

fn main() -> Result<(), String> {
    let path = std::env::args().nth(1).ok_or("usage: probe <video file>")?;
    let decoder = GStreamer::new()?;

    // Offline: the frame that belongs at each time, exactly.
    let mut clip = decoder.open(Path::new(&path), false, Pace::Offline)?;
    for time in [0.0, 1.0, 5.0] {
        let started = Instant::now();
        match clip.frame(time) {
            Some(f) => println!(
                "offline  t={time:>4.1}s  {}x{}  ({} ms)",
                f.width,
                f.height,
                started.elapsed().as_millis()
            ),
            None => println!("offline  t={time:>4.1}s  no frame"),
        }
    }
    drop(clip);

    // Live: two seconds on its own clock, with sound (turned low).
    let mut clip = decoder.open(Path::new(&path), false, Pace::Live)?;
    clip.set_volume(0.05);
    let (started, mut frames) = (Instant::now(), 0);
    while started.elapsed() < Duration::from_secs(2) {
        frames += usize::from(clip.frame(0.0).is_some());
        std::thread::sleep(Duration::from_millis(4));
    }
    println!("live     {frames} frames in 2 s  (done: {})", clip.done());
    Ok(())
}
