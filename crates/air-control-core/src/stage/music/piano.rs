//! Instrument submodule.
use super::*;

// --- ПИАНИНО (2 октавы C3..C5: 15 белых + 10 чёрных клавиш) ---

// Геометрия клавиатуры в нормализованных координатах кадра.
const PIANO_MARGIN: f32 = 0.04;
const PIANO_TOP: f32 = 0.60;
const PIANO_HEIGHT: f32 = 0.35;
const BLACK_W_RATIO: f32 = 0.62;
const BLACK_H_RATIO: f32 = 0.62;

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
    /// ЛЕВЫЙ край клавиши в долях ширины белой клавиши (0.65 → клавиша начинается
    /// на 65% первой белой и заканчивается на 127%, т.е. стоит ровно на стыке C/D).
    pub left_offset_ratio: f32,
}

const WHITE_KEYS: &[WhiteKey] = &[
    WhiteKey { name: "C3", note: 48, white_idx: 0 },
    WhiteKey { name: "D3", note: 50, white_idx: 1 },
    WhiteKey { name: "E3", note: 52, white_idx: 2 },
    WhiteKey { name: "F3", note: 53, white_idx: 3 },
    WhiteKey { name: "G3", note: 55, white_idx: 4 },
    WhiteKey { name: "A3", note: 57, white_idx: 5 },
    WhiteKey { name: "B3", note: 59, white_idx: 6 },
    WhiteKey { name: "C4", note: 60, white_idx: 7 },
    WhiteKey { name: "D4", note: 62, white_idx: 8 },
    WhiteKey { name: "E4", note: 64, white_idx: 9 },
    WhiteKey { name: "F4", note: 65, white_idx: 10 },
    WhiteKey { name: "G4", note: 67, white_idx: 11 },
    WhiteKey { name: "A4", note: 69, white_idx: 12 },
    WhiteKey { name: "B4", note: 71, white_idx: 13 },
    WhiteKey { name: "C5", note: 72, white_idx: 14 },
];

const BLACK_KEYS: &[BlackKey] = &[
    BlackKey { name: "C#3", note: 49, left_offset_ratio: 0.65 },
    BlackKey { name: "D#3", note: 51, left_offset_ratio: 1.70 },
    BlackKey { name: "F#3", note: 54, left_offset_ratio: 3.65 },
    BlackKey { name: "G#3", note: 56, left_offset_ratio: 4.68 },
    BlackKey { name: "A#3", note: 58, left_offset_ratio: 5.72 },
    BlackKey { name: "C#4", note: 61, left_offset_ratio: 7.65 },
    BlackKey { name: "D#4", note: 63, left_offset_ratio: 8.70 },
    BlackKey { name: "F#4", note: 66, left_offset_ratio: 10.65 },
    BlackKey { name: "G#4", note: 68, left_offset_ratio: 11.68 },
    BlackKey { name: "A#4", note: 70, left_offset_ratio: 12.72 },
];

fn white_key_width() -> f32 {
    (1.0 - PIANO_MARGIN * 2.0) / WHITE_KEYS.len() as f32
}

/// Левый край чёрной клавиши (раньше из него по ошибке вычиталась половина ширины,
/// из-за чего клавиши рисовались и ловили касания на ~0.3 белой левее, чем нужно).
fn black_left(key: &BlackKey) -> f32 {
    PIANO_MARGIN + key.left_offset_ratio * white_key_width()
}

/// Имя клавиши по MIDI-ноте.
pub(crate) fn piano_key_name(note: u8) -> Option<&'static str> {
    WHITE_KEYS
        .iter()
        .find(|k| k.note == note)
        .map(|k| k.name)
        .or_else(|| BLACK_KEYS.iter().find(|k| k.note == note).map(|k| k.name))
}

/// Точка на клавише, куда рисуем маркер «палец стоит здесь».
fn key_anchor(note: u8) -> Option<(f32, f32)> {
    let white_w = white_key_width();
    if let Some(key) = WHITE_KEYS.iter().find(|k| k.note == note) {
        return Some((
            PIANO_MARGIN + (key.white_idx as f32 + 0.5) * white_w,
            PIANO_TOP + PIANO_HEIGHT - 0.095,
        ));
    }
    let black_h = PIANO_HEIGHT * BLACK_H_RATIO;
    BLACK_KEYS.iter().find(|k| k.note == note).map(|key| {
        (
            black_left(key) + white_w * BLACK_W_RATIO * 0.5,
            PIANO_TOP + black_h - 0.07,
        )
    })
}

impl Stage {
    pub(crate) fn play_piano(&mut self, now: u64, hands: &[TrackedHand]) {
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

    pub(crate) fn ensure_piano_audio(&mut self) {
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

    pub(crate) fn piano_audio_available(&self) -> bool {
        self.music.piano_audio.is_some()
    }

    pub(crate) fn mark_piano_hit(&mut self, now: u64, name: &str, note: u8) {
        self.music.lit_key = Some((note, now + 180));
        self.label = name.into();
        self.last_hit = now;
    }

    /// Нота по клику мышью на экранной клавиатуре (MIDI 48..=72).
    /// Подключается так же, как `preview_guitar_string`.
    pub(crate) fn preview_piano_key(&mut self, note: u8, now: u64) {
        if self.music.instrument != 0 || self.music.muted {
            return;
        }
        let Some(name) = piano_key_name(note) else {
            return;
        };
        let velocity = self.velocity(0.82);
        self.shot(0, note, velocity, now.saturating_add(700));
        self.mark_piano_hit(now, name, note);
    }

    pub(crate) fn paint_piano(&self, image: &mut RgbImage) {
        let margin = PIANO_MARGIN;
        let top = PIANO_TOP;
        let height = PIANO_HEIGHT;
        let total_w = 1.0 - margin * 2.0;
        let white_w = white_key_width();
        let black_w = white_w * BLACK_W_RATIO;
        let black_h = height * BLACK_H_RATIO;

        // Клавиша «нажата», если под пальцем (удерживается) или только что сыграна.
        let is_down = |note: u8| {
            self.music.holds.iter().any(|h| h.note == note)
                || self.music.lit_key.is_some_and(|(n, _)| n == note)
        };

        // Корпус: тень, фоллборд, красное сукно.
        fill_rect(
            image,
            margin - 0.014,
            top - 0.050,
            total_w + 0.028,
            height + 0.075,
            [10, 12, 16],
            0.62,
        );
        fill_rect(
            image,
            margin - 0.014,
            top - 0.050,
            total_w + 0.028,
            0.034,
            [34, 38, 48],
            0.88,
        );
        fill_rect(image, margin, top - 0.012, total_w, 0.007, [190, 36, 58], 0.9);

        let preset_name = ["SOFT", "BRIGHT", "HARP"][self.music.piano_preset.min(2) as usize];
        draw_label(image, margin + 0.008, top - 0.046, "GRAND", [226, 232, 240], 0.85, 1);
        draw_label(image, margin + 0.09, top - 0.046, preset_name, [110, 250, 190], 0.85, 1);

        // Белые клавиши.
        for (i, key) in WHITE_KEYS.iter().enumerate() {
            let x = margin + i as f32 * white_w;
            let kx = x + white_w * 0.03;
            let kw = white_w * 0.94;
            let down = is_down(key.note);

            if down {
                // Нажатая клавиша: мятный градиент из двух полос.
                fill_rect(image, kx, top, kw, height * 0.55, [160, 247, 196], 0.95);
                fill_rect(
                    image,
                    kx,
                    top + height * 0.55,
                    kw,
                    height * 0.45,
                    [52, 211, 153],
                    0.95,
                );
            } else {
                fill_rect(image, kx, top, kw, height * 0.70, [252, 252, 254], 0.76);
                fill_rect(
                    image,
                    kx,
                    top + height * 0.70,
                    kw,
                    height * 0.30,
                    [236, 237, 242],
                    0.78,
                );
                // Нижняя кромка клавиши.
                fill_rect(image, kx, top + height - 0.016, kw, 0.016, [196, 198, 206], 0.92);
            }
            // Боковая фаска.
            fill_rect(image, kx, top, white_w * 0.05, height, [0, 0, 0], 0.07);

            let is_c = key.name.starts_with('C');
            draw_label(
                image,
                kx + kw * 0.20,
                top + height - 0.062,
                key.name,
                if down {
                    [255, 255, 255]
                } else if is_c {
                    [40, 44, 52]
                } else {
                    [128, 134, 146]
                },
                0.95,
                1,
            );
        }

        // Чёрные клавиши (теперь стоят ровно на стыках белых).
        for key in BLACK_KEYS {
            let x = black_left(key);
            let down = is_down(key.note);
            let (body, bevel, gloss) = if down {
                ([52, 211, 153], [110, 250, 190], [180, 255, 220])
            } else {
                ([22, 24, 30], [58, 62, 72], [96, 104, 122])
            };

            fill_rect(image, x, top, black_w, black_h, body, 0.97);
            fill_rect(image, x, top, black_w * 0.10, black_h, bevel, 0.75);
            fill_rect(image, x + black_w * 0.90, top, black_w * 0.10, black_h, bevel, 0.55);
            fill_rect(
                image,
                x + black_w * 0.18,
                top,
                black_w * 0.64,
                black_h * 0.55,
                gloss,
                0.30,
            );
            // Передняя грань.
            fill_rect(
                image,
                x,
                top + black_h - 0.022,
                black_w,
                0.022,
                if down { [34, 170, 120] } else { [48, 52, 62] },
                0.95,
            );
        }

        // Маркеры пальцев: где сейчас «стоят» удерживаемые ноты.
        for hold in &self.music.holds {
            if let Some((ax, ay)) = key_anchor(hold.note) {
                let (cx, cy) = px(image, ax, ay);
                disc(image, cx, cy, 10, [255, 255, 255], 0.92);
                disc(image, cx, cy, 6, [52, 199, 89], 1.0);
            }
        }

        // Зона педали сустейна (кулак в левом верхнем углу).
        let active = self.music.sustain_active;
        let pad = if active { [52, 211, 153] } else { [255, 255, 255] };
        fill_rect(image, 0.02, 0.06, 0.20, 0.34, pad, if active { 0.34 } else { 0.09 });
        thick_hline(image, 0.02, 0.22, 0.06, pad, if active { 0.9 } else { 0.35 }, 2);
        thick_hline(image, 0.02, 0.22, 0.40, pad, if active { 0.9 } else { 0.35 }, 2);
        draw_label(image, 0.04, 0.10, "FIST", [255, 255, 255], 0.9, 2);
        draw_label(
            image,
            0.04,
            0.17,
            if active { "SUSTAIN ON" } else { "SUSTAIN" },
            if active { [220, 255, 235] } else { [226, 232, 240] },
            0.9,
            1,
        );
    }
}

pub(crate) fn find_piano_key(x: f32, y: f32) -> Option<(u8, &'static str)> {
    let margin = PIANO_MARGIN;
    let top = PIANO_TOP;
    let height = PIANO_HEIGHT;
    if !(top..=top + height).contains(&y) || !(margin..=1.0 - margin).contains(&x) {
        return None;
    }

    let white_w = white_key_width();
    let black_w = white_w * BLACK_W_RATIO;
    let black_h = height * BLACK_H_RATIO;

    if y <= top + black_h {
        for key in BLACK_KEYS {
            let kx = black_left(key);
            if (kx..=kx + black_w).contains(&x) {
                return Some((key.note, key.name));
            }
        }
    }

    let idx = ((x - margin) / white_w).floor() as usize;
    WHITE_KEYS.get(idx).map(|k| (k.note, k.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_keys_sit_on_white_key_boundaries() {
        for key in BLACK_KEYS {
            let center = key.left_offset_ratio + BLACK_W_RATIO * 0.5;
            let nearest_boundary = center.round();
            assert!(
                (center - nearest_boundary).abs() < 0.12,
                "{} is not centred on a white-key boundary ({center})",
                key.name
            );
        }
    }

    #[test]
    fn find_key_distinguishes_black_and_white() {
        let white_w = white_key_width();
        // Центр C#3 на верхней половине клавиши.
        let cx = PIANO_MARGIN + (0.65 + BLACK_W_RATIO * 0.5) * white_w;
        assert_eq!(find_piano_key(cx, PIANO_TOP + 0.05), Some((49, "C#3")));
        // Под чёрными клавишами — белая клавиша того же места.
        let below = PIANO_TOP + PIANO_HEIGHT * 0.9;
        assert!(matches!(find_piano_key(cx, below), Some((48 | 50, _))));
        // Левая часть C3 — белая клавиша даже на уровне чёрных.
        let left = PIANO_MARGIN + 0.3 * white_w;
        assert_eq!(find_piano_key(left, PIANO_TOP + 0.05), Some((48, "C3")));
        // Вне клавиатуры.
        assert_eq!(find_piano_key(0.5, 0.2), None);
    }

    #[test]
    fn piano_key_names_cover_the_range() {
        assert_eq!(piano_key_name(60), Some("C4"));
        assert_eq!(piano_key_name(61), Some("C#4"));
        assert_eq!(piano_key_name(72), Some("C5"));
        assert_eq!(piano_key_name(30), None);
    }

    #[test]
    fn preview_piano_key_updates_feedback() {
        let mut stage = Stage::default();
        stage.music.instrument = 0;
        stage.preview_piano_key(60, 100);
        assert_eq!(stage.label, "C4");
        assert_eq!(stage.last_hit, 100);
        assert_eq!(stage.music.lit_key, Some((60, 280)));
    }
}