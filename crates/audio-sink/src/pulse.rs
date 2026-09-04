use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::time::Duration;

use crate::error::AudioError;
use crate::pcm::{BYTES_PER_FRAME, CHANNELS, SampleRate};
use crate::{Playback, StopSignal};

const PA_STREAM_PLAYBACK: c_int = 1;
const PA_SAMPLE_S16LE: c_int = 3;
const PA_BUFFER_ATTR_DEFAULT: u32 = u32::MAX;
const PLAYBACK_CHUNK_FRAMES: usize = 2_048;

/// The server keeps this much audio queued and blocks writes beyond it, which paces the writer
/// without a client-side clock. Deep enough to ride out scheduler jitter between writes; a stop
/// flushes the queue, so it never delays cancellation.
const TARGET_BUFFER: Duration = Duration::from_millis(400);
/// Playback starts once this much is queued rather than waiting for the full target, so the
/// first word is not held back.
const PREBUFFER: Duration = Duration::from_millis(40);

#[repr(C)]
struct PaSampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}

#[repr(C)]
struct PaBufferAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}

#[repr(C)]
struct PaSimple {
    _private: [u8; 0],
}

#[link(name = "pulse-simple")]
unsafe extern "C" {
    fn pa_simple_new(
        server: *const c_char,
        name: *const c_char,
        dir: c_int,
        dev: *const c_char,
        stream_name: *const c_char,
        ss: *const PaSampleSpec,
        map: *const c_void,
        attr: *const PaBufferAttr,
        error: *mut c_int,
    ) -> *mut PaSimple;
    fn pa_simple_free(s: *mut PaSimple);
    fn pa_simple_write(
        s: *mut PaSimple,
        data: *const c_void,
        bytes: usize,
        error: *mut c_int,
    ) -> c_int;
    fn pa_simple_drain(s: *mut PaSimple, error: *mut c_int) -> c_int;
    fn pa_simple_flush(s: *mut PaSimple, error: *mut c_int) -> c_int;
}

#[link(name = "pulse")]
unsafe extern "C" {
    fn pa_strerror(error: c_int) -> *const c_char;
}

pub struct PulseBackend {
    handle: *mut PaSimple,
    rate: SampleRate,
}

impl PulseBackend {
    pub fn open(rate: SampleRate) -> Result<Self, AudioError> {
        let sample_spec = PaSampleSpec {
            format: PA_SAMPLE_S16LE,
            rate: rate.hz(),
            channels: CHANNELS as u8,
        };
        let buffer_attr = PaBufferAttr {
            maxlength: PA_BUFFER_ATTR_DEFAULT,
            tlength: bytes_for(rate, TARGET_BUFFER),
            prebuf: bytes_for(rate, PREBUFFER),
            minreq: PA_BUFFER_ATTR_DEFAULT,
            fragsize: PA_BUFFER_ATTR_DEFAULT,
        };
        let mut error = 0;

        let handle = unsafe {
            pa_simple_new(
                std::ptr::null(),
                c"Offline translator".as_ptr(),
                PA_STREAM_PLAYBACK,
                std::ptr::null(),
                c"Text to speech".as_ptr(),
                &sample_spec,
                std::ptr::null(),
                &buffer_attr,
                &mut error,
            )
        };

        if handle.is_null() {
            return Err(AudioError::DeviceUnavailable {
                detail: format!(
                    "failed to connect to PulseAudio: {}",
                    pulse_error_message(error)
                ),
            });
        }

        Ok(Self { handle, rate })
    }

    pub fn sample_rate(&self) -> SampleRate {
        self.rate
    }

    pub fn write(
        &self,
        samples: &[i16],
        stop: &mut dyn StopSignal,
    ) -> Result<Playback, AudioError> {
        // Each write blocks until the server has room under TARGET_BUFFER, so the
        // stop check between chunks runs about once per chunk of playback.
        for chunk in samples.chunks(PLAYBACK_CHUNK_FRAMES) {
            if stop.should_stop() {
                self.discard()?;
                return Ok(Playback::Interrupted);
            }

            self.write_chunk(chunk)?;
        }

        Ok(Playback::Completed)
    }

    pub fn drain(&self) -> Result<(), AudioError> {
        let mut error = 0;
        if unsafe { pa_simple_drain(self.handle, &mut error) } < 0 {
            return Err(AudioError::WriteFailed {
                detail: format!("PulseAudio drain failed: {}", pulse_error_message(error)),
            });
        }

        Ok(())
    }

    pub fn discard(&self) -> Result<(), AudioError> {
        let mut error = 0;
        if unsafe { pa_simple_flush(self.handle, &mut error) } < 0 {
            return Err(AudioError::WriteFailed {
                detail: format!("PulseAudio flush failed: {}", pulse_error_message(error)),
            });
        }

        Ok(())
    }

    fn write_chunk(&self, samples: &[i16]) -> Result<(), AudioError> {
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        debug_assert_eq!(bytes.len(), samples.len() * BYTES_PER_FRAME);

        let mut error = 0;
        let written = unsafe {
            pa_simple_write(
                self.handle,
                bytes.as_ptr().cast::<c_void>(),
                bytes.len(),
                &mut error,
            )
        };

        if written < 0 {
            return Err(AudioError::WriteFailed {
                detail: format!("PulseAudio write failed: {}", pulse_error_message(error)),
            });
        }

        Ok(())
    }
}

impl Drop for PulseBackend {
    fn drop(&mut self) {
        unsafe { pa_simple_free(self.handle) };
    }
}

fn bytes_for(rate: SampleRate, duration: Duration) -> u32 {
    u32::try_from(rate.frames_in(duration) * BYTES_PER_FRAME).expect("buffer target fits in u32")
}

fn pulse_error_message(error: c_int) -> String {
    let message = unsafe { pa_strerror(error) };
    if message.is_null() {
        return format!("unknown PulseAudio error ({error})");
    }

    unsafe { CStr::from_ptr(message) }
        .to_string_lossy()
        .into_owned()
}
