use std::num::NonZeroU32;
use std::time::Duration;

use crate::error::AudioError;

pub const CHANNELS: u16 = 1;
pub const BITS_PER_SAMPLE: u16 = 16;
pub const BYTES_PER_FRAME: usize = (BITS_PER_SAMPLE as usize / 8) * CHANNELS as usize;

const NANOS_PER_SEC: u128 = 1_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleRate(NonZeroU32);

impl SampleRate {
    pub fn hz(self) -> u32 {
        self.0.get()
    }

    pub fn frames_in(self, duration: Duration) -> usize {
        let frames = duration.as_nanos() * u128::from(self.hz()) / NANOS_PER_SEC;
        usize::try_from(frames).expect("requested more audio frames than fit in memory")
    }

    // Rounded up, so waiting out a chunk never returns before the audio it
    // covers has been consumed.
    pub fn duration_of(self, frames: usize) -> Duration {
        let hz = u128::from(self.hz());
        let nanos = (frames as u128 * NANOS_PER_SEC).div_ceil(hz);
        Duration::from_nanos(u64::try_from(nanos).expect("audio longer than 584 years"))
    }
}

impl TryFrom<i32> for SampleRate {
    type Error = AudioError;

    fn try_from(hz: i32) -> Result<Self, AudioError> {
        u32::try_from(hz)
            .ok()
            .and_then(NonZeroU32::new)
            .map(Self)
            .ok_or(AudioError::InvalidSampleRate { hz })
    }
}

// Tagged with the rate it was produced at so a sink can reject audio that does
// not belong to the stream it opened.
#[derive(Clone, Copy, Debug)]
pub struct PcmSamples<'a> {
    rate: SampleRate,
    samples: &'a [i16],
}

impl<'a> PcmSamples<'a> {
    pub fn new(rate: SampleRate, samples: &'a [i16]) -> Self {
        Self { rate, samples }
    }

    pub fn rate(self) -> SampleRate {
        self.rate
    }

    pub fn samples(self) -> &'a [i16] {
        self.samples
    }
}

#[cfg(test)]
mod tests {
    use super::{BYTES_PER_FRAME, SampleRate};
    use crate::error::AudioError;
    use std::time::Duration;

    fn rate(hz: i32) -> SampleRate {
        SampleRate::try_from(hz).unwrap()
    }

    #[test]
    fn a_frame_is_one_little_endian_i16() {
        assert_eq!(BYTES_PER_FRAME, size_of::<i16>());
    }

    #[test]
    fn non_positive_rates_are_rejected() {
        assert_eq!(
            SampleRate::try_from(0),
            Err(AudioError::InvalidSampleRate { hz: 0 })
        );
        assert_eq!(
            SampleRate::try_from(-22_050),
            Err(AudioError::InvalidSampleRate { hz: -22_050 })
        );
        assert_eq!(rate(22_050).hz(), 22_050);
    }

    #[test]
    fn frame_counts_truncate_to_whole_frames() {
        assert_eq!(rate(22_050).frames_in(Duration::from_secs(1)), 22_050);
        assert_eq!(rate(48_000).frames_in(Duration::from_millis(250)), 12_000);
        assert_eq!(rate(22_050).frames_in(Duration::from_millis(1)), 22);
        assert_eq!(rate(22_050).frames_in(Duration::ZERO), 0);
        assert_eq!(rate(8_000).frames_in(Duration::from_micros(1)), 0);
    }

    #[test]
    fn frame_counts_survive_long_pauses() {
        assert_eq!(
            rate(48_000).frames_in(Duration::from_secs(3_600)),
            172_800_000
        );
    }

    #[test]
    fn durations_and_frame_counts_round_trip() {
        let rate = rate(44_100);
        assert_eq!(rate.duration_of(44_100), Duration::from_secs(1));
        assert_eq!(rate.duration_of(0), Duration::ZERO);
        assert_eq!(rate.frames_in(rate.duration_of(2_048)), 2_048);
        assert_eq!(rate.duration_of(2_048), Duration::from_nanos(46_439_910));
    }
}
