use std::cell::Cell;
use std::marker::PhantomData;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::Audio::{
    AUDCLNT_E_UNSUPPORTED_FORMAT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, IAudioClient, IAudioRenderClient, IMMDeviceEnumerator,
    MMDeviceEnumerator, WAVE_FORMAT_PCM, WAVEFORMATEX, eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize,
};
use windows::core::Error as WinError;

use crate::error::AudioError;
use crate::pcm::{BYTES_PER_FRAME, SampleRate};
use crate::wait::sleep_interruptibly;
use crate::wasapi_format::{
    FormatNegotiation, HrCode, WaveFormatFields, describe, initialize_error,
};
use crate::{Playback, StopSignal};

// Samples reach the endpoint buffer by memcpy, which only produces the
// little-endian PCM the format declares on a little-endian target.
const _: () = assert!(cfg!(target_endian = "little"));
const _: () = assert!(crate::wasapi_format::S_OK == windows::Win32::Foundation::S_OK.0);
const _: () =
    assert!(crate::wasapi_format::AUDCLNT_E_UNSUPPORTED_FORMAT == AUDCLNT_E_UNSUPPORTED_FORMAT.0);
const _: () = assert!(
    crate::wasapi_format::AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM == AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
);
const _: () = assert!(
    crate::wasapi_format::AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
        == AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
);
const _: () = assert!(crate::wasapi_format::WAVE_FORMAT_PCM as u32 == WAVE_FORMAT_PCM);

// 100-nanosecond units, the unit IAudioClient::Initialize takes. A tenth of a
// second of slack keeps the endpoint fed across scheduler hiccups without
// delaying a stop request by more than that.
const BUFFER_DURATION_HNS: i64 = 100 * 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamState {
    Stopped,
    Running,
}

enum ComApartment {
    Owned,
    Inherited,
}

impl ComApartment {
    fn enter() -> Result<Self, AudioError> {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            return Ok(Self::Inherited);
        }

        if hr.is_err() {
            return Err(AudioError::DeviceUnavailable {
                detail: describe("CoInitializeEx", HrCode(hr.0), &hr.message()),
            });
        }

        Ok(Self::Owned)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if matches!(self, Self::Owned) {
            unsafe { CoUninitialize() };
        }
    }
}

pub struct WasapiBackend {
    render: IAudioRenderClient,
    client: IAudioClient,
    buffer_frames: u32,
    rate: SampleRate,
    state: Cell<StreamState>,
    _apartment: ComApartment,
    // COM objects and the apartment belong to the thread that created them.
    _not_send: PhantomData<*const ()>,
}

impl WasapiBackend {
    pub fn open(rate: SampleRate) -> Result<Self, AudioError> {
        let apartment = ComApartment::enter()?;

        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .map_err(|err| device_error("CoCreateInstance(MMDeviceEnumerator)", &err))?;
        let endpoint = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
            .map_err(|err| device_error("IMMDeviceEnumerator::GetDefaultAudioEndpoint", &err))?;
        let client: IAudioClient = unsafe { endpoint.Activate(CLSCTX_ALL, None) }
            .map_err(|err| device_error("IMMDevice::Activate(IAudioClient)", &err))?;
        let device_hz = mix_format_rate_hz(&client)?;

        let format = wave_format(rate);
        let negotiation = FormatNegotiation::from_is_format_supported(HrCode(
            unsafe { client.IsFormatSupported(AUDCLNT_SHAREMODE_SHARED, &format, None) }.0,
        ));

        unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                negotiation.stream_flags(),
                BUFFER_DURATION_HNS,
                0,
                &format,
                None,
            )
        }
        .map_err(|err| initialize_error(HrCode(err.code().0), &err.message(), rate, device_hz))?;

        let buffer_frames = unsafe { client.GetBufferSize() }
            .map_err(|err| device_error("IAudioClient::GetBufferSize", &err))?;
        let render: IAudioRenderClient = unsafe { client.GetService() }
            .map_err(|err| device_error("IAudioClient::GetService(IAudioRenderClient)", &err))?;

        Ok(Self {
            render,
            client,
            buffer_frames,
            rate,
            state: Cell::new(StreamState::Stopped),
            _apartment: apartment,
            _not_send: PhantomData,
        })
    }

    pub fn sample_rate(&self) -> SampleRate {
        self.rate
    }

    pub fn write(
        &self,
        samples: &[i16],
        stop: &mut dyn StopSignal,
    ) -> Result<Playback, AudioError> {
        self.start()?;

        let mut submitted = 0;
        while submitted < samples.len() {
            if stop.should_stop() {
                self.discard()?;
                return Ok(Playback::Interrupted);
            }

            let free = self.buffer_frames - self.padding_frames()?;
            if free == 0 {
                if sleep_interruptibly(self.half_buffer_duration(), stop) == Playback::Interrupted {
                    self.discard()?;
                    return Ok(Playback::Interrupted);
                }
                continue;
            }

            let count = (free as usize).min(samples.len() - submitted);
            self.submit(&samples[submitted..submitted + count])?;
            submitted += count;
        }

        Ok(Playback::Completed)
    }

    pub fn drain(&self) -> Result<(), AudioError> {
        if self.state.get() == StreamState::Stopped {
            return Ok(());
        }

        loop {
            let padding = self.padding_frames()?;
            if padding == 0 {
                return self.stop();
            }

            thread::sleep(
                self.rate
                    .duration_of(padding as usize)
                    .max(Duration::from_millis(1)),
            );
        }
    }

    pub fn discard(&self) -> Result<(), AudioError> {
        if self.state.get() == StreamState::Stopped {
            return Ok(());
        }

        self.stop()?;
        unsafe { self.client.Reset() }.map_err(|err| device_error("IAudioClient::Reset", &err))
    }

    fn start(&self) -> Result<(), AudioError> {
        if self.state.get() == StreamState::Running {
            return Ok(());
        }

        unsafe { self.client.Start() }.map_err(|err| device_error("IAudioClient::Start", &err))?;
        self.state.set(StreamState::Running);
        Ok(())
    }

    fn stop(&self) -> Result<(), AudioError> {
        unsafe { self.client.Stop() }.map_err(|err| device_error("IAudioClient::Stop", &err))?;
        self.state.set(StreamState::Stopped);
        Ok(())
    }

    fn padding_frames(&self) -> Result<u32, AudioError> {
        unsafe { self.client.GetCurrentPadding() }
            .map_err(|err| write_error("IAudioClient::GetCurrentPadding", &err))
    }

    fn half_buffer_duration(&self) -> Duration {
        self.rate.duration_of(self.buffer_frames as usize / 2)
    }

    fn submit(&self, samples: &[i16]) -> Result<(), AudioError> {
        let frames = samples.len() as u32;
        let buffer = unsafe { self.render.GetBuffer(frames) }
            .map_err(|err| write_error("IAudioRenderClient::GetBuffer", &err))?;
        debug_assert!(!buffer.is_null());

        unsafe {
            std::ptr::copy_nonoverlapping(
                samples.as_ptr().cast::<u8>(),
                buffer,
                samples.len() * BYTES_PER_FRAME,
            );
        }

        unsafe { self.render.ReleaseBuffer(frames, 0) }
            .map_err(|err| write_error("IAudioRenderClient::ReleaseBuffer", &err))
    }
}

impl Drop for WasapiBackend {
    fn drop(&mut self) {
        if self.state.get() == StreamState::Running {
            let _ = unsafe { self.client.Stop() };
        }
    }
}

fn wave_format(rate: SampleRate) -> WAVEFORMATEX {
    let fields = WaveFormatFields::mono_s16(rate);
    WAVEFORMATEX {
        wFormatTag: fields.format_tag,
        nChannels: fields.channels,
        nSamplesPerSec: fields.samples_per_sec,
        nAvgBytesPerSec: fields.avg_bytes_per_sec,
        nBlockAlign: fields.block_align,
        wBitsPerSample: fields.bits_per_sample,
        cbSize: fields.cb_size,
    }
}

fn mix_format_rate_hz(client: &IAudioClient) -> Result<u32, AudioError> {
    let format = unsafe { client.GetMixFormat() }
        .map_err(|err| device_error("IAudioClient::GetMixFormat", &err))?;
    debug_assert!(!format.is_null());

    let rate_hz = unsafe { (*format).nSamplesPerSec };
    unsafe { CoTaskMemFree(Some(format.cast())) };
    Ok(rate_hz)
}

fn device_error(operation: &str, err: &WinError) -> AudioError {
    AudioError::DeviceUnavailable {
        detail: describe(operation, HrCode(err.code().0), &err.message()),
    }
}

fn write_error(operation: &str, err: &WinError) -> AudioError {
    AudioError::WriteFailed {
        detail: describe(operation, HrCode(err.code().0), &err.message()),
    }
}
