//! Instrument submodule.
use super::*;

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

/// Keep strings inside the cover-safe band so low E / high e aren't cropped off-screen.
pub const STRING_Y: [f32; 6] = [0.28, 0.36, 0.44, 0.52, 0.60, 0.68];
// Калибр струн: 6-я толстая (E), 1-я тонкая (e)
const STRING_THICKNESS: [f32; 6] = [5.0, 4.0, 3.2, 2.4, 1.8, 1.4];
const GUITAR_PICK_X0: f32 = 0.28;
const GUITAR_PICK_X1: f32 = 0.98;


impl Stage {
    pub(crate) fn play_guitar(&mut self, now: u64, hands: &[TrackedHand], pinch_ratio: f32) {
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
        self.music.guitar_tips.clear();
        for (hand_idx, hand) in hands.iter().enumerate() {
            let palm = palm_of(hand);
            // Picking / strum lives on the fretboard side (right). Chord bank is left.
            if palm.0 < GUITAR_PICK_X0 - 0.02 {
                continue;
            }

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

            let sens = self.sensitivity.clamp(0.55, 1.8);
            // Strum only on a clear wrist sweep — don't steal soft finger picks.
            let strum_threshold_y = 0.048 / sens;
            let strum_spd_threshold = 0.78 / sens;

            if dy.abs() > strum_threshold_y
                && wrist_spd > strum_spd_threshold
                && now.saturating_sub(self.last_hit) > 130
            {
                let down = dy > 0.0;
                self.strum_arpeggiated(now, active_notes, down, palm_mute, wrist_spd);
                continue;
            }

            // Finger picks: all five tips, looser curl gate, cross OR near-swipe.
            for (finger_idx, &landmark) in TIPS.iter().enumerate() {
                let shape = hand.world.as_ref().unwrap_or(&hand.landmarks);
                let joints = match finger_idx {
                    0 => [1, 2, 3, 4],
                    1 => [5, 6, 7, 8],
                    2 => [9, 10, 11, 12],
                    3 => [13, 14, 15, 16],
                    _ => [17, 18, 19, 20],
                };
                // Only skip clearly clenched fingers (was 0.68 — killed half the picks).
                let curl = finger_curl(shape, joints);
                if finger_idx == 0 {
                    if curl > 0.92 {
                        continue;
                    }
                } else if curl > 0.88 {
                    continue;
                }

                let (x, y) = tip(hand, landmark);
                if !(GUITAR_PICK_X0..GUITAR_PICK_X1).contains(&x)
                    || !(STRING_Y[0] - 0.06..STRING_Y[5] + 0.06).contains(&y)
                {
                    continue;
                }
                self.music
                    .guitar_tips
                    .push((x, y, finger_idx as u8));

                let prev = sample.tips[finger_idx];
                let tip_speed = dist2((x, y), (prev[0], prev[1])) / dt;
                // Slow intentional plucks still count.
                if tip_speed < 0.022 / sens {
                    continue;
                }

                let cooled = self.music.pluck_at.iter().any(|(h, f, at)| {
                    *h == hand_idx as u8 && *f == finger_idx as u8 && now.saturating_sub(*at) < 28
                });
                if cooled {
                    continue;
                }

                let Some((string_idx, cross_frac)) =
                    pluck_string_hit(prev[1], y, tip_speed, sens)
                else {
                    continue;
                };
                if now.saturating_sub(self.music.string_hit_at[string_idx]) < 32 {
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
                        || now.saturating_sub(*at) < 220
                });
                self.music
                    .pluck_at
                    .push((hand_idx as u8, finger_idx as u8, hit_at));
                self.music.string_hit_at[string_idx] = hit_at;

                let mut vel = self.velocity((0.38 + tip_speed / 1.55).min(1.0));
                if palm_mute {
                    vel = (vel as f32 * 0.75).round() as u8;
                }

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
    pub(crate) fn update_guitar_bend_and_vibrato(
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
    pub(crate) fn configure_guitar_mode(&mut self, mode: u8) {
        self.music.guitar_mode = mode;
        self.silence();
    }

    pub(crate) fn set_guitar_tone(&mut self, tone: u8) {
        self.music.guitar_tone = tone;
        self.apply_program();
    }

    pub(crate) fn set_capo(&mut self, capo: u8) {
        self.music.capo = capo.min(7);
        self.silence();
    }

    pub(crate) fn guitar_audio_available(&self) -> bool {
        self.music.guitar_audio.is_some()
    }

    pub(crate) fn ensure_guitar_audio(&mut self) {
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

    pub(crate) fn preview_guitar_string(&mut self, string: usize, now: u64) {
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

    pub(crate) fn active_guitar_notes(&self) -> [u8; 6] {
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

    // --- ГИТАРА: оверлей поверх камеры ---

    pub(crate) fn paint_guitar(&self, image: &mut RgbImage) {
        let fretboard_x = 0.30;
        let fretboard_w = 0.66;
        let fretboard_y = STRING_Y[0] - 0.05;
        let fretboard_h = (STRING_Y[5] + 0.05) - fretboard_y;
        let string_names = ["E", "A", "D", "G", "B", "e"];

        // Warm wood rail — light so the camera still reads through.
        fill_rect(
            image,
            fretboard_x - 0.02,
            fretboard_y,
            fretboard_w + 0.05,
            fretboard_h,
            [42, 28, 18],
            0.28,
        );
        // Top / bottom binding.
        thick_hline(
            image,
            fretboard_x - 0.02,
            fretboard_x + fretboard_w + 0.03,
            fretboard_y,
            [232, 196, 140],
            0.55,
            2,
        );
        thick_hline(
            image,
            fretboard_x - 0.02,
            fretboard_x + fretboard_w + 0.03,
            fretboard_y + fretboard_h,
            [232, 196, 140],
            0.45,
            2,
        );
        // Nut
        fill_rect(
            image,
            fretboard_x - 0.012,
            fretboard_y,
            0.012,
            fretboard_h,
            [248, 250, 252],
            0.85,
        );

        // Chord bank — glass tiles over camera.
        fill_rect(image, 0.01, 0.06, 0.27, 0.88, [8, 12, 20], 0.30);
        let active = &CHORDS[self.music.chord.min(CHORDS.len() - 1)];
        draw_label(image, 0.025, 0.075, "CHORD", [148, 163, 184], 0.7, 1);
        draw_label(image, 0.025, 0.105, active.name, [255, 255, 255], 0.95, 2);
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
                    if lit { chord.color } else { [15, 23, 42] },
                    if lit { 0.72 } else { 0.42 },
                );
                if lit {
                    thick_hline(image, x, x + w, y, [255, 255, 255], 0.7, 2);
                    thick_hline(image, x, x + w, y + h, [255, 255, 255], 0.35, 1);
                }
                draw_label(
                    image,
                    x + 0.012,
                    y + h * 0.30,
                    chord.name,
                    if lit { [15, 23, 42] } else { [226, 232, 240] },
                    0.95,
                    if chord.name.len() > 4 { 1 } else { 2 },
                );
            }
        } else {
            draw_label(image, 0.04, 0.40, "SOLO", [94, 234, 212], 0.85, 2);
            draw_label(image, 0.04, 0.48, "FRET", [148, 163, 184], 0.65, 1);
        }

        // Frets + inlays.
        let num_frets = 12;
        let fret_step = fretboard_w / num_frets as f32;
        for fret in 1..=num_frets {
            let fx = fretboard_x + fret as f32 * fret_step;
            vline(
                image,
                fx,
                fretboard_y,
                fretboard_y + fretboard_h,
                [120, 90, 60],
                if fret == 12 { 0.55 } else { 0.28 },
            );
            let marker_x = fx - fret_step * 0.5;
            let mid_y = fretboard_y + fretboard_h * 0.5;
            if matches!(fret, 3 | 5 | 7 | 9) {
                let (cx, cy) = px(image, marker_x, mid_y);
                disc(image, cx, cy, 4, [241, 245, 249], 0.55);
            } else if fret == 12 {
                let (cx1, cy1) = px(image, marker_x, mid_y - 0.10);
                let (cx2, cy2) = px(image, marker_x, mid_y + 0.10);
                disc(image, cx1, cy1, 4, [241, 245, 249], 0.6);
                disc(image, cx2, cy2, 4, [241, 245, 249], 0.6);
            }
        }

        // Strings.
        for (index, &y) in STRING_Y.iter().enumerate() {
            let energy = self.music.string_energy[index].clamp(0.0, 1.0);
            let base_thick = STRING_THICKNESS[index];
            let core = if index < 3 {
                [196, 150, 90]
            } else {
                [226, 232, 240]
            };
            let hit = energy > 0.12;
            let thickness = (base_thick + if hit { energy * 2.2 } else { 0.0 })
                .round()
                .max(2.0) as i32;
            let x0 = fretboard_x - 0.01;
            let x1 = fretboard_x + fretboard_w + 0.02;
            // Shadow under the wire.
            thick_hline(image, x0, x1, y + 0.004, [0, 0, 0], 0.25, thickness + 1);
            thick_hline(
                image,
                x0,
                x1,
                y,
                if hit { [255, 220, 120] } else { core },
                if hit { 0.95 } else { 0.82 },
                thickness,
            );
            thick_hline(
                image,
                x0,
                x1,
                y - 0.003,
                [255, 255, 255],
                if hit { 0.7 } else { 0.35 },
                1,
            );
            draw_label(
                image,
                x1 + 0.01,
                y - 0.012,
                string_names[index],
                if hit { [255, 236, 170] } else { [203, 213, 225] },
                0.8,
                1,
            );
            if hit {
                let (cx, cy) = px(image, 0.64, y);
                disc(
                    image,
                    cx,
                    cy,
                    (4.0 + energy * 6.0) as i32,
                    [255, 236, 170],
                    0.55,
                );
            }

            if self.music.guitar_mode == 1 {
                if let Some(fret) = self.music.solo_fret[index] {
                    if fret > 0 && fret <= 12 {
                        let fx = fretboard_x + (fret as f32 - 0.5) * fret_step;
                        let (cx, cy) = px(image, fx, y);
                        disc(image, cx, cy, 6, [45, 212, 191], 0.85);
                        ring(image, cx, cy, 9.0, [255, 255, 255], 0.55);
                    }
                }
            }
        }

        // Live fingertips — see what the tracker is using.
        for &(x, y, finger) in &self.music.guitar_tips {
            let (cx, cy) = px(image, x, y);
            let color = match finger {
                0 => [251, 146, 60],
                1 => [56, 189, 248],
                2 => [52, 211, 153],
                3 => [167, 139, 250],
                _ => [244, 114, 182],
            };
            disc(image, cx, cy, 5, color, 1.0);
            ring(image, cx, cy, 8.0, [255, 255, 255], 0.55);
        }

        draw_label(image, 0.32, 0.035, "PICK ZONE", [226, 232, 240], 0.55, 1);
        if self.music.capo > 0 {
            draw_label(
                image,
                0.72,
                0.035,
                &format!("CAPO {}", self.music.capo),
                [250, 204, 21],
                0.85,
                1,
            );
        }
    }

    // Затухание струн (физический decay)
    pub(crate) fn decay_strings(&mut self, now: u64) {
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

}

pub(crate) fn fret_at(x: f32) -> Option<u8> {
    let fretboard_x = 0.28;
    let fretboard_w = 0.70;
    if !(fretboard_x..=fretboard_x + fretboard_w).contains(&x) {
        return None;
    }
    let step = fretboard_w / 12.0;
    let fret = ((x - fretboard_x) / step).floor() as u8 + 1;
    Some(fret.clamp(1, 12))
}

pub(crate) fn palm_mute_active(hands: &[TrackedHand]) -> bool {
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

pub(crate) fn pose_of(hand: &TrackedHand, pinch_ratio: f32) -> Pose {
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

pub(crate) fn chord_rect(index: usize) -> (f32, f32, f32, f32) {
    let col = if index < 8 { 0.0 } else { 1.0 };
    let row = if index < 8 { index } else { index - 8 };
    (0.018 + col * 0.128, 0.155 + row as f32 * 0.084, 0.118, 0.076)
}

fn chord_quality_label(name: &str) -> &'static str {
    if name.ends_with("m7") {
        "min7"
    } else if name.contains("add9") {
        "add9"
    } else if name.contains("sus4") {
        "sus4"
    } else if name.contains("sus2") {
        "sus2"
    } else if name.ends_with('7') {
        "dom7"
    } else if name.ends_with('m') {
        "minor"
    } else {
        "major"
    }
}

pub(crate) fn chord_at(x: f32, y: f32) -> Option<usize> {
    (0..CHORDS.len()).find(|index| {
        let (zx, zy, w, h) = chord_rect(*index);
        (zx..zx + w).contains(&x) && (zy..zy + h).contains(&y)
    })
}

pub(crate) fn nearest_string(y: f32, reach: f32) -> Option<usize> {
    STRING_Y
        .iter()
        .enumerate()
        .filter(|(_, string_y)| (y - *string_y).abs() <= reach)
        .min_by(|left, right| (y - *left.1).abs().total_cmp(&(y - *right.1).abs()))
        .map(|(index, _)| index)
}

pub(crate) fn crossed_string_subframe(prev_y: f32, y: f32) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32, f32)> = None;
    let travel = (prev_y - y).abs();
    // Low FPS can jump further between frames — still count the cross.
    if travel < 1e-5 || travel > 0.38 {
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

/// Classic string cross, or a fast near-swipe that grazes a string (low-FPS friendly).
fn pluck_string_hit(prev_y: f32, y: f32, tip_speed: f32, sens: f32) -> Option<(usize, f32)> {
    if let Some(hit) = crossed_string_subframe(prev_y, y) {
        return Some(hit);
    }
    let dy = y - prev_y;
    if dy.abs() < 0.005 {
        return None;
    }
    // Edge strings (low E / high e) get a slightly fatter target.
    let reach = (0.036 / sens).clamp(0.024, 0.055);
    let Some(index) = nearest_string(y, reach) else {
        return None;
    };
    let edge = index == 0 || index == 5;
    let sy = STRING_Y[index];
    let pad = if edge { 0.0 } else { 0.002 };
    let approaching = (prev_y - sy).abs() > (y - sy).abs() + pad;
    let fast = tip_speed > (if edge { 0.032 } else { 0.042 }) / sens;
    if approaching && fast {
        Some((index, 0.55))
    } else {
        None
    }
}

