use super::*;
#[path = "audio.rs"]
mod audio;

use crate::midi::MidiOut;
use audio::{GuitarAudio, PianoAudio};

// --- ПИАНИНО (2 октавы C3..C5: 15 белых + 10 чёрных клавиш) ---

#[derive(Clone, Copy, Debug)]
pub(super) struct WhiteKey {
    pub name: &'static str,
    pub note: u8,
    pub white_idx: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BlackKey {
    pub name: &'static str,
    pub note: u8,
    pub left_offset_ratio: f32,
}

const WHITE_KEYS: &[WhiteKey] = &[
    WhiteKey {
        name: "C3",
        note: 48,
        white_idx: 0,
    },
    WhiteKey {
        name: "D3",
        note: 50,
        white_idx: 1,
    },
    WhiteKey {
        name: "E3",
        note: 52,
        white_idx: 2,
    },
    WhiteKey {
        name: "F3",
        note: 53,
        white_idx: 3,
    },
    WhiteKey {
        name: "G3",
        note: 55,
        white_idx: 4,
    },
    WhiteKey {
        name: "A3",
        note: 57,
        white_idx: 5,
    },
    WhiteKey {
        name: "B3",
        note: 59,
        white_idx: 6,
    },
    WhiteKey {
        name: "C4",
        note: 60,
        white_idx: 7,
    },
    WhiteKey {
        name: "D4",
        note: 62,
        white_idx: 8,
    },
    WhiteKey {
        name: "E4",
        note: 64,
        white_idx: 9,
    },
    WhiteKey {
        name: "F4",
        note: 65,
        white_idx: 10,
    },
    WhiteKey {
        name: "G4",
        note: 67,
        white_idx: 11,
    },
    WhiteKey {
        name: "A4",
        note: 69,
        white_idx: 12,
    },
    WhiteKey {
        name: "B4",
        note: 71,
        white_idx: 13,
    },
    WhiteKey {
        name: "C5",
        note: 72,
        white_idx: 14,
    },
];

const BLACK_KEYS: &[BlackKey] = &[
    BlackKey {
        name: "C#3",
        note: 49,
        left_offset_ratio: 0.65,
    },
    BlackKey {
        name: "D#3",
        note: 51,
        left_offset_ratio: 1.70,
    },
    BlackKey {
        name: "F#3",
        note: 54,
        left_offset_ratio: 3.65,
    },
    BlackKey {
        name: "G#3",
        note: 56,
        left_offset_ratio: 4.68,
    },
    BlackKey {
        name: "A#3",
        note: 58,
        left_offset_ratio: 5.72,
    },
    BlackKey {
        name: "C#4",
        note: 61,
        left_offset_ratio: 7.65,
    },
    BlackKey {
        name: "D#4",
        note: 63,
        left_offset_ratio: 8.70,
    },
    BlackKey {
        name: "F#4",
        note: 66,
        left_offset_ratio: 10.65,
    },
    BlackKey {
        name: "G#4",
        note: 68,
        left_offset_ratio: 11.68,
    },
    BlackKey {
        name: "A#4",
        note: 70,
        left_offset_ratio: 12.72,
    },
];

// --- БАРАБАНЫ ---

struct Drum {
    name: &'static str,
    note: u8,
    x: f32,
    y: f32,
    r: f32,
    color: [u8; 3],
}

const DRUMS: &[Drum] = &[
    Drum {
        name: "Бочка",
        note: 36,
        x: 0.50,
        y: 0.80,
        r: 0.095,
        color: [244, 114, 182],
    },
    Drum {
        name: "Малый",
        note: 38,
        x: 0.33,
        y: 0.68,
        r: 0.080,
        color: [56, 189, 248],
    },
    Drum {
        name: "Хэт",
        note: 42,
        x: 0.17,
        y: 0.55,
        r: 0.075,
        color: [52, 211, 153],
    },
    Drum {
        name: "Том 1",
        note: 48,
        x: 0.40,
        y: 0.50,
        r: 0.070,
        color: [167, 139, 250],
    },
    Drum {
        name: "Том 2",
        note: 45,
        x: 0.60,
        y: 0.50,
        r: 0.070,
        color: [192, 132, 252],
    },
    Drum {
        name: "Крэш",
        note: 49,
        x: 0.83,
        y: 0.54,
        r: 0.080,
        color: [251, 191, 36],
    },
    Drum {
        name: "Райд",
        note: 51,
        x: 0.70,
        y: 0.70,
        r: 0.078,
        color: [251, 146, 60],
    },
];

// --- ГИТАРА: 16 ПРОФЕССИОНАЛЬНЫХ АККОРДОВ ---

#[derive(Clone, Copy)]
pub struct Chord {
    pub name: &'static str,
    pub strings: [u8; 6], // E2, A2, D3, G3, B3, E4
    pub power: [u8; 6],   // Пауэр-аккорд (Mute/Chug)
    pub color: [u8; 3],
}

// Базовый строй открытых струн гитары (E2, A2, D3, G3, B3, E4)
pub const OPEN_STRINGS: [u8; 6] = [40, 45, 50, 55, 59, 64];
// Строй бас-гитары (E1, A1, D2, G2, B2, E3)
pub const BASS_STRINGS: [u8; 6] = [28, 33, 38, 43, 47, 52];

pub const CHORDS: &[Chord] = &[
    Chord {
        name: "Em",
        strings: [40, 47, 52, 55, 59, 64],
        power: [40, 47, 52, 59, 0, 0],
        color: [52, 211, 153],
    },
    Chord {
        name: "Am",
        strings: [0, 45, 52, 57, 60, 64],
        power: [45, 52, 57, 64, 0, 0],
        color: [56, 189, 248],
    },
    Chord {
        name: "C",
        strings: [0, 48, 52, 55, 60, 64],
        power: [48, 55, 60, 67, 0, 0],
        color: [167, 139, 250],
    },
    Chord {
        name: "G",
        strings: [43, 47, 50, 55, 59, 67],
        power: [43, 50, 55, 62, 0, 0],
        color: [251, 191, 36],
    },
    Chord {
        name: "D",
        strings: [0, 0, 50, 57, 62, 66],
        power: [50, 57, 62, 69, 0, 0],
        color: [244, 114, 182],
    },
    Chord {
        name: "A",
        strings: [0, 45, 52, 57, 61, 64],
        power: [45, 52, 57, 64, 0, 0],
        color: [34, 211, 238],
    },
    Chord {
        name: "E",
        strings: [40, 47, 52, 56, 59, 64],
        power: [40, 47, 52, 59, 0, 0],
        color: [251, 113, 133],
    },
    Chord {
        name: "F",
        strings: [41, 48, 53, 57, 60, 65],
        power: [41, 48, 53, 60, 0, 0],
        color: [192, 132, 252],
    },
    Chord {
        name: "Dm",
        strings: [0, 0, 50, 57, 62, 65],
        power: [50, 57, 62, 69, 0, 0],
        color: [129, 140, 248],
    },
    Chord {
        name: "Bm",
        strings: [0, 47, 54, 59, 62, 66],
        power: [47, 54, 59, 66, 0, 0],
        color: [103, 232, 249],
    },
    Chord {
        name: "Em7",
        strings: [40, 47, 50, 55, 59, 64],
        power: [40, 47, 52, 62, 0, 0],
        color: [134, 239, 172],
    },
    Chord {
        name: "Cadd9",
        strings: [0, 48, 52, 55, 62, 64],
        power: [48, 55, 62, 67, 0, 0],
        color: [221, 214, 254],
    },
    Chord {
        name: "Dsus4",
        strings: [0, 0, 50, 57, 62, 67],
        power: [50, 57, 62, 69, 0, 0],
        color: [253, 164, 175],
    },
    Chord {
        name: "Asus2",
        strings: [0, 45, 52, 57, 59, 64],
        power: [45, 52, 57, 64, 0, 0],
        color: [147, 197, 253],
    },
    Chord {
        name: "F#m",
        strings: [42, 49, 54, 57, 61, 66],
        power: [42, 49, 54, 61, 0, 0],
        color: [216, 180, 254],
    },
    Chord {
        name: "B7",
        strings: [0, 47, 53, 57, 59, 66],
        power: [47, 54, 59, 66, 0, 0],
        color: [253, 230, 138],
    },
];

pub const STRING_Y: [f32; 6] = [0.26, 0.34, 0.42, 0.50, 0.58, 0.66];
// Калибр струн: 6-я толстая (4.2px), 1-я тонкая (1.2px)
const STRING_THICKNESS: [f32; 6] = [4.2, 3.4, 2.7, 2.0, 1.5, 1.2];

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
    pub color: [u8; 3],
    pub _lane: u8,
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
    pub(super) midi_failed: bool,
    guitar_audio_failed: bool,
    piano_audio_failed: bool,
    holds: Vec<Hold>,
    piano_down: [u8; 128],
    pending_off: Vec<ScheduledOff>,
    pending_on: Vec<ScheduledOn>,
    drum_at: [u64; 7],
    pluck_at: Vec<(u8, u8, u64)>,
    string_hit_at: [u64; 6],
    pub(super) string_energy: [f32; 6],
    pub(super) string_phase: [f32; 6],
    pub(super) chord: usize,
    pub(super) solo_fret: [Option<u8>; 6], // Зажатый лад на каждой из 6 струн
    pub(super) lit_key: Option<(u8, u64)>,
    pub(super) lit_drum: Option<(usize, u64)>,
    pub(super) melody_balls: Vec<MelodyBall>,
    pub(super) melody_index: usize,
    pub(super) melody_song: u8,
    pub(super) melody_score: u32,
    pub(super) melody_combo: u32,
    pub(super) melody_next_at: u64,
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
            midi_failed: false,
            guitar_audio_failed: false,
            piano_audio_failed: false,
            holds: Vec::with_capacity(32),
            piano_down: [0; 128],
            pending_off: Vec::with_capacity(48),
            pending_on: Vec::with_capacity(48),
            drum_at: [0; 7],
            pluck_at: Vec::with_capacity(16),
            string_hit_at: [0; 6],
            string_energy: [0.0; 6],
            string_phase: [0.0; 6],
            chord: 0,
            solo_fret: [None; 6],
            lit_key: None,
            lit_drum: None,
            melody_balls: Vec::with_capacity(8),
            melody_index: 0,
            melody_song: 0,
            melody_score: 0,
            melody_combo: 0,
            melody_next_at: 0,
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

const MELODY_TWINKLE: &[MelodyNote] = &[
    (60, 0, 1),
    (60, 420, 1),
    (67, 420, 3),
    (67, 420, 3),
    (69, 420, 4),
    (69, 420, 4),
    (67, 520, 3),
    (65, 520, 2),
    (65, 420, 2),
    (64, 420, 1),
    (64, 420, 1),
    (62, 420, 0),
    (62, 420, 0),
    (60, 520, 1),
];

const MELODY_JOY: &[MelodyNote] = &[
    (64, 0, 2),
    (64, 360, 2),
    (65, 360, 3),
    (67, 360, 4),
    (67, 360, 4),
    (65, 360, 3),
    (64, 360, 2),
    (62, 360, 1),
    (60, 360, 0),
    (60, 360, 0),
    (62, 360, 1),
    (64, 360, 2),
    (64, 480, 2),
    (62, 240, 1),
    (62, 480, 1),
];

fn melody_song(song: u8) -> &'static [MelodyNote] {
    if song == 1 {
        MELODY_JOY
    } else {
        MELODY_TWINKLE
    }
}

fn melody_ball_color(note: u8) -> [u8; 3] {
    match note % 12 {
        0 => [251, 113, 133],
        2 => [251, 191, 36],
        4 => [52, 211, 153],
        5 => [96, 165, 250],
        7 => [167, 139, 250],
        9 => [244, 114, 182],
        _ => [125, 211, 252],
    }
}

impl Stage {
    pub(super) fn clear_music_holds(&mut self) {
        self.music.holds.clear();
        self.music.melody_balls.clear();
        self.music.melody_index = 0;
        self.music.melody_next_at = 0;
        self.music.melody_combo = 0;
    }

    pub(super) fn reset_melody(&mut self, now: u64) {
        self.music.melody_balls.clear();
        self.music.melody_index = 0;
        self.music.melody_score = 0;
        self.music.melody_combo = 0;
        self.music.melody_next_at = now;
        self.label = if self.music.melody_song == 1 {
            "Мелодия · Ода к радости".into()
        } else {
            "Мелодия · Звёздочка".into()
        };
    }

    // --- ПИАНИНО ---
    pub(super) fn play_piano(&mut self, now: u64, hands: &[TrackedHand]) {
        let sustain = hands.iter().any(|h| {
            let p = palm_of(h);
            p.0 < 0.22 && p.1 < 0.45 && pose_of(h, 0.45) == Pose::Fist
        });
        if sustain != self.music.sustain_active {
            self.music.sustain_active = sustain;
            if let Some(midi) = &self.music.midi {
                midi.control(0, 64, if sustain { 127 } else { 0 });
            }
        }

        let mut next = Vec::new();
        let all_tips = [4usize, 8, 12, 16, 20];

        for (hand_idx, hand) in hands.iter().enumerate() {
            let sample = self.prev_for(hand);
            let metric = hand.world.is_some();
            let sens = self.sensitivity.max(0.35);

            for &landmark in &all_tips {
                let (x, y) = tip(hand, landmark);
                let Some((note, name)) = find_piano_key(x, y) else {
                    continue;
                };

                let prev_y = sample.and_then(|s| tip_index(landmark).map(|f| s.tips[f][1]));
                let dt = sample.map(|s| frame_dt(s.at, now)).unwrap_or(1.0 / 60.0);
                let dy = prev_y.map(|old_y| y - old_y).unwrap_or(0.0);
                let down_speed = dy / dt;

                let existing = self
                    .music
                    .holds
                    .iter()
                    .find(|h| h.hand == hand_idx as u8 && h.landmark == landmark as u8)
                    .map(|h| h.note);

                if let Some(cur_note) = existing {
                    if cur_note != note {
                        self.release_note(cur_note);
                        let vel = self.velocity(0.55);
                        self.attack(note, vel);
                        self.mark_piano_hit(now, name, note);
                    }
                    next.push(Hold {
                        hand: hand_idx as u8,
                        landmark: landmark as u8,
                        note,
                    });
                    continue;
                }

                // Snappier strikes: lower threshold + credit a soft tap when the tip lands on a key.
                let strike_threshold = if metric { 0.14 / sens } else { 0.09 / sens };
                let entered_from_above = prev_y.is_some_and(|oy| oy < 0.60 && y >= 0.60);
                let resting_press = existing.is_none()
                    && prev_y.is_some_and(|oy| (oy - y).abs() < 0.01)
                    && y >= 0.62
                    && now.saturating_sub(self.music.last_attack_at) > 90;

                if down_speed > strike_threshold || entered_from_above || resting_press {
                    let raw_power = (down_speed / if metric { 1.35 } else { 1.9 }).clamp(0.0, 1.0);
                    let tap = if resting_press && down_speed <= strike_threshold {
                        0.34
                    } else {
                        0.30 + raw_power.powf(0.8) * 0.70
                    };
                    let vel = self.velocity(tap.min(1.0));

                    self.attack(note, vel);
                    self.music.last_attack_at = now;
                    self.mark_piano_hit(now, name, note);
                    next.push(Hold {
                        hand: hand_idx as u8,
                        landmark: landmark as u8,
                        note,
                    });
                }
            }
        }

        for old in self.music.holds.clone() {
            if !next
                .iter()
                .any(|h| h.hand == old.hand && h.landmark == old.landmark && h.note == old.note)
            {
                self.release_note(old.note);
            }
        }
        self.music.holds = next;
    }

    // --- БАРАБАНЫ ---
    pub(super) fn play_drums(&mut self, now: u64, hands: &[TrackedHand]) {
        let is_hat_closed = hands.iter().any(|h| pose_of(h, 0.45) == Pose::Fist);

        for hand in hands {
            let Some(sample) = self.prev_for(hand) else {
                continue;
            };
            let dt = frame_dt(sample.at, now);
            let palm = palm_of(hand);
            let finger = tip(hand, 8);

            let palm_down_spd = (palm.1 - sample.palm[1]) / dt;
            let tip_down_spd = (finger.1 - sample.tips[1][1]) / dt;

            if palm_down_spd < 0.05 && tip_down_spd < 0.05 {
                continue;
            }

            let sens = self.sensitivity.max(0.35);
            let threshold = 0.26 / sens;

            for (point, down_spd, is_tip) in
                [(palm, palm_down_spd, false), (finger, tip_down_spd, true)]
            {
                if down_spd < threshold {
                    continue;
                }

                for (zone_idx, drum) in DRUMS.iter().enumerate() {
                    if dist2(point, (drum.x, drum.y)) > drum.r * 1.55 {
                        continue;
                    }
                    if now.saturating_sub(self.music.drum_at[zone_idx]) < 120 {
                        continue;
                    }

                    self.music.drum_at[zone_idx] = now;
                    let (note, name) = if drum.note == 42 {
                        if is_hat_closed {
                            (42, "Хэт (закр.)")
                        } else {
                            (46, "Хэт (откр.)")
                        }
                    } else {
                        (drum.note, drum.name)
                    };

                    let spd_norm = (down_spd / 2.6).clamp(0.0, 1.0);
                    let vel =
                        self.velocity((0.35 + spd_norm * 0.65) * if is_tip { 1.1 } else { 0.95 });

                    self.shot(9, note, vel, now + 120);
                    self.music.lit_drum = Some((zone_idx, now + 160));
                    self.label = name.into();
                    self.last_hit = now;
                    break;
                }
            }
        }
    }

    // =========================================================================
    // --- МЕЛОДИЯ: шарики-ноты, по которым надо попасть пальцами ---
    // =========================================================================

    pub(super) fn play_melody(&mut self, now: u64, hands: &[TrackedHand]) {
        let song = melody_song(self.music.melody_song);
        if song.is_empty() {
            return;
        }

        if self.music.melody_next_at == 0 {
            self.music.melody_next_at = now;
        }

        // Spawn upcoming notes as falling balls (keep a few on screen).
        while self.music.melody_balls.len() < 3 && self.music.melody_index < song.len() {
            if now < self.music.melody_next_at {
                break;
            }
            let (note, _gap, lane) = song[self.music.melody_index];
            let x = 0.18 + f32::from(lane.clamp(0, 4)) * 0.16;
            self.music.melody_balls.push(MelodyBall {
                x,
                y: 0.10,
                note,
                radius: 0.062,
                born_at: now,
                color: melody_ball_color(note),
                _lane: lane,
            });
            self.music.melody_index += 1;
            if self.music.melody_index < song.len() {
                let next_gap = song[self.music.melody_index].1.max(280);
                self.music.melody_next_at = now.saturating_add(next_gap);
            }
        }

        // Fall + expire misses (in place).
        let step = 0.014 * self.sensitivity.max(0.55);
        let mut missed = false;
        let mut write = 0usize;
        let len = self.music.melody_balls.len();
        for read in 0..len {
            self.music.melody_balls[read].y += step;
            let age = now.saturating_sub(self.music.melody_balls[read].born_at);
            let y = self.music.melody_balls[read].y;
            if y > 0.92 || age > 3000 {
                missed = true;
                continue;
            }
            if write != read {
                self.music.melody_balls.swap(write, read);
            }
            write += 1;
        }
        self.music.melody_balls.truncate(write);
        if missed {
            self.music.melody_combo = 0;
            self.label = "Мимо · дальше!".into();
        }

        // Hit test fingertips only (no heap tips buffer).
        let mut hit_notes = [0u8; 4];
        let mut hit_count = 0usize;
        self.music.melody_balls.retain(|ball| {
            let reach = ball.radius * 1.7;
            let reach2 = reach * reach;
            let hit = hands.iter().any(|hand| {
                [4usize, 8, 12, 16, 20].into_iter().any(|idx| {
                    let p = tip(hand, idx);
                    dist2(p, (ball.x, ball.y)) <= reach2
                })
            });
            if hit && hit_count < hit_notes.len() {
                hit_notes[hit_count] = ball.note;
                hit_count += 1;
                false
            } else {
                !hit
            }
        });

        for note in hit_notes.into_iter().take(hit_count) {
            let vel = self.velocity((0.55 + self.music.melody_combo as f32 * 0.04).min(1.0));
            self.shot(0, note, vel, now + 380);
            self.music.melody_score = self.music.melody_score.saturating_add(1);
            self.music.melody_combo = self.music.melody_combo.saturating_add(1);
            self.music.lit_key = Some((note, now + 200));
            self.label = format!(
                "+{} · комбо {}",
                self.music.melody_score, self.music.melody_combo
            );
            self.last_hit = now;
        }

        // Loop the song when finished and board is clear.
        if self.music.melody_index >= song.len() && self.music.melody_balls.is_empty() {
            self.music.melody_index = 0;
            self.music.melody_next_at = now.saturating_add(700);
            self.label = format!("Круг! Счёт {}", self.music.melody_score);
        }
    }

    pub(super) fn paint_melody(&self, image: &mut RgbImage) {
        // Thin hit line only — full-frame alpha wash was a major FPS killer.
        thick_hline(image, 0.12, 0.88, 0.62, [52, 211, 153], 0.55, 2);

        for lane in 0..5u8 {
            let x = 0.18 + f32::from(lane) * 0.16;
            vline(image, x, 0.10, 0.86, [148, 163, 184], 0.22);
        }

        for ball in &self.music.melody_balls {
            let (cx, cy) = px(image, ball.x, ball.y);
            let radius = (ball.radius * image.width() as f32).round() as i32;
            disc(image, cx, cy, radius, ball.color, 1.0);
            disc(image, cx, cy, (radius / 3).max(2), [255, 255, 255], 1.0);
        }

        // Compact score bar (one rect) instead of dozens of discs.
        let score = self.music.melody_score.min(20) as f32;
        if score > 0.0 {
            fill_rect(image, 0.10, 0.03, score * 0.035, 0.012, [52, 211, 153], 0.9);
        }
    }

    // =========================================================================
    // --- ПРОФЕССИОНАЛЬНАЯ ГИТАРА: РИТМ, АККОРДЫ, СОЛО-ГРИФ И БОЙ ---
    // =========================================================================

    pub(super) fn play_guitar(&mut self, now: u64, hands: &[TrackedHand], pinch_ratio: f32) {
        let capo = self.music.capo;
        let is_bass = self.music.guitar_tone == 4;
        let base_strings = if is_bass { BASS_STRINGS } else { OPEN_STRINGS };

        // 1. ЛЕВАЯ РУКА: ВЫБОР АККОРДА ИЛИ ЗАЖАТИЕ ЛАДОВ НА ГРИФЕ
        let mut palm_mute = false;

        for hand in hands {
            let palm = palm_of(hand);

            if self.music.guitar_mode == 0 {
                // РЕЖИМ 0: СЕТКА АККОРДОВ (слева x < 0.32)
                if palm.0 < 0.32 {
                    if let Some(zone) = chord_at(palm.0, palm.1) {
                        if self.music.chord != zone {
                            self.music.chord = zone;
                        }
                    }
                }
            } else {
                // РЕЖИМ 1: СОЛО НА ГРИФЕ 12 ЛАДОВ (x от 0.12 до 0.58)
                if (0.10..0.60).contains(&palm.0) {
                    for &tip_id in &[8usize, 12] {
                        // Указательный и средний пальцы левой руки
                        let (fx, fy) = tip(hand, tip_id);
                        if let (Some(string_idx), Some(fret_idx)) =
                            (nearest_string(fy, 0.045), fret_at(fx))
                        {
                            self.music.solo_fret[string_idx] = Some(fret_idx);
                        }
                    }
                }
            }

            // Проверка глушения струн ладонью (Palm Mute / Chucking)
            if palm.0 >= 0.35 && palm_mute_active(hands) {
                palm_mute = true;
            }
        }

        // Подтяжка струны (Бенд) и аналоговое вибрато
        self.update_guitar_bend_and_vibrato(now, hands, pinch_ratio);

        // Рассчитываем актуальные ноты струн с учётом каподастра / аккорда / ладов
        let active_notes: [u8; 6] = if self.music.guitar_mode == 0 {
            let chord = &CHORDS[self.music.chord];
            let raw = if palm_mute {
                chord.power
            } else {
                chord.strings
            };
            let mut trans = [0u8; 6];
            for i in 0..6 {
                if raw[i] > 0 {
                    trans[i] = raw[i].saturating_add(capo);
                }
            }
            trans
        } else {
            // Соло: базовый строй + зажатый лад (или открытая струна) + каподастр
            let mut solo = [0u8; 6];
            for i in 0..6 {
                let fret = self.music.solo_fret[i].unwrap_or(0);
                solo[i] = base_strings[i].saturating_add(fret).saturating_add(capo);
            }
            solo
        };

        // 2. ПРАВАЯ РУКА: БОЙ (STRUM) ИЛИ ИНДИВИДУАЛЬНЫЙ ПЕРЕБОР СТРУН
        for (hand_idx, hand) in hands.iter().enumerate() {
            let palm = palm_of(hand);
            if palm.0 < 0.34 {
                continue;
            } // Пропускаем левую руку

            let Some(sample) = self.prev_for(hand) else {
                continue;
            };
            let dt = frame_dt(sample.at, now);
            let alpha = ema_alpha(dt, 0.08);

            let dy = palm.1 - sample.palm[1];
            let wrist_spd = ema(
                sample.wrist_speed,
                dist2(palm, (sample.palm[0], sample.palm[1])) / dt,
                alpha,
            );

            let sens = self.sensitivity.max(0.35);
            // Strum needs a clear wrist sweep so gentle finger picks stay precise.
            let strum_threshold_y = 0.028 / sens;
            let strum_spd_threshold = 0.52 / sens;

            // А. ПОЛНОЦЕННЫЙ ГИТАРНЫЙ БОЙ (Strum с арпеджиацией и акцентами)
            if dy.abs() > strum_threshold_y
                && wrist_spd > strum_spd_threshold
                && now.saturating_sub(self.last_hit) > 110
            {
                let down = dy > 0.0;
                self.strum_arpeggiated(now, active_notes, down, palm_mute, wrist_spd);
                continue;
            }

            // Б. ПЕРЕБОР СТРУН ПАЛЬЦАМИ — только реальное пересечение струны.
            for (finger_idx, &landmark) in TIPS.iter().enumerate() {
                let shape = hand.world.as_ref().unwrap_or(&hand.landmarks);
                let joints = match finger_idx {
                    0 => [1, 2, 3, 4],
                    1 => [5, 6, 7, 8],
                    2 => [9, 10, 11, 12],
                    3 => [13, 14, 15, 16],
                    _ => [17, 18, 19, 20],
                };
                if finger_curl(shape, joints) > 0.68 {
                    continue;
                }

                let (x, y) = tip(hand, landmark);
                if !(0.34..0.98).contains(&x) || !(0.18..0.76).contains(&y) {
                    continue;
                }

                let prev = sample.tips[finger_idx];
                let tip_speed = dist2((x, y), (prev[0], prev[1])) / dt;
                if tip_speed < 0.065 / sens {
                    continue;
                }

                let cooled = self.music.pluck_at.iter().any(|(h, f, at)| {
                    *h == hand_idx as u8 && *f == finger_idx as u8 && now.saturating_sub(*at) < 42
                });
                if cooled {
                    continue;
                }

                let Some((string_idx, cross_frac)) = crossed_string_subframe(prev[1], y) else {
                    continue;
                };
                if now.saturating_sub(self.music.string_hit_at[string_idx]) < 48 {
                    continue;
                }

                let note = active_notes[string_idx];
                if note == 0 {
                    continue;
                }

                let hit_at =
                    sample.at + ((now.saturating_sub(sample.at) as f32) * cross_frac) as u64;
                self.music.pluck_at.retain(|(h, f, at)| {
                    *h != hand_idx as u8
                        || *f != finger_idx as u8
                        || now.saturating_sub(*at) < 280
                });
                self.music
                    .pluck_at
                    .push((hand_idx as u8, finger_idx as u8, hit_at));
                self.music.string_hit_at[string_idx] = hit_at;

                let mut vel = self.velocity((0.40 + tip_speed / 1.85).min(1.0));
                if palm_mute {
                    vel = (vel as f32 * 0.75).round() as u8;
                }

                // Sample audio is already short; MIDI off stays brief for fallback.
                let life = match self.music.guitar_tone {
                    1 => 420,
                    2 => 480,
                    3 => 560,
                    4 => 360,
                    _ => 320,
                };

                self.shot(0, note, vel, hit_at + if palm_mute { 120 } else { life });
                self.music.string_energy[string_idx] = 1.0;

                if self.music.guitar_mode == 0 {
                    self.label = format!(
                        "{} · Струна {}",
                        CHORDS[self.music.chord].name,
                        6 - string_idx
                    );
                } else {
                    let fret = self.music.solo_fret[string_idx].unwrap_or(0);
                    self.label = format!("Струна {} · Лад {}", 6 - string_idx, fret);
                }
                self.last_hit = hit_at;
            }
        }

        // Очищаем лады в соло-режиме, если пальцы убраны
        if self.music.guitar_mode == 1 && hands.is_empty() {
            self.music.solo_fret = [None; 6];
        }
    }

    // Реалистичный арпеджированный гитарный бой (Strum)
    fn strum_arpeggiated(
        &mut self,
        now: u64,
        notes: [u8; 6],
        down: bool,
        palm_mute: bool,
        speed: f32,
    ) {
        let base_vel = self.velocity((0.35 + (speed / 2.8).min(0.65)).clamp(0.15, 1.0));
        let vel = if palm_mute {
            (base_vel as f32 * 0.70).round() as u8
        } else {
            base_vel
        };

        let life = match self.music.guitar_tone {
            1 => 460,
            2 => 520,
            3 => 620,
            4 => 380,
            _ => 360,
        };

        // Snappy arpeggio gap — fast sweeps almost land together.
        let string_delay_ms = ((16.0 - (speed * 6.0).min(12.0)).max(4.0)) as u64;

        let indices: Vec<usize> = if down {
            (0..6).collect() // Бой вниз: от 6-й к 1-й струне
        } else {
            (0..6).rev().collect() // Бой вверх: от 1-й к 6-й струне
        };

        for (step, &idx) in indices.iter().enumerate() {
            let note = notes[idx];
            if note == 0 {
                continue;
            }

            // Акценты: бой вниз громче на басах, бой вверх громче на тонких струнах
            let accent = if down {
                1.08 - (idx as f32 * 0.03)
            } else {
                0.90 + (idx as f32 * 0.04)
            };
            let note_vel = ((vel as f32 * accent).round() as u8).clamp(1, 127);

            let strike_at = now + (step as u64 * string_delay_ms);
            self.schedule_note(
                0,
                note,
                note_vel,
                strike_at,
                strike_at + if palm_mute { 110 } else { life },
            );
            self.music.string_energy[idx] = 1.0;
            self.music.string_hit_at[idx] = strike_at;
        }

        let way = if down {
            "↓ Вниз"
        } else {
            "↑ Вверх"
        };
        if self.music.guitar_mode == 0 {
            let chord = CHORDS[self.music.chord].name;
            self.label = if palm_mute {
                format!("{chord} · Palm Mute {way}")
            } else {
                format!("{chord} · Бой {way}")
            };
        } else {
            self.label = if palm_mute {
                format!("Гриф · Chug {way}")
            } else {
                format!("Гриф · Чёс {way}")
            };
        }
        self.last_hit = now;
    }

    // Подтяжка струн (Pitch Bend) и аналоговое вибрато
    pub(super) fn update_guitar_bend_and_vibrato(
        &mut self,
        now: u64,
        hands: &[TrackedHand],
        pinch_ratio: f32,
    ) {
        let pinching = hands.iter().find(|hand| {
            let palm = palm_of(hand);
            (0.12..0.60).contains(&palm.0)
                && crate::gestures::pinch_active(
                    &hand.landmarks,
                    self.music.bend_active,
                    pinch_ratio,
                )
        });

        if let Some(hand) = pinching {
            let tip = tip(hand, 8);
            if !self.music.bend_active {
                self.music.bend_active = true;
                self.music.bend_origin_y = Some(tip.1);
                return;
            }

            let origin = self.music.bend_origin_y.unwrap_or(tip.1);
            let delta = (origin - tip.1).clamp(-0.18, 0.18);
            let amount = (delta / 0.12).clamp(-1.0, 1.0);
            let value = (8192.0 + amount * 3400.0).round() as i32;

            if let Some(midi) = &self.music.midi {
                midi.pitch_bend(0, value.clamp(0, 16_383) as u16);
            }
            if amount.abs() > 0.08 {
                self.label = format!("Бенд · {:+.0}%", amount * 100.0);
                self.last_hit = now;
            }
        } else if self.music.bend_active {
            self.music.bend_active = false;
            self.music.bend_origin_y = None;
            if let Some(midi) = &self.music.midi {
                midi.pitch_bend(0, 8192); // возврат строя
            }
        }
    }

    // Установка параметров гитары
    pub(super) fn configure_guitar_mode(&mut self, mode: u8) {
        self.music.guitar_mode = mode;
        self.silence();
    }

    pub(super) fn set_guitar_tone(&mut self, tone: u8) {
        self.music.guitar_tone = tone;
        self.apply_program();
    }

    pub(super) fn set_capo(&mut self, capo: u8) {
        self.music.capo = capo.min(7);
        self.silence();
    }

    pub(super) fn guitar_audio_available(&self) -> bool {
        self.music.guitar_audio.is_some()
    }

    pub(super) fn piano_audio_available(&self) -> bool {
        self.music.piano_audio.is_some()
    }

    pub(super) fn ensure_guitar_audio(&mut self) {
        if self.music.guitar_audio.is_some() || self.music.guitar_audio_failed {
            return;
        }
        if cfg!(test) {
            return;
        }
        match GuitarAudio::open() {
            Ok(audio) => self.music.guitar_audio = Some(audio),
            Err(error) => {
                tracing::warn!(%error, "guitar sample audio could not be opened");
                self.music.guitar_audio_failed = true;
            }
        }
    }

    pub(super) fn ensure_piano_audio(&mut self) {
        if self.music.piano_audio.is_some() || self.music.piano_audio_failed {
            return;
        }
        if cfg!(test) {
            return;
        }
        match PianoAudio::open() {
            Ok(audio) => self.music.piano_audio = Some(audio),
            Err(error) => {
                tracing::warn!(%error, "piano sample audio could not be opened");
                self.music.piano_audio_failed = true;
            }
        }
    }

    pub(super) fn preview_guitar_string(&mut self, string: usize, now: u64) {
        if self.music.instrument != 2 || self.music.muted {
            return;
        }
        let Some(note) = self.active_guitar_notes().get(string).copied() else {
            return;
        };
        if note == 0 {
            return;
        }
        let velocity = self.velocity(0.78);
        self.shot(0, note, velocity, now.saturating_add(500));
        self.music.string_energy[string] = 1.0;
        let string_number = 6 - string;
        self.label = format!("{} · струна {string_number}", CHORDS[self.music.chord].name);
        self.last_hit = now;
    }

    fn active_guitar_notes(&self) -> [u8; 6] {
        let capo = self.music.capo;
        let is_bass = self.music.guitar_tone == 4;
        let base_strings = if is_bass { BASS_STRINGS } else { OPEN_STRINGS };

        if self.music.guitar_mode == 0 {
            let chord = &CHORDS[self.music.chord];
            std::array::from_fn(|index| {
                if chord.strings[index] == 0 {
                    0
                } else {
                    chord.strings[index].saturating_add(capo)
                }
            })
        } else {
            std::array::from_fn(|index| {
                base_strings[index]
                    .saturating_add(self.music.solo_fret[index].unwrap_or(0))
                    .saturating_add(capo)
            })
        }
    }

    // --- ОТРИСОВКА ГИТАРЫ (drawn into play_preview only, not the shared camera frame) ---

    pub(super) fn paint_guitar(&self, image: &mut RgbImage) {
        let fretboard_x = 0.28;
        let fretboard_w = 0.70;
        let fretboard_y = 0.20;
        let fretboard_h = 0.54;

        // Light fretboard frame (avoid full-area alpha wash).
        thick_hline(
            image,
            fretboard_x,
            fretboard_x + fretboard_w,
            fretboard_y,
            [180, 185, 195],
            0.45,
            2,
        );
        thick_hline(
            image,
            fretboard_x,
            fretboard_x + fretboard_w,
            fretboard_y + fretboard_h,
            [180, 185, 195],
            0.45,
            2,
        );
        vline(
            image,
            0.34,
            fretboard_y,
            fretboard_y + fretboard_h,
            [16, 185, 129],
            0.55,
        );

        // 2. Chord pads (left) or nut marker (solo)
        if self.music.guitar_mode == 0 {
            for (index, chord) in CHORDS.iter().enumerate() {
                let (x, y, w, h) = chord_rect(index);
                let lit = index == self.music.chord;
                fill_rect(
                    image,
                    x,
                    y,
                    w,
                    h,
                    chord.color,
                    if lit { 0.78 } else { 0.16 },
                );
                if lit {
                    fill_rect(image, x, y, w, 0.008, [255, 255, 255], 0.85);
                }
            }
        } else {
            fill_rect(
                image,
                fretboard_x - 0.012,
                fretboard_y,
                0.012,
                fretboard_h,
                [240, 235, 220],
                0.85,
            );
        }

        // 3. Ладовые порожки (12 ладов) и инкрустация (3, 5, 7, 9, 12 лады)
        let num_frets = 12;
        let fret_step = fretboard_w / num_frets as f32;

        for fret in 1..=num_frets {
            let fx = fretboard_x + fret as f32 * fret_step;
            // Ладовая проволока
            vline(
                image,
                fx,
                fretboard_y,
                fretboard_y + fretboard_h,
                [180, 185, 195],
                0.65,
            );

            // Перламутровые маркеры на грифе
            let marker_x = fx - fret_step * 0.5;
            let mid_y = fretboard_y + fretboard_h * 0.5;

            if fret == 3 || fret == 5 || fret == 7 || fret == 9 {
                let (cx, cy) = px(image, marker_x, mid_y);
                disc(image, cx, cy, 5, [220, 225, 230], 0.5);
            } else if fret == 12 {
                // Двойная точка на 12 ладу (октава)
                let (cx1, cy1) = px(image, marker_x, mid_y - 0.08);
                let (cx2, cy2) = px(image, marker_x, mid_y + 0.08);
                disc(image, cx1, cy1, 5, [220, 225, 230], 0.55);
                disc(image, cx2, cy2, 5, [220, 225, 230], 0.55);
            }
        }

        // 4. Strings — light overlay; heavy sine strokes were a major FPS killer.
        for (index, &y) in STRING_Y.iter().enumerate() {
            let energy = self.music.string_energy[index].clamp(0.0, 1.0);
            let base_thick = STRING_THICKNESS[index];
            let alpha = 0.48 + energy * 0.52;

            let color = if energy > 0.18 {
                [255, 236, 150]
            } else if index < 3 {
                [195, 145, 90]
            } else {
                [220, 225, 235]
            };

            let thickness = (base_thick + energy * 2.8).round().max(2.0) as i32;
            // Always draw a solid string — vibration only when hit, so the play target stays clear.
            thick_hline(
                image,
                fretboard_x - 0.02,
                0.98,
                y,
                color,
                alpha.max(0.62),
                thickness,
            );
            if energy > 0.14 {
                let (cx, cy) = px(image, 0.64, y);
                disc(
                    image,
                    cx,
                    cy,
                    (5.0 + energy * 7.0) as i32,
                    [255, 240, 160],
                    energy * 0.55,
                );
            }

            if self.music.guitar_mode == 1 {
                if let Some(fret) = self.music.solo_fret[index] {
                    if fret > 0 && fret <= 12 {
                        let fx = fretboard_x + (fret as f32 - 0.5) * fret_step;
                        let (cx, cy) = px(image, fx, y);
                        disc(image, cx, cy, 7, [52, 211, 153], 0.85);
                        ring(image, cx, cy, 11.0, [255, 255, 255], 0.85);
                    }
                }
            }
        }
    }

    // Затухание струн (физический decay)
    pub(super) fn decay_strings(&mut self, now: u64) {
        let dt = if self.music.last_decay_ms == 0 {
            1.0 / 30.0
        } else {
            frame_dt(self.music.last_decay_ms, now)
        };
        self.music.last_decay_ms = now;

        // Скорость затухания зависит от тембра
        let decay_rate = match self.music.guitar_tone {
            3 => 0.34,
            2 => 0.40,
            1 => 0.46,
            _ => 0.55,
        };

        let keep = 1.0 - ema_alpha(dt, decay_rate);
        for (index, energy) in self.music.string_energy.iter_mut().enumerate() {
            *energy *= keep;
            if *energy < 0.01 {
                *energy = 0.0;
            }
            self.music.string_phase[index] = (self.music.string_phase[index]
                + dt * (28.0 + *energy * 44.0))
                % std::f32::consts::TAU;
        }
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

    pub(super) fn mark_piano_hit(&mut self, now: u64, name: &str, note: u8) {
        self.music.lit_key = Some((note, now + 180));
        self.label = name.into();
        self.last_hit = now;
    }

    pub(super) fn paint_music(&self, image: &mut RgbImage) {
        match self.music.instrument {
            0 => self.paint_piano(image),
            1 => self.paint_drums(image),
            2 => self.paint_guitar(image),
            _ => self.paint_melody(image),
        }
    }

    pub(super) fn paint_piano(&self, image: &mut RgbImage) {
        let margin = 0.04;
        let top = 0.60;
        let height = 0.35;
        let total_w = 1.0 - margin * 2.0;
        let white_w = total_w / WHITE_KEYS.len() as f32;

        fill_rect(
            image,
            margin - 0.01,
            top - 0.03,
            total_w + 0.02,
            height + 0.04,
            [16, 20, 26],
            0.65,
        );

        for (i, key) in WHITE_KEYS.iter().enumerate() {
            let x = margin + i as f32 * white_w;
            let is_lit = self.music.lit_key.is_some_and(|(n, _)| n == key.note);
            let color = if is_lit {
                [110, 250, 190]
            } else {
                [248, 249, 252]
            };

            fill_rect(
                image,
                x + white_w * 0.03,
                top,
                white_w * 0.94,
                height,
                color,
                if is_lit { 0.95 } else { 0.72 },
            );
            fill_rect(
                image,
                x + white_w * 0.03,
                top + height - 0.015,
                white_w * 0.94,
                0.015,
                [200, 200, 205],
                0.9,
            );
        }

        let black_w = white_w * 0.62;
        let black_h = height * 0.62;
        for key in BLACK_KEYS {
            let x = margin + key.left_offset_ratio * white_w - (black_w * 0.5);
            let is_lit = self.music.lit_key.is_some_and(|(n, _)| n == key.note);
            let color = if is_lit { [52, 211, 153] } else { [24, 28, 36] };

            fill_rect(image, x, top, black_w, black_h, color, 0.95);
            fill_rect(
                image,
                x + 0.001,
                top,
                black_w - 0.002,
                0.01,
                [80, 85, 95],
                0.7,
            );
        }

        if self.music.sustain_active {
            let (cx, cy) = px(image, margin + 0.03, top - 0.015);
            disc(image, cx, cy, 7, [52, 211, 153], 0.9);
        }
    }

    pub(super) fn paint_drums(&self, image: &mut RgbImage) {
        for (index, drum) in DRUMS.iter().enumerate() {
            let (cx, cy) = px(image, drum.x, drum.y);
            let radius = (drum.r * image.width() as f32).round() as i32;
            let lit = self.music.lit_drum.is_some_and(|(zone, _)| zone == index);

            disc(image, cx, cy, radius + 8, [15, 18, 24], 0.35);
            disc(
                image,
                cx,
                cy,
                radius,
                drum.color,
                if lit { 0.85 } else { 0.40 },
            );
            ring(
                image,
                cx,
                cy,
                radius as f32,
                [255, 255, 255],
                if lit { 0.98 } else { 0.55 },
            );
            ring(image, cx, cy, radius as f32 * 0.75, drum.color, 0.8);

            if lit {
                ring(image, cx, cy, radius as f32 + 16.0, [255, 255, 255], 0.6);
                ring(image, cx, cy, radius as f32 + 28.0, drum.color, 0.3);
            }
        }
    }

    pub(super) fn paint_hits(&self, _image: &mut RgbImage) {}

    pub(super) fn switched_instrument(&mut self, now: u64, hands: &[TrackedHand]) -> bool {
        if self.mode != 1 || hands.len() < 2 {
            self.music.swipe_armed = true;
            return false;
        }
        if now.saturating_sub(self.music.last_switch) < 700 {
            return false;
        }
        let left_i = if palm_of(&hands[0]).0 <= palm_of(&hands[1]).0 {
            0
        } else {
            1
        };
        let right_i = 1 - left_i;
        if palm_of(&hands[right_i]).0 - palm_of(&hands[left_i]).0 < 0.14 {
            self.music.swipe_armed = true;
            return false;
        }
        let (Some(left_prev), Some(right_prev)) = (
            self.prev_for(&hands[left_i]),
            self.prev_for(&hands[right_i]),
        ) else {
            return false;
        };
        let dt = frame_dt(left_prev.at, now);
        let threshold = -0.90 / self.sensitivity.max(0.4);
        let left_vy = (palm_of(&hands[left_i]).1 - left_prev.palm[1]) / dt;
        let right_vy = (palm_of(&hands[right_i]).1 - right_prev.palm[1]) / dt;
        let both_up = left_vy < threshold && right_vy < threshold;
        if !both_up {
            if left_vy > threshold * 0.35 && right_vy > threshold * 0.35 {
                self.music.swipe_armed = true;
            }
            return false;
        }
        if !self.music.swipe_armed {
            return false;
        }
        self.music.swipe_armed = false;
        self.music.last_switch = now;
        self.music.instrument = (self.music.instrument + 1) % 4;
        self.silence();
        self.clear_music_holds();
        if self.music.instrument == 2 {
            self.ensure_guitar_audio();
        } else if matches!(self.music.instrument, 0 | 3) {
            self.ensure_piano_audio();
            if self.music.instrument == 3 {
                self.reset_melody(now);
            }
        }
        self.apply_program();
        self.label = match self.music.instrument {
            0 => "Пианино",
            1 => "Барабаны",
            2 => "Гитара",
            _ => "Мелодия",
        }
        .into();
        self.last_hit = now;
        true
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
}

// Вспомогательные функции

pub(super) fn find_piano_key(x: f32, y: f32) -> Option<(u8, &'static str)> {
    let margin = 0.04;
    let top = 0.60;
    let height = 0.35;
    if !(top..=top + height).contains(&y) || !(margin..=1.0 - margin).contains(&x) {
        return None;
    }

    let total_w = 1.0 - margin * 2.0;
    let white_w = total_w / WHITE_KEYS.len() as f32;
    let black_w = white_w * 0.62;
    let black_h = height * 0.62;

    if y <= top + black_h {
        for key in BLACK_KEYS {
            let kx = margin + key.left_offset_ratio * white_w - (black_w * 0.5);
            if (kx..=kx + black_w).contains(&x) {
                return Some((key.note, key.name));
            }
        }
    }

    let idx = ((x - margin) / white_w).floor() as usize;
    WHITE_KEYS.get(idx).map(|k| (k.note, k.name))
}

pub(super) fn fret_at(x: f32) -> Option<u8> {
    let fretboard_x = 0.28;
    let fretboard_w = 0.70;
    if !(fretboard_x..=fretboard_x + fretboard_w).contains(&x) {
        return None;
    }
    let step = fretboard_w / 12.0;
    let fret = ((x - fretboard_x) / step).floor() as u8 + 1;
    Some(fret.clamp(1, 12))
}

pub(super) fn palm_mute_active(hands: &[TrackedHand]) -> bool {
    let median_string = (STRING_Y[2] + STRING_Y[3]) * 0.5;
    hands.iter().any(|hand| {
        let palm = palm_of(hand);
        if palm.0 < 0.35 {
            return false;
        }
        if !(STRING_Y[0] - 0.04..STRING_Y[5] + 0.10).contains(&palm.1) {
            return false;
        }
        if palm.1 < median_string - 0.02 {
            return false;
        }
        let open = hand_openness(&hand.landmarks);
        (0.32..0.92).contains(&open)
    })
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

pub(super) fn chord_rect(index: usize) -> (f32, f32, f32, f32) {
    let col = if index < 8 { 0.0 } else { 1.0 };
    let row = if index < 8 { index } else { index - 8 };
    (0.015 + col * 0.125, 0.15 + row as f32 * 0.082, 0.115, 0.074)
}

pub(super) fn chord_at(x: f32, y: f32) -> Option<usize> {
    (0..CHORDS.len()).find(|index| {
        let (zx, zy, w, h) = chord_rect(*index);
        (zx..zx + w).contains(&x) && (zy..zy + h).contains(&y)
    })
}

pub(super) fn nearest_string(y: f32, reach: f32) -> Option<usize> {
    STRING_Y
        .iter()
        .enumerate()
        .filter(|(_, string_y)| (y - *string_y).abs() <= reach)
        .min_by(|left, right| (y - *left.1).abs().total_cmp(&(y - *right.1).abs()))
        .map(|(index, _)| index)
}

pub(super) fn crossed_string_subframe(prev_y: f32, y: f32) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32, f32)> = None;
    let travel = (prev_y - y).abs();
    // Allow slightly larger jumps so fast picks still register on lower FPS.
    if travel < 1e-5 || travel > 0.22 {
        return None;
    }
    for (index, string_y) in STRING_Y.iter().enumerate() {
        let crossed = (prev_y - string_y) * (y - string_y) <= 0.0;
        if !crossed {
            continue;
        }
        let denom = y - prev_y;
        let frac = if denom.abs() < 1e-6 {
            0.5
        } else {
            ((string_y - prev_y) / denom).clamp(0.0, 1.0)
        };
        let gap = (y - string_y).abs();
        if best.is_none_or(|(_, other_gap, _)| gap < other_gap) {
            best = Some((index, gap, frac));
        }
    }
    best.map(|(index, _, frac)| (index, frac))
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
