use std::f64::consts::TAU;
use std::time::Duration;

use audio_sink::{AudioError, AudioSink, PcmSamples, SampleRate};

const RATE_HZ: i32 = 44_100;
const TONE_HZ: f64 = 440.0;
const AMPLITUDE: f64 = 0.25;
const FADE: Duration = Duration::from_millis(5);

fn tone(rate: SampleRate, duration: Duration) -> Vec<i16> {
    let frames = rate.frames_in(duration);
    let fade_frames = rate.frames_in(FADE).max(1);
    let step = TAU * TONE_HZ / f64::from(rate.hz());

    (0..frames)
        .map(|frame| {
            let ramp_in = (frame as f64 / fade_frames as f64).min(1.0);
            let ramp_out = ((frames - frame) as f64 / fade_frames as f64).min(1.0);
            let envelope = AMPLITUDE * ramp_in.min(ramp_out);
            ((step * frame as f64).sin() * envelope * f64::from(i16::MAX)) as i16
        })
        .collect()
}

fn main() -> Result<(), AudioError> {
    let rate = SampleRate::try_from(RATE_HZ)?;
    let samples = tone(rate, Duration::from_secs(1));
    println!("generated {} frames at {} Hz", samples.len(), rate.hz());

    let sink = AudioSink::open(rate)?;
    println!("opened sink");

    let mut keep_playing = || false;
    let played = sink.play(PcmSamples::new(rate, &samples), &mut keep_playing)?;
    println!("play returned {played:?}");

    let paused = sink.play_silence(Duration::from_millis(200), &mut keep_playing)?;
    println!("play_silence returned {paused:?}");

    sink.drain()?;
    println!("drained");
    Ok(())
}
