//! No-op audio sink for platforms without the PulseAudio backend. Keeps the TTS
//! path compiling; real playback on those platforms needs a native backend
//! (e.g. WASAPI/cpal) wired in here.

use translator::PcmAudio;

pub struct PulsePlaybackStream;

impl PulsePlaybackStream {
    pub fn new(_sample_rate: i32) -> Result<Self, String> {
        Ok(Self)
    }

    pub fn write_audio<F>(&self, _audio: &PcmAudio, _should_stop: &mut F) -> Result<(), String>
    where
        F: FnMut() -> bool,
    {
        Ok(())
    }

    pub fn write_pause_ms<F>(&self, _duration_ms: i32, _should_stop: &mut F) -> Result<(), String>
    where
        F: FnMut() -> bool,
    {
        Ok(())
    }

    pub fn flush(&self) -> Result<(), String> {
        Ok(())
    }
}
