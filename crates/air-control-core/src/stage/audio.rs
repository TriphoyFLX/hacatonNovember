use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

use rodio::{buffer::SamplesBuffer, Decoder, OutputStream, OutputStreamHandle, Sink, Source};

const GUITAR_SAMPLE: &[u8] =
    include_bytes!("../../../note-one-shot-pluck-guitar-5_C_major (1).wav");
const PIANO_SAMPLE: &[u8] = include_bytes!("../../../bright-power-grand-piano-one-shot_C.wav");
const DRUM_KICK_SAMPLE: &[u8] = include_bytes!("../../../one-shot-kick.wav");
const DRUM_SNARE_SAMPLE: &[u8] = include_bytes!("../../../snare-basic_C_minor.wav");
const DRUM_CLOSED_HAT_SAMPLE: &[u8] =
    include_bytes!("../../../phonk-closed-hi-hats-thin_C_minor.wav");
const DRUM_OPEN_HAT_SAMPLE: &[u8] = include_bytes!("../../../lil-jon-type-open-hi-hat.wav");

const SAMPLE_ROOT_NOTE: u8 = 60; // C4 for pitched instrument one-shots
/// Guitar one-shot sits near mid-neck; treat open low E (40) relative to E3 so bass stays audible.
const GUITAR_SAMPLE_ROOT: u8 = 52; // E3
const GUITAR_MAX_SECONDS: f32 = 1.15;
const PIANO_MAX_SECONDS: f32 = 1.35;
const DRUM_MAX_SECONDS: f32 = 1.8;
const POOL_SIZE: usize = 12;

pub(super) struct GuitarAudio {
    commands: SyncSender<PlayCommand>,
}

pub(super) struct PianoAudio {
    commands: SyncSender<PlayCommand>,
}

/// Kit one-shots: 0 kick · 1 snare · 2 closed hat · 3 open hat (native pitch).
pub(super) struct DrumAudio {
    commands: SyncSender<PlayCommand>,
}

enum PlayCommand {
    Play { note: u8, velocity: u8, tone: u8 },
    Stop,
}

struct SampleRenderer {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    tones: Vec<SamplesBuffer<f32>>,
    sinks: Vec<Sink>,
    next: usize,
    root_note: u8,
}

impl GuitarAudio {
    pub(super) fn open() -> Result<Self, String> {
        open_player(
            "air-control-guitar-audio",
            build_guitar_tones,
            GUITAR_SAMPLE_ROOT,
        )
        .map(|commands| Self { commands })
    }

    pub(super) fn play(&self, note: u8, velocity: u8, tone: u8) -> Result<(), String> {
        send_play(&self.commands, note, velocity, tone)
    }

    pub(super) fn stop_all(&self) {
        send_stop(&self.commands, "guitar");
    }
}

impl PianoAudio {
    pub(super) fn open() -> Result<Self, String> {
        open_player(
            "air-control-piano-audio",
            build_piano_tones,
            SAMPLE_ROOT_NOTE,
        )
        .map(|commands| Self { commands })
    }

    pub(super) fn play(&self, note: u8, velocity: u8, tone: u8) -> Result<(), String> {
        send_play(&self.commands, note, velocity, tone)
    }

    pub(super) fn stop_all(&self) {
        send_stop(&self.commands, "piano");
    }
}

impl DrumAudio {
    pub(super) fn open() -> Result<Self, String> {
        open_player(
            "air-control-drum-audio",
            build_drum_tones,
            SAMPLE_ROOT_NOTE,
        )
        .map(|commands| Self { commands })
    }

    /// `kit`: 0 kick · 1 snare · 2 closed hat · 3 open hat. Pitch is fixed.
    pub(super) fn play(&self, kit: u8, velocity: u8) -> Result<(), String> {
        send_play(&self.commands, SAMPLE_ROOT_NOTE, velocity, kit)
    }

    pub(super) fn stop_all(&self) {
        send_stop(&self.commands, "drums");
    }
}

fn open_player(
    thread_name: &str,
    build_tones: fn() -> Result<(u16, u32, Vec<Vec<f32>>), String>,
    root_note: u8,
) -> Result<SyncSender<PlayCommand>, String> {
    let (commands, receiver) = mpsc::sync_channel(96);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let name = thread_name.to_string();
    thread::Builder::new()
        .name(name)
        .spawn(move || sample_thread(receiver, ready_tx, build_tones, root_note))
        .map_err(|error| error.to_string())?;
    ready_rx.recv().map_err(|error| error.to_string())??;
    Ok(commands)
}

fn send_play(
    commands: &SyncSender<PlayCommand>,
    note: u8,
    velocity: u8,
    tone: u8,
) -> Result<(), String> {
    let command = PlayCommand::Play {
        note,
        velocity,
        tone,
    };
    commands
        .try_send(command)
        .or_else(|_| {
            commands.send(PlayCommand::Play {
                note,
                velocity,
                tone,
            })
        })
        .map_err(|error| error.to_string())
}

fn send_stop(commands: &SyncSender<PlayCommand>, label: &str) {
    if commands.try_send(PlayCommand::Stop).is_err() && commands.send(PlayCommand::Stop).is_err() {
        tracing::warn!(%label, "sample audio thread is no longer available");
    }
}

fn sample_thread(
    receiver: Receiver<PlayCommand>,
    ready: mpsc::SyncSender<Result<(), String>>,
    build_tones: fn() -> Result<(u16, u32, Vec<Vec<f32>>), String>,
    root_note: u8,
) {
    let mut renderer = match SampleRenderer::open(build_tones, root_note) {
        Ok(renderer) => {
            if ready.send(Ok(())).is_err() {
                return;
            }
            renderer
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };

    while let Ok(command) = receiver.recv() {
        match command {
            PlayCommand::Play {
                note,
                velocity,
                tone,
            } => {
                if let Err(error) = renderer.play(note, velocity, tone) {
                    tracing::warn!(%error, "sample playback failed");
                }
            }
            PlayCommand::Stop => renderer.stop_all(),
        }
    }
}

impl SampleRenderer {
    fn open(
        build_tones: fn() -> Result<(u16, u32, Vec<Vec<f32>>), String>,
        root_note: u8,
    ) -> Result<Self, String> {
        let (stream, stream_handle) =
            OutputStream::try_default().map_err(|error| error.to_string())?;
        let (channels, sample_rate, tone_bufs) = build_tones()?;
        let tones = tone_bufs
            .into_iter()
            .map(|samples| SamplesBuffer::new(channels, sample_rate, samples))
            .collect();

        let mut sinks = Vec::with_capacity(POOL_SIZE);
        for _ in 0..POOL_SIZE {
            sinks.push(Sink::try_new(&stream_handle).map_err(|error| error.to_string())?);
        }

        Ok(Self {
            _stream: stream,
            stream_handle,
            tones,
            sinks,
            next: 0,
            root_note,
        })
    }

    fn play(&mut self, note: u8, velocity: u8, tone: u8) -> Result<(), String> {
        if self.sinks.is_empty() {
            self.sinks
                .push(Sink::try_new(&self.stream_handle).map_err(|error| error.to_string())?);
        }
        let index = self.next % self.sinks.len();
        self.next = self.next.wrapping_add(1);
        let sink = &self.sinks[index];
        sink.stop();
        sink.clear();

        let speed = playback_speed_from(note, self.root_note).clamp(0.42, 2.6);
        // Low E was nearly silent: slow playback + short one-shot. Boost the bottom.
        let low_boost = if speed < 0.72 {
            (0.72 / speed).sqrt().clamp(1.0, 1.55)
        } else {
            1.0
        };
        let gain = (f32::from(velocity.min(127)) / 127.0).powf(1.1) * 1.08 * low_boost;
        let Some(sample) = self.tones.get(usize::from(tone).min(self.tones.len().saturating_sub(1)))
        else {
            return Err(format!("unknown sample tone: {tone}"));
        };
        sink.append(sample.clone().speed(speed).amplify(gain.clamp(0.04, 1.55)));
        sink.play();
        Ok(())
    }

    fn stop_all(&mut self) {
        for sink in &self.sinks {
            sink.stop();
            sink.clear();
        }
    }
}

fn playback_speed(note: u8) -> f32 {
    playback_speed_from(note, SAMPLE_ROOT_NOTE)
}

fn playback_speed_from(note: u8, root: u8) -> f32 {
    2.0_f32.powf((i32::from(note) - i32::from(root)) as f32 / 12.0)
}

fn decode_wav(bytes: &'static [u8], max_seconds: f32) -> Result<(u16, u32, Vec<f32>), String> {
    let decoder = Decoder::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let interleaved = decoder
        .map(|sample| f32::from(sample) / f32::from(i16::MAX))
        .collect::<Vec<_>>();
    if interleaved.is_empty() || channels == 0 {
        return Err("sample contains no audio".into());
    }

    let mono = if channels == 1 {
        interleaved
    } else {
        let ch = channels as usize;
        interleaved
            .chunks_exact(ch)
            .map(|frame| frame.iter().sum::<f32>() / ch as f32)
            .collect()
    };

    let max_samples = ((sample_rate as f32) * max_seconds).round() as usize;
    let mut samples = if mono.len() > max_samples {
        mono[..max_samples].to_vec()
    } else {
        mono
    };
    apply_release_fade(&mut samples, sample_rate, 0.10);
    Ok((1, sample_rate, samples))
}

fn apply_release_fade(samples: &mut [f32], sample_rate: u32, seconds: f32) {
    let fade = ((sample_rate as f32) * seconds).round().max(32.0) as usize;
    let start = samples.len().saturating_sub(fade);
    let denom = fade.max(1) as f32;
    for (offset, sample) in samples[start..].iter_mut().enumerate() {
        *sample *= 1.0 - (offset as f32 / denom);
    }
}

fn build_guitar_tones() -> Result<(u16, u32, Vec<Vec<f32>>), String> {
    let (channels, sample_rate, samples) = decode_wav(GUITAR_SAMPLE, GUITAR_MAX_SECONDS)?;
    let tones = (0..5)
        .map(|tone| process_guitar_tone(samples.clone(), channels as usize, tone))
        .collect();
    Ok((channels, sample_rate, tones))
}

fn build_piano_tones() -> Result<(u16, u32, Vec<Vec<f32>>), String> {
    let (channels, sample_rate, samples) = decode_wav(PIANO_SAMPLE, PIANO_MAX_SECONDS)?;
    let tones = (0..3)
        .map(|tone| process_piano_tone(samples.clone(), sample_rate, tone))
        .collect();
    Ok((channels, sample_rate, tones))
}

fn build_drum_tones() -> Result<(u16, u32, Vec<Vec<f32>>), String> {
    let (ch0, sr0, kick) = decode_wav(DRUM_KICK_SAMPLE, DRUM_MAX_SECONDS)?;
    let (_ch1, sr1, snare) = decode_wav(DRUM_SNARE_SAMPLE, DRUM_MAX_SECONDS)?;
    let (_ch2, sr2, closed) = decode_wav(DRUM_CLOSED_HAT_SAMPLE, DRUM_MAX_SECONDS)?;
    let (_ch3, sr3, open) = decode_wav(DRUM_OPEN_HAT_SAMPLE, DRUM_MAX_SECONDS)?;
    let snare = resample_mono(snare, sr1, sr0);
    let closed = resample_mono(closed, sr2, sr0);
    let open = resample_mono(open, sr3, sr0);
    Ok((ch0, sr0, vec![kick, snare, closed, open]))
}

fn resample_mono(samples: Vec<f32>, from: u32, to: u32) -> Vec<f32> {
    if from == 0 || to == 0 || from == to || samples.is_empty() {
        return samples;
    }
    let ratio = to as f32 / from as f32;
    let new_len = ((samples.len() as f32) * ratio).round().max(1.0) as usize;
    let last = samples.len().saturating_sub(1);
    (0..new_len)
        .map(|index| {
            let src = index as f32 / ratio;
            let i0 = (src.floor() as usize).min(last);
            let i1 = (i0 + 1).min(last);
            let t = src - i0 as f32;
            samples[i0] * (1.0 - t) + samples[i1] * t
        })
        .collect()
}

fn process_guitar_tone(mut samples: Vec<f32>, channels: usize, tone: u8) -> Vec<f32> {
    let channels = channels.max(1);
    if tone == 4 {
        let mut low_pass = vec![0.0; channels];
        for frame in samples.chunks_exact_mut(channels) {
            for (channel, sample) in frame.iter_mut().enumerate() {
                low_pass[channel] += (*sample - low_pass[channel]) * 0.22;
                *sample = low_pass[channel];
            }
        }
    } else {
        let drive: f32 = match tone {
            0 => 1.0,
            1 => 1.35,
            2 => 2.2,
            _ => 4.5,
        };
        if drive > 1.0 {
            let normalization = drive.tanh();
            for sample in &mut samples {
                *sample = (*sample * drive).tanh() / normalization;
            }
        }
    }
    samples
}

fn process_piano_tone(mut samples: Vec<f32>, sample_rate: u32, tone: u8) -> Vec<f32> {
    match tone {
        // Soft / muted grand — gentle low-pass
        0 => {
            let mut low = 0.0_f32;
            for sample in &mut samples {
                low += (*sample - low) * 0.18;
                *sample = low * 0.92;
            }
        }
        // Bright power grand — keep the bundled presence, slight polish
        1 => {
            for sample in &mut samples {
                *sample = (*sample * 1.12).tanh();
            }
        }
        // Pluck / harp-ish — shorter and brighter
        _ => {
            let keep = ((sample_rate as f32) * 0.72).round() as usize;
            if samples.len() > keep {
                samples.truncate(keep);
                apply_release_fade(&mut samples, sample_rate, 0.06);
            }
            let mut low = 0.0_f32;
            for sample in &mut samples {
                low += (*sample - low) * 0.35;
                let bright = *sample + (*sample - low) * 0.55;
                *sample = bright.clamp(-1.0, 1.0);
            }
        }
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_root_and_octaves_have_expected_pitch() {
        assert!((playback_speed(60) - 1.0).abs() < f32::EPSILON);
        assert!((playback_speed(72) - 2.0).abs() < f32::EPSILON);
        assert!((playback_speed(48) - 0.5).abs() < f32::EPSILON);
        // Open low E relative to guitar root E3 is one octave down.
        assert!((playback_speed_from(40, GUITAR_SAMPLE_ROOT) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn guitar_tone_processing_preserves_sample_length() {
        let sample = vec![0.1, -0.1, 0.2, -0.2];
        for tone in 0..5 {
            assert_eq!(
                process_guitar_tone(sample.clone(), 1, tone).len(),
                sample.len()
            );
        }
    }

    #[test]
    fn bundled_guitar_sample_decodes_as_mono_attack() {
        let (channels, sample_rate, samples) =
            decode_wav(GUITAR_SAMPLE, GUITAR_MAX_SECONDS).expect("guitar sample");
        assert_eq!(channels, 1);
        assert_eq!(sample_rate, 44_100);
        let max_len = ((sample_rate as f32) * GUITAR_MAX_SECONDS).round() as usize + 8;
        assert!(samples.len() <= max_len);
        assert!(samples.len() > sample_rate as usize / 4);
    }

    #[test]
    fn bundled_piano_sample_decodes_and_builds_presets() {
        let (channels, sample_rate, tones) = build_piano_tones().expect("piano tones");
        assert_eq!(channels, 1);
        assert_eq!(sample_rate, 44_100);
        assert_eq!(tones.len(), 3);
        assert!(tones[1].len() > sample_rate as usize / 3);
        assert!(tones[2].len() <= tones[1].len());
    }

    #[test]
    fn bundled_drum_kit_decodes_four_shots() {
        let (channels, _sample_rate, tones) = build_drum_tones().expect("drum tones");
        assert_eq!(channels, 1);
        assert_eq!(tones.len(), 4);
        assert!(tones[0].len() > 200);
        assert!(tones[1].len() > 200);
        assert!(tones[2].len() > 200);
        assert!(tones[3].len() > 200);
    }
}
