use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioError {
    InvalidSampleRate { hz: i32 },
    RateMismatch { stream_hz: u32, audio_hz: u32 },
    DeviceUnavailable { detail: String },
    UnsupportedFormat { requested_hz: u32, device_hz: u32 },
    WriteFailed { detail: String },
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleRate { hz } => write!(f, "invalid sample rate: {hz} Hz"),
            Self::RateMismatch {
                stream_hz,
                audio_hz,
            } => write!(
                f,
                "mismatched sample rate for playback stream: expected {stream_hz} Hz, got {audio_hz} Hz"
            ),
            Self::DeviceUnavailable { detail } => write!(f, "audio device unavailable: {detail}"),
            Self::UnsupportedFormat {
                requested_hz,
                device_hz,
            } => write!(
                f,
                "device rejected 16-bit mono at {requested_hz} Hz; it renders at {device_hz} Hz"
            ),
            Self::WriteFailed { detail } => write!(f, "audio write failed: {detail}"),
        }
    }
}

impl std::error::Error for AudioError {}

#[cfg(test)]
mod tests {
    use super::AudioError;

    #[test]
    fn messages_name_the_offending_rates() {
        assert_eq!(
            AudioError::InvalidSampleRate { hz: -1 }.to_string(),
            "invalid sample rate: -1 Hz"
        );
        assert_eq!(
            AudioError::RateMismatch {
                stream_hz: 22_050,
                audio_hz: 16_000,
            }
            .to_string(),
            "mismatched sample rate for playback stream: expected 22050 Hz, got 16000 Hz"
        );
        assert_eq!(
            AudioError::UnsupportedFormat {
                requested_hz: 22_050,
                device_hz: 48_000,
            }
            .to_string(),
            "device rejected 16-bit mono at 22050 Hz; it renders at 48000 Hz"
        );
    }
}
