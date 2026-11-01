//! Instrument submodule.
use super::*;

// --- БАРАБАНЫ ---

struct Drum {
    name: &'static str,
    note: u8,
    x: f32,
    y: f32,
    r: f32,
    color: [u8; 3],
}

/// Only pads with bundled samples (kick / snare / closed / open). Spaced for easy aiming.
const DRUMS: &[Drum] = &[
    Drum {
        name: "Хэт",
        note: 42,
        x: 0.16,
        y: 0.34,
        r: 0.070,
        color: [52, 199, 89],
    },
    Drum {
        name: "Опен",
        note: 46,
        x: 0.84,
        y: 0.34,
        r: 0.070,
        color: [48, 209, 88],
    },
    Drum {
        name: "Снейр",
        note: 38,
        x: 0.26,
        y: 0.64,
        r: 0.082,
        color: [110, 200, 140],
    },
    Drum {
        name: "Бочка",
        note: 36,
        x: 0.74,
        y: 0.74,
        r: 0.100,
        color: [34, 170, 80],
    },
];


impl Stage {
    pub(crate) fn play_drums(&mut self, now: u64, hands: &[TrackedHand]) {
        let prev_hover = self.music.drum_hover;
        self.music.drum_aim.clear();
        self.music.drum_hover = None;

        let sens = self.sensitivity.clamp(0.75, 1.5);
        let reach_mul = 1.18 * sens;

        for hand in hands {
            let palm = palm_of(hand);
            self.music.drum_aim.push(palm);

            let mut best: Option<(usize, f32)> = None;
            for (zone_idx, drum) in DRUMS.iter().enumerate() {
                let d = dist2(palm, (drum.x, drum.y));
                let need = drum.r * reach_mul;
                if d <= need && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((zone_idx, d));
                }
            }

            let Some((zone_idx, _)) = best else {
                continue;
            };
            self.music.drum_hover = Some(zone_idx);

            // Trigger on enter only — leave and come back to hit again.
            let entered = prev_hover != Some(zone_idx);
            if !entered {
                continue;
            }
            if now.saturating_sub(self.music.drum_at[zone_idx]) < 120 {
                continue;
            }

            let drum = &DRUMS[zone_idx];
            self.music.drum_at[zone_idx] = now;
            let vel = self.velocity(0.88);
            self.shot(9, drum.note, vel, now + 140);
            self.music.lit_drum = Some((zone_idx, now + 180));
            self.label = drum.name.into();
            self.last_hit = now;
        }
    }

    // =========================================================================
    // --- МЕЛОДИЯ: Ладонь / Кулак / Мастер ---
    // =========================================================================

    pub(crate) fn ensure_drum_audio(&mut self) {
        if self.music.drum_audio.is_some() || self.music.drum_audio_failed {
            return;
        }
        if cfg!(test) {
            return;
        }
        match DrumAudio::open() {
            Ok(audio) => self.music.drum_audio = Some(audio),
            Err(error) => {
                tracing::warn!(%error, "drum sample audio could not be opened");
                self.music.drum_audio_failed = true;
            }
        }
    }

    pub(crate) fn drum_audio_available(&self) -> bool {
        self.music.drum_audio.is_some()
    }

    pub(crate) fn drum_kit_for_note(note: u8) -> Option<u8> {
        match note {
            36 => Some(0), // kick / бочка
            38 => Some(1), // snare / снейр
            42 => Some(2), // closed hat
            46 => Some(3), // open hat
            _ => None,
        }
    }

    pub(crate) fn paint_drums(&self, image: &mut RgbImage) {
        draw_label(
            image,
            0.08,
            0.03,
            "Наведи ладонь на пэд",
            [255, 255, 255],
            0.92,
            1,
        );

        for (index, drum) in DRUMS.iter().enumerate() {
            let (cx, cy) = px(image, drum.x, drum.y);
            let radius = (drum.r * image.width() as f32).round() as i32;
            let lit = self.music.lit_drum.is_some_and(|(zone, _)| zone == index);
            let hover = self.music.drum_hover == Some(index);

            disc(image, cx, cy, radius + 10, [255, 255, 255], 0.14);
            disc(
                image,
                cx,
                cy,
                radius,
                if lit || hover {
                    drum.color
                } else {
                    [245, 245, 247]
                },
                if lit {
                    0.88
                } else if hover {
                    0.72
                } else {
                    0.42
                },
            );
            ring(
                image,
                cx,
                cy,
                radius as f32,
                if lit || hover {
                    [255, 255, 255]
                } else {
                    [52, 199, 89]
                },
                if lit || hover { 0.95 } else { 0.55 },
            );
            if lit {
                ring(image, cx, cy, radius as f32 + 16.0, [52, 199, 89], 0.5);
            } else if hover {
                ring(image, cx, cy, radius as f32 + 12.0, [52, 199, 89], 0.4);
            }

            draw_label(
                image,
                drum.x - 0.035,
                drum.y + drum.r + 0.016,
                drum.name,
                [255, 255, 255],
                0.95,
                1,
            );
        }

        let orb_r = (0.050 * image.width() as f32).round() as i32;
        for palm in &self.music.drum_aim {
            let (cx, cy) = px(image, palm.0, palm.1);
            disc(image, cx, cy, orb_r + 5, [52, 199, 89], 0.12);
            disc(image, cx, cy, orb_r, [255, 255, 255], 0.36);
            disc(image, cx, cy, (orb_r / 3).max(3), [52, 199, 89], 0.55);
        }
    }

}
