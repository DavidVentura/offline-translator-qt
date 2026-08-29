use std::thread;
use std::time::{Duration, Instant};

use crate::{Playback, StopSignal};

/// Cancellation is only as responsive as this slice, so it stays well under the
/// gap a listener would notice after pressing stop.
const SLICE: Duration = Duration::from_millis(20);

pub fn sleep_interruptibly(duration: Duration, stop: &mut dyn StopSignal) -> Playback {
    let deadline = Instant::now() + duration;

    loop {
        if stop.should_stop() {
            return Playback::Interrupted;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Playback::Completed;
        }

        thread::sleep(remaining.min(SLICE));
    }
}
