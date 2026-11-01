//! Music instruments: piano, drums, guitar, melody.
use super::*;
#[path = "../audio.rs"]
mod audio;
use audio::{DrumAudio, GuitarAudio, PianoAudio};

mod piano;
mod drums;
mod guitar;
mod melody;

pub use guitar::{Chord, CHORDS, OPEN_STRINGS, BASS_STRINGS, STRING_Y};

use crate::midi::MidiOut;

#[derive(Clone, Copy)]
struct Hold {
    hand: u8,
    landmark: u8,
    note: u8,
}

#[derive(Clone, Copy)]
struct ScheduledOff {
    at: u64,
    channel: u8,
    note: u8,
}

#[derive(Clone, Copy)]
struct ScheduledOn {
    at: u64,
    channel: u8,
    note: u8,
    velocity: u8,
    off_at: u64,
}

#[derive(Clone, Debug)]
pub(super) struct MelodyBall {
    pub x: f32,
    pub y: f32,
    pub note: u8,
    pub radius: f32,
    pub born_at: u64,
    /// Absolute time (ms) when the ball should cross the hit line.
    pub hit_at: u64,
    pub color: [u8; 3],
    pub lane: u8,
}

#[derive(Clone, Copy, Debug)]
enum MelodyJudge {
    Perfect,
    Good,
    Ok,
}

pub(super) struct Music {
    pub(super) instrument: u8,
    pub(super) piano_preset: u8,
    pub(super) guitar_tone: u8, // 0: Acoustic, 1: Clean/Chorus, 2: Crunch, 3: Distortion Lead, 4: Bass
    pub(super) guitar_mode: u8, // 0: Аккорды/Ритм, 1: Соло/Гриф 12 ладов
    pub(super) capo: u8,        // 0..7 лад
    pub(super) muted: bool,
    pub(super) volume: f32,
    midi: Option<MidiOut>,
    guitar_audio: Option<GuitarAudio>,
    piano_audio: Option<PianoAudio>,
    drum_audio: Option<DrumAudio>,
    pub(super) midi_failed: bool,
    guitar_audio_failed: bool,
    piano_audio_failed: bool,
    drum_audio_failed: bool,
    holds: Vec<Hold>,
    piano_down: [u8; 128],
    pending_off: Vec<ScheduledOff>,
    pending_on: Vec<ScheduledOn>,
    drum_at: [u64; 4],
    pluck_at: Vec<(u8, u8, u64)>,
    string_hit_at: [u64; 6],
    pub(super) string_energy: [f32; 6],
    pub(super) string_phase: [f32; 6],
    pub(super) chord: usize,
    pub(super) solo_fret: [Option<u8>; 6], // Зажатый лад на каждой из 6 струн
    pub(super) lit_key: Option<(u8, u64)>,
    pub(super) lit_drum: Option<(usize, u64)>,
    /// Palm aim points while on drums.
    pub(super) drum_aim: Vec<(f32, f32)>,
    /// Pad under the palm (for highlight).
    pub(super) drum_hover: Option<usize>,
    /// Live picking fingertips (norm coords) for guitar HUD.
    pub(super) guitar_tips: Vec<(f32, f32, u8)>,
    pub(super) melody_balls: Vec<MelodyBall>,
    pub(super) melody_index: usize,
    pub(super) melody_song: u8,
    /// 0 easy · 1 normal · 2 hard
    pub(super) melody_difficulty: u8,
    pub(super) melody_score: u32,
    pub(super) melody_combo: u32,
    pub(super) melody_best_combo: u32,
    pub(super) melody_perfects: u32,
    pub(super) melody_goods: u32,
    pub(super) melody_oks: u32,
    pub(super) melody_misses: u32,
    pub(super) melody_lives: u8,
    pub(super) melody_max_lives: u8,
    /// Song clock origin (first note hit time). 0 = not started.
    pub(super) melody_origin: u64,
    pub(super) melody_next_at: u64,
    /// Live palm aim points for paint (norm coords).
    pub(super) melody_tips: Vec<(f32, f32)>,
    /// Fist-mode lock: note under palm ready to crush (x, y).
    pub(super) melody_aim_lock: Option<(f32, f32)>,
    /// Last hit flash: (x, y, until_ms, judge 0=ok 1=good 2=perfect)
    pub(super) melody_flash: Option<(f32, f32, u64, u8)>,
    /// Floating judge tag until_ms
    pub(super) melody_banner_until: u64,
    last_switch: u64,
    swipe_armed: bool,
    last_attack_at: u64,
    last_decay_ms: u64,
    bend_origin_y: Option<f32>,
    bend_active: bool,
    pub(super) sustain_active: bool,
}

impl Default for Music {
    fn default() -> Self {
        Self {
            instrument: 2, // Гитара по умолчанию
            piano_preset: 1, // Яркий рояль (bundled bright grand one-shot)
            guitar_tone: 0,
            guitar_mode: 0,
            capo: 0,
            muted: false,
            volume: 0.88,
            midi: None,
            guitar_audio: None,
            piano_audio: None,
            drum_audio: None,
            midi_failed: false,
            guitar_audio_failed: false,
            piano_audio_failed: false,
            drum_audio_failed: false,
            holds: Vec::with_capacity(32),
            piano_down: [0; 128],
            pending_off: Vec::with_capacity(48),
            pending_on: Vec::with_capacity(48),
            drum_at: [0; 4],
            pluck_at: Vec::with_capacity(16),
            string_hit_at: [0; 6],
            string_energy: [0.0; 6],
            string_phase: [0.0; 6],
            chord: 0,
            solo_fret: [None; 6],
            lit_key: None,
            lit_drum: None,
            drum_aim: Vec::with_capacity(4),
            drum_hover: None,
            guitar_tips: Vec::with_capacity(10),
            melody_balls: Vec::with_capacity(10),
            melody_index: 0,
            melody_song: 0,
            melody_difficulty: 0,
            melody_score: 0,
            melody_combo: 0,
            melody_best_combo: 0,
            melody_perfects: 0,
            melody_goods: 0,
            melody_oks: 0,
            melody_misses: 0,
            melody_lives: 3,
            melody_max_lives: 3,
            melody_origin: 0,
            melody_next_at: 0,
            melody_tips: Vec::with_capacity(10),
            melody_aim_lock: None,
            melody_flash: None,
            melody_banner_until: 0,
            last_switch: 0,
            swipe_armed: true,
            last_attack_at: 0,
            last_decay_ms: 0,
            bend_origin_y: None,
            bend_active: false,
            sustain_active: false,
        }
    }
}

/// (midi note, delay after previous note in ms, horizontal lane 0..4)
type MelodyNote = (u8, u64, u8);


impl Stage {
    pub(super) fn clear_music_holds(&mut self) {
        self.music.holds.clear();
        self.music.melody_balls.clear();
        self.music.melody_index = 0;
        self.music.melody_next_at = 0;
        self.music.melody_origin = 0;
        self.music.melody_combo = 0;
        self.music.melody_tips.clear();
        self.music.melody_aim_lock = None;
        self.music.melody_flash = None;
        self.music.melody_banner_until = 0;
        self.music.drum_aim.clear();
        self.music.drum_hover = None;
        self.music.guitar_tips.clear();
    }

    pub(super) fn paint_music(&self, image: &mut RgbImage) {
        match self.music.instrument {
            0 => self.paint_piano(image),
            1 => self.paint_drums(image),
            2 => self.paint_guitar(image),
            _ => self.paint_melody(image),
        }
    }

    pub(super) fn paint_hits(&self, _image: &mut RgbImage) {}

    pub(super) fn switched_instrument(&mut self, _now: u64, _hands: &[TrackedHand]) -> bool {
        // Two-hand raise used to cycle instruments and stole Melody mid-play.
        // Switch only via the UI chips.
        false
    }

    pub(super) fn ensure_midi(&mut self) {
        if self.music.midi.is_some() || self.music.midi_failed {
            return;
        }
        if cfg!(test) {
            self.music.midi_failed = false;
            return;
        }
        match MidiOut::open() {
            Some(midi) => self.music.midi = Some(midi),
            None => self.music.midi_failed = true,
        }
    }

    pub(super) fn apply_program(&mut self) {
        let Some(midi) = &self.music.midi else { return };
        if self.music.instrument == 2 {
            // 0: Acoustic Steel, 1: Clean Electric, 2: Overdrive, 3: Distortion Lead, 4: Electric Bass
            let program = match self.music.guitar_tone {
                1 => 27, // Clean Electric Guitar
                2 => 29, // Overdriven Guitar
                3 => 30, // Distortion Guitar
                4 => 33, // Electric Bass (finger)
                _ => 25, // Acoustic Guitar (steel)
            };
            midi.program(0, program);

            // Реверберация и хорус для сочного гитарного звучания
            if self.music.guitar_tone == 1 {
                midi.control(0, 91, 75); // Reverb
                midi.control(0, 93, 85); // Chorus
            } else if self.music.guitar_tone == 3 {
                midi.control(0, 91, 60); // Lead Reverb
                midi.control(0, 93, 0);
            }
        } else if matches!(self.music.instrument, 0 | 3) {
            let prog = [0, 1, 46][self.music.piano_preset.min(2) as usize];
            midi.program(0, prog);
        }
    }

    pub(super) fn push_volume(&self) {
        let Some(midi) = &self.music.midi else { return };
        let value = (self.music.volume.clamp(0.0, 1.0) * 127.0).round() as u8;
        midi.control(0, 7, value);
        midi.control(9, 7, value);
    }

    pub(super) fn attack(&mut self, note: u8, velocity: u8) {
        if self.music.muted || velocity == 0 {
            return;
        }
        let slot = &mut self.music.piano_down[note as usize];
        let first = *slot == 0;
        *slot = slot.saturating_add(1);
        if !first {
            return;
        }

        // Prefer the bundled grand one-shot; MIDI stays as a fallback.
        if let Some(audio) = &self.music.piano_audio {
            match audio.play(note, velocity, self.music.piano_preset) {
                Ok(()) => return,
                Err(error) => {
                    tracing::warn!(%error, "piano sample playback failed");
                    self.music.piano_audio = None;
                    self.music.piano_audio_failed = true;
                }
            }
        }
        if let Some(midi) = &self.music.midi {
            midi.note_on(0, note, velocity);
        }
    }

    pub(super) fn release_note(&mut self, note: u8) {
        let slot = &mut self.music.piano_down[note as usize];
        if *slot == 0 {
            return;
        }
        *slot -= 1;
        if *slot == 0 {
            if let Some(midi) = &self.music.midi {
                midi.note_off(0, note);
            }
        }
    }

    pub(super) fn shot(&mut self, channel: u8, note: u8, velocity: u8, off_at: u64) {
        if self.music.muted || velocity == 0 {
            return;
        }
        self.play_note_on(channel, note, velocity);
        if self.music.midi.is_some() {
            self.music.pending_off.push(ScheduledOff {
                at: off_at,
                channel,
                note,
            });
        }
    }

    fn play_note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        if channel == 9 {
            if let Some(kit) = Self::drum_kit_for_note(note) {
                if let Some(audio) = &self.music.drum_audio {
                    match audio.play(kit, velocity) {
                        Ok(()) => return,
                        Err(error) => {
                            tracing::warn!(%error, "drum sample playback failed");
                            self.music.drum_audio = None;
                            self.music.drum_audio_failed = true;
                        }
                    }
                }
            }
        }
        if channel == 0 && self.music.instrument == 2 {
            let result = self
                .music
                .guitar_audio
                .as_mut()
                .map(|audio| audio.play(note, velocity, self.music.guitar_tone));
            if let Some(result) = result {
                match result {
                    Ok(()) => return,
                    Err(error) => {
                        tracing::warn!(%error, "guitar sample playback failed");
                        self.music.guitar_audio = None;
                        self.music.guitar_audio_failed = true;
                    }
                }
            }
        }
        if channel == 0 && matches!(self.music.instrument, 0 | 3) {
            if let Some(audio) = &self.music.piano_audio {
                match audio.play(note, velocity, self.music.piano_preset) {
                    Ok(()) => return,
                    Err(error) => {
                        tracing::warn!(%error, "piano sample playback failed");
                        self.music.piano_audio = None;
                        self.music.piano_audio_failed = true;
                    }
                }
            }
        }
        if let Some(midi) = &self.music.midi {
            midi.note_on(channel, note, velocity);
        }
    }

    pub(super) fn silence(&mut self) {
        self.music.pending_off.clear();
        self.music.pending_on.clear();
        self.music.holds.clear();
        self.music.piano_down = [0; 128];
        self.music.bend_active = false;
        self.music.bend_origin_y = None;
        if let Some(audio) = &mut self.music.guitar_audio {
            audio.stop_all();
        }
        if let Some(audio) = &mut self.music.piano_audio {
            audio.stop_all();
        }
        if let Some(audio) = &mut self.music.drum_audio {
            audio.stop_all();
        }
        if let Some(midi) = &self.music.midi {
            midi.pitch_bend(0, 8192);
            midi.control(0, 64, 0);
            midi.silence();
        }
    }

    pub(super) fn velocity(&self, amount: f32) -> u8 {
        let scaled = amount.clamp(0.0, 1.0) * self.music.volume.clamp(0.0, 1.0);
        if scaled < 0.02 {
            return 0;
        }
        (scaled * 127.0).round().clamp(1.0, 127.0) as u8
    }
    pub(super) fn schedule_note(
        &mut self,
        channel: u8,
        note: u8,
        velocity: u8,
        on_at: u64,
        off_at: u64,
    ) {
        if self.music.muted || velocity == 0 {
            return;
        }
        self.music.pending_on.push(ScheduledOn {
            at: on_at,
            channel,
            note,
            velocity,
            off_at,
        });
    }

    pub(super) fn flush_offs(&mut self, now: u64) {
        let ready_on: Vec<_> = self.music.pending_on.drain(..).collect();
        let mut wait_on = Vec::new();
        for item in ready_on {
            if item.at <= now {
                self.play_note_on(item.channel, item.note, item.velocity);
                if self.music.midi.is_some() {
                    self.music.pending_off.push(ScheduledOff {
                        at: item.off_at,
                        channel: item.channel,
                        note: item.note,
                    });
                }
            } else {
                wait_on.push(item);
            }
        }
        self.music.pending_on = wait_on;

        let due_off: Vec<_> = self.music.pending_off.drain(..).collect();
        let mut wait_off = Vec::new();
        for item in due_off {
            if item.at <= now {
                if let Some(midi) = &self.music.midi {
                    midi.note_off(item.channel, item.note);
                }
            } else {
                wait_off.push(item);
            }
        }
        self.music.pending_off = wait_off;
    }

}

pub(super) fn pose_of(hand: &TrackedHand, pinch_ratio: f32) -> Pose {
    if crate::gestures::pinch_active(&hand.landmarks, false, pinch_ratio) {
        return Pose::Pinch;
    }
    let shape = hand.world.as_ref().unwrap_or(&hand.landmarks);
    let open = hand_openness(shape);
    let curls = [
        finger_curl(shape, [5, 6, 7, 8]),
        finger_curl(shape, [9, 10, 11, 12]),
        finger_curl(shape, [13, 14, 15, 16]),
        finger_curl(shape, [17, 18, 19, 20]),
    ];
    if open < 0.28 || curls.iter().all(|value| *value > 0.65) {
        return Pose::Fist;
    }
    if open > 0.55 {
        return Pose::Palm;
    }
    let index_open = 1.0 - curls[0];
    if index_open > 0.55 && curls[1] > 0.55 && curls[2] > 0.55 && curls[3] > 0.55 {
        return Pose::Point;
    }
    Pose::Other
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_selection_and_capo_change_playable_string_notes() {
        let mut stage = Stage::default();
        stage.set_guitar_chord(3);
        stage.set_guitar_capo(2);
        assert_eq!(stage.active_guitar_notes(), [45, 49, 52, 57, 61, 69]);
    }

    #[test]
    fn solo_frets_change_only_the_selected_string() {
        let mut stage = Stage::default();
        stage.set_guitar_mode(1);
        stage.set_guitar_capo(2);
        stage.music.solo_fret[3] = Some(3);
        assert_eq!(stage.active_guitar_notes(), [42, 47, 52, 60, 61, 66]);
    }

    #[test]
    fn string_preview_updates_feedback_and_animation() {
        let mut stage = Stage::default();
        stage.preview_guitar_string(0, 100);
        assert_eq!(stage.music.string_energy[0], 1.0);
        assert_eq!(stage.last_hit, 100);
        assert_eq!(stage.label, "Em · струна 6");
    }
}
