use std::fmt;

use crate::error::AudioError;
use crate::pcm::{BITS_PER_SAMPLE, BYTES_PER_FRAME, CHANNELS, SampleRate};

pub const WAVE_FORMAT_PCM: u16 = 1;
pub const S_OK: i32 = 0;
pub const AUDCLNT_E_UNSUPPORTED_FORMAT: i32 = 0x8889_0008u32 as i32;
pub const AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM: u32 = 0x8000_0000;
pub const AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HrCode(pub i32);

impl fmt::Display for HrCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

// Shared-mode WASAPI only mixes the endpoint's own format. Anything else goes
// through the OS resampler, which we ask for by name rather than rolling our own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatNegotiation {
    Native,
    OsResampled,
}

impl FormatNegotiation {
    pub fn from_is_format_supported(hr: HrCode) -> Self {
        if hr == HrCode(S_OK) {
            return Self::Native;
        }

        Self::OsResampled
    }

    pub fn stream_flags(self) -> u32 {
        match self {
            Self::Native => 0,
            Self::OsResampled => {
                AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaveFormatFields {
    pub format_tag: u16,
    pub channels: u16,
    pub samples_per_sec: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub cb_size: u16,
}

impl WaveFormatFields {
    pub fn mono_s16(rate: SampleRate) -> Self {
        let block_align = BYTES_PER_FRAME as u16;
        Self {
            format_tag: WAVE_FORMAT_PCM,
            channels: CHANNELS,
            samples_per_sec: rate.hz(),
            avg_bytes_per_sec: rate.hz() * u32::from(block_align),
            block_align,
            bits_per_sample: BITS_PER_SAMPLE,
            cb_size: 0,
        }
    }
}

pub fn describe(operation: &str, hr: HrCode, message: &str) -> String {
    format!("{operation} failed: {message} ({hr})")
}

pub fn initialize_error(
    hr: HrCode,
    message: &str,
    requested: SampleRate,
    device_hz: u32,
) -> AudioError {
    if hr == HrCode(AUDCLNT_E_UNSUPPORTED_FORMAT) {
        return AudioError::UnsupportedFormat {
            requested_hz: requested.hz(),
            device_hz,
        };
    }

    AudioError::DeviceUnavailable {
        detail: describe("IAudioClient::Initialize", hr, message),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AUDCLNT_E_UNSUPPORTED_FORMAT, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
        AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, FormatNegotiation, HrCode, S_OK, WAVE_FORMAT_PCM,
        WaveFormatFields, describe, initialize_error,
    };
    use crate::error::AudioError;
    use crate::pcm::SampleRate;

    const S_FALSE: i32 = 1;

    #[test]
    fn only_an_exact_match_skips_the_os_resampler() {
        assert_eq!(
            FormatNegotiation::from_is_format_supported(HrCode(S_OK)),
            FormatNegotiation::Native
        );
        assert_eq!(FormatNegotiation::Native.stream_flags(), 0);

        for hr in [S_FALSE, AUDCLNT_E_UNSUPPORTED_FORMAT] {
            assert_eq!(
                FormatNegotiation::from_is_format_supported(HrCode(hr)),
                FormatNegotiation::OsResampled
            );
        }
        assert_eq!(
            FormatNegotiation::OsResampled.stream_flags(),
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
        );
    }

    #[test]
    fn mono_s16_format_describes_two_byte_frames() {
        let format = WaveFormatFields::mono_s16(SampleRate::try_from(22_050).unwrap());
        assert_eq!(
            format,
            WaveFormatFields {
                format_tag: WAVE_FORMAT_PCM,
                channels: 1,
                samples_per_sec: 22_050,
                avg_bytes_per_sec: 44_100,
                block_align: 2,
                bits_per_sample: 16,
                cb_size: 0,
            }
        );
    }

    #[test]
    fn a_rejected_format_reports_both_rates() {
        let requested = SampleRate::try_from(22_050).unwrap();
        assert_eq!(
            initialize_error(
                HrCode(AUDCLNT_E_UNSUPPORTED_FORMAT),
                "Unsupported format",
                requested,
                48_000
            ),
            AudioError::UnsupportedFormat {
                requested_hz: 22_050,
                device_hz: 48_000,
            }
        );
    }

    #[test]
    fn other_initialize_failures_keep_the_hresult() {
        let requested = SampleRate::try_from(22_050).unwrap();
        assert_eq!(
            initialize_error(
                HrCode(0x8889_0001u32 as i32),
                "Not initialized",
                requested,
                0
            ),
            AudioError::DeviceUnavailable {
                detail: "IAudioClient::Initialize failed: Not initialized (0x88890001)".to_string(),
            }
        );
    }

    #[test]
    fn descriptions_name_the_failing_call() {
        assert_eq!(
            describe(
                "IAudioRenderClient::GetBuffer",
                HrCode(0x8889_0006u32 as i32),
                "Buffer too large"
            ),
            "IAudioRenderClient::GetBuffer failed: Buffer too large (0x88890006)"
        );
    }
}
