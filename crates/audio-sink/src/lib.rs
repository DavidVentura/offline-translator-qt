mod error;
mod pcm;
#[cfg(windows)]
mod wait;

#[cfg(any(windows, test))]
mod wasapi_format;

#[cfg(target_os = "linux")]
mod pulse;
#[cfg(windows)]
mod wasapi;

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("audio-sink has no playback backend for this target");

#[cfg(target_os = "linux")]
use crate::pulse::PulseBackend as Backend;
#[cfg(windows)]
use crate::wasapi::WasapiBackend as Backend;

use std::time::Duration;

pub use crate::error::AudioError;
pub use crate::pcm::{PcmSamples, SampleRate};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Playback {
    Completed,
    Interrupted,
}

pub trait StopSignal {
    fn should_stop(&mut self) -> bool;
}

impl<F> StopSignal for F
where
    F: FnMut() -> bool,
{
    fn should_stop(&mut self) -> bool {
        self()
    }
}

pub struct AudioSink {
    backend: Backend,
}

impl AudioSink {
    pub fn open(rate: SampleRate) -> Result<Self, AudioError> {
        Ok(Self {
            backend: Backend::open(rate)?,
        })
    }

    pub fn sample_rate(&self) -> SampleRate {
        self.backend.sample_rate()
    }

    pub fn play(
        &self,
        audio: PcmSamples<'_>,
        stop: &mut impl StopSignal,
    ) -> Result<Playback, AudioError> {
        let stream_rate = self.sample_rate();
        if audio.rate() != stream_rate {
            return Err(AudioError::RateMismatch {
                stream_hz: stream_rate.hz(),
                audio_hz: audio.rate().hz(),
            });
        }

        self.backend.write(audio.samples(), stop)
    }

    pub fn play_silence(
        &self,
        duration: Duration,
        stop: &mut impl StopSignal,
    ) -> Result<Playback, AudioError> {
        let silence = vec![0i16; self.sample_rate().frames_in(duration)];
        self.backend.write(&silence, stop)
    }

    pub fn drain(&self) -> Result<(), AudioError> {
        self.backend.drain()
    }

    pub fn discard(&self) -> Result<(), AudioError> {
        self.backend.discard()
    }
}
