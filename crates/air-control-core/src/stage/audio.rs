use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use rodio::{buffer::SamplesBuffer, Decoder, OutputStream, OutputStreamHandle, Sink, Source};

const GUITAR_SAMPLE: &[u8] =
    include_bytes!("../../../note-one-shot-pluck-guitar-5_C_major (1).wav");
const SAMPLE_ROOT_NOTE: u8 = 60;
const MAX_ACTIVE_NOTES: usize = 32;

pub(super) struct GuitarAudio {
    commands: Sender<AudioCommand>,
}

enum AudioCommand {
    Play { note: u8, velocity: u8, tone: u8 },
    Stop,
}

struct GuitarRenderer {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    tones: Vec<SamplesBuffer<f32>>,
    sinks: Vec<Sink>,
}

impl GuitarAudio {
    pub(super) fn open() -> Result<Self, String> {
        let (commands, receiver) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("air-control-guitar-audio".into())
            .spawn(move || audio_thread(receiver, ready_tx))
            .map_err(|error| error.to_string())?;

        ready_rx.recv().map_err(|error| error.to_string())??;
        Ok(Self { commands })
    }

    pub(super) fn play(&self, note: u8, velocity: u8, tone: u8) -> Result<(), String> {
        self.commands
            .send(AudioCommand::Play {
                note,
                velocity,
                tone,
            })
            .map_err(|error| error.to_string())
    }

    pub(super) fn stop_all(&self) {
        if self.commands.send(AudioCommand::Stop).is_err() {
            tracing::warn!("guitar audio thread is no longer available");
        }
    }
}

fn audio_thread(receiver: Receiver<AudioCommand>, ready: mpsc::SyncSender<Result<(), String>>) {
    let mut renderer = match GuitarRenderer::open() {
        Ok(renderer) => {
            if ready.send(Ok(())).is_err() {
                return;
            }
            renderer
        }
        Err(error) => {
            if ready.send(Err(error)).is_err() {
                tracing::warn!("guitar audio initialization failed after the stage was closed");
            }
            return;
        }
    };

    while let Ok(command) = receiver.recv() {
        match command {
            AudioCommand::Play {
                note,
                velocity,
                tone,
            } => {
                if let Err(error) = renderer.play(note, velocity, tone) {
                    tracing::warn!(%error, "guitar sample playback failed");
                }
            }
            AudioCommand::Stop => renderer.stop_all(),
        }
    }
}

impl GuitarRenderer {
    fn open() -> Result<Self, String> {
        let (stream, stream_handle) =
            OutputStream::try_default().map_err(|error| error.to_string())?;
        let (channels, sample_rate, samples) = decode_sample()?;

        let tones = (0..5)
            .map(|tone| {
                SamplesBuffer::new(
                    channels,
                    sample_rate,
                    process_tone(samples.clone(), channels as usize, tone),
                )
            })
            .collect();

        Ok(Self {
            _stream: stream,
            stream_handle,
            tones,
            sinks: Vec::new(),
        })
    }

    fn play(&mut self, note: u8, velocity: u8, tone: u8) -> Result<(), String> {
        self.sinks.retain(|sink| !sink.empty());
        if self.sinks.len() >= MAX_ACTIVE_NOTES {
            self.sinks.remove(0).stop();
        }

        let sink = Sink::try_new(&self.stream_handle).map_err(|error| error.to_string())?;
        let speed = playback_speed(note);
        let gain = (f32::from(velocity.min(127)) / 127.0).powf(1.35);
        let Some(sample) = self.tones.get(usize::from(tone.min(4))) else {
            return Err(format!("unknown guitar tone: {tone}"));
        };
        sink.append(sample.clone().speed(speed).amplify(gain));
        self.sinks.push(sink);
        Ok(())
    }

    fn stop_all(&mut self) {
        for sink in self.sinks.drain(..) {
            sink.stop();
        }
    }
}

fn playback_speed(note: u8) -> f32 {
    2.0_f32.powf((i32::from(note) - i32::from(SAMPLE_ROOT_NOTE)) as f32 / 12.0)
}

fn decode_sample() -> Result<(u16, u32, Vec<f32>), String> {
    let decoder = Decoder::new(Cursor::new(GUITAR_SAMPLE)).map_err(|error| error.to_string())?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples = decoder
        .map(|sample| f32::from(sample) / f32::from(i16::MAX))
        .collect::<Vec<_>>();
    if samples.is_empty() || channels == 0 {
        return Err("The guitar sample contains no audio".into());
    }
    Ok((channels, sample_rate, samples))
}

fn process_tone(mut samples: Vec<f32>, channels: usize, tone: u8) -> Vec<f32> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_root_and_octaves_have_expected_pitch() {
        assert!((playback_speed(60) - 1.0).abs() < f32::EPSILON);
        assert!((playback_speed(72) - 2.0).abs() < f32::EPSILON);
        assert!((playback_speed(48) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn tone_processing_preserves_sample_length() {
        let sample = vec![0.1, -0.1, 0.2, -0.2];
        for tone in 0..5 {
            assert_eq!(process_tone(sample.clone(), 2, tone).len(), sample.len());
        }
    }

    #[test]
    fn bundled_guitar_sample_decodes_as_stereo_audio() {
        let (channels, sample_rate, samples) =
            decode_sample().expect("bundled guitar sample should decode");
        assert_eq!(channels, 2);
        assert_eq!(sample_rate, 44_100);
        assert!(samples.len() > sample_rate as usize);
    }
}
