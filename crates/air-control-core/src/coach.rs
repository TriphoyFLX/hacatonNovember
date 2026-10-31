//! Live coaching: light checks, hand stillness, and mistake memory.

use image::RgbImage;

use crate::model::TrackedHand;

#[derive(Debug, Clone, Default)]
pub struct MistakeLog {
    entries: Vec<(String, String)>,
}

impl MistakeLog {
    pub fn note(&mut self, wanted: &str, seen: &str) {
        if wanted.is_empty() || seen.is_empty() || wanted == seen || seen == "—" {
            return;
        }
        self.entries.push((wanted.into(), seen.into()));
        if self.entries.len() > 10 {
            let drop = self.entries.len() - 10;
            self.entries.drain(0..drop);
        }
    }

    pub fn summary(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }
        let mut counts: Vec<(String, String, u32)> = Vec::new();
        for (wanted, seen) in &self.entries {
            if let Some(slot) = counts
                .iter_mut()
                .find(|(w, s, _)| w == wanted && s == seen)
            {
                slot.2 += 1;
            } else {
                counts.push((wanted.clone(), seen.clone(), 1));
            }
        }
        counts.sort_by(|a, b| b.2.cmp(&a.2));
        let (wanted, seen, n) = &counts[0];
        if *n < 3 {
            return String::new();
        }
        format!(
            "Часто хотели «{}», а видели «{}» ({} раз). Пройдите «Подстроить руку» или тренировку щипка.",
            pose_ru(wanted),
            pose_ru(seen),
            n
        )
    }
}

fn pose_ru(id: &str) -> &str {
    match id {
        "pinch" => "щипок",
        "index_point" => "указательный",
        "open_palm" => "ладонь",
        "closed_fist" => "кулак",
        "v_sign" => "V",
        "thumb_up" => "большой вверх",
        "thumb_down" => "большой вниз",
        other => other,
    }
}

#[derive(Debug, Default, Clone)]
pub struct Stillness {
    samples: Vec<(f32, f32)>,
}

impl Stillness {
    pub fn push(&mut self, wrist: (f32, f32)) {
        self.samples.push(wrist);
        if self.samples.len() > 8 {
            self.samples.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// True when the wrist has been calm enough for a pose command.
    pub fn calm(&self) -> bool {
        if self.samples.len() < 4 {
            return true;
        }
        let mut travel = 0.0;
        for pair in self.samples.windows(2) {
            let dx = pair[1].0 - pair[0].0;
            let dy = pair[1].1 - pair[0].1;
            travel += (dx * dx + dy * dy).sqrt();
        }
        travel < 0.07
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneHint {
    Ok,
    Dark,
    Bright,
    Flat,
    Soft,
}

pub fn scene_hint(image: &RgbImage, hand: Option<&TrackedHand>) -> SceneHint {
    let (sum, count) = if let Some(hand) = hand {
        sample_hand_patch(image, hand)
    } else {
        sample_center(image)
    };
    if count == 0 {
        return SceneHint::Ok;
    }
    let mean = sum / count as f32;
    if mean < 48.0 {
        SceneHint::Dark
    } else if mean > 210.0 {
        SceneHint::Bright
    } else if hand_background_flat(image, hand) {
        SceneHint::Flat
    } else if laplacian_var(image, hand) < 28.0 {
        SceneHint::Soft
    } else {
        SceneHint::Ok
    }
}

pub fn scene_hint_text(hint: SceneHint) -> &'static str {
    match hint {
        SceneHint::Ok => "",
        SceneHint::Dark => "Темно: включите свет или повернитесь к окну.",
        SceneHint::Bright => "Слишком яркий свет сзади. Встаньте лицом к свету.",
        SceneHint::Flat => "Рука сливается с фоном. Смените фон или одежду.",
        SceneHint::Soft => "Камера пересфокусировалась. Закройте другие приложения камеры или зафиксируйте фокус.",
    }
}

/// Variance of a cheap Laplacian — low means the patch is soft / hunting focus.
fn laplacian_var(image: &RgbImage, hand: Option<&TrackedHand>) -> f32 {
    let (cx, cy, span) = if let Some(hand) = hand {
        let w = image.width().saturating_sub(1) as f32;
        let h = image.height().saturating_sub(1) as f32;
        let x = ((hand.landmarks[0][0] + hand.landmarks[9][0]) * 0.5).clamp(0.0, 1.0) * w;
        let y = ((hand.landmarks[0][1] + hand.landmarks[9][1]) * 0.5).clamp(0.0, 1.0) * h;
        (x as i32, y as i32, 28i32)
    } else {
        (
            (image.width() / 2) as i32,
            (image.height() / 2) as i32,
            36i32,
        )
    };
    let mut values = Vec::with_capacity(256);
    for y in (cy - span)..(cy + span) {
        for x in (cx - span)..(cx + span) {
            if (x + y) % 3 != 0 {
                continue;
            }
            let c = luma_at(image, x, y);
            let l = luma_at(image, x - 1, y);
            let r = luma_at(image, x + 1, y);
            let u = luma_at(image, x, y - 1);
            let d = luma_at(image, x, y + 1);
            values.push(4.0 * c - l - r - u - d);
        }
    }
    if values.len() < 8 {
        return 100.0;
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    values.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / values.len() as f32
}

fn sample_center(image: &RgbImage) -> (f32, u32) {
    let w = image.width().max(1);
    let h = image.height().max(1);
    let mut sum = 0.0;
    let mut count = 0u32;
    for y in (h / 4)..(h * 3 / 4) {
        for x in (w / 4)..(w * 3 / 4) {
            if (x + y) % 5 != 0 {
                continue;
            }
            let p = image.get_pixel(x, y).0;
            sum += (p[0] as f32 + p[1] as f32 + p[2] as f32) / 3.0;
            count += 1;
        }
    }
    (sum, count)
}

fn sample_hand_patch(image: &RgbImage, hand: &TrackedHand) -> (f32, u32) {
    let w = image.width().saturating_sub(1) as f32;
    let h = image.height().saturating_sub(1) as f32;
    let mut sum = 0.0;
    let mut count = 0u32;
    for index in [0usize, 5, 9, 13, 17, 8] {
        let x = (hand.landmarks[index][0].clamp(0.0, 1.0) * w).round() as i32;
        let y = (hand.landmarks[index][1].clamp(0.0, 1.0) * h).round() as i32;
        for dy in -3..=3 {
            for dx in -3..=3 {
                let px = x + dx;
                let py = y + dy;
                if px < 0 || py < 0 || px as u32 >= image.width() || py as u32 >= image.height() {
                    continue;
                }
                let p = image.get_pixel(px as u32, py as u32).0;
                sum += (p[0] as f32 + p[1] as f32 + p[2] as f32) / 3.0;
                count += 1;
            }
        }
    }
    (sum, count)
}

fn hand_background_flat(image: &RgbImage, hand: Option<&TrackedHand>) -> bool {
    let Some(hand) = hand else {
        return false;
    };
    let w = image.width().saturating_sub(1) as f32;
    let h = image.height().saturating_sub(1) as f32;
    let palm = (
        ((hand.landmarks[0][0] + hand.landmarks[9][0]) * 0.5).clamp(0.0, 1.0) * w,
        ((hand.landmarks[0][1] + hand.landmarks[9][1]) * 0.5).clamp(0.0, 1.0) * h,
    );
    let bg = (
        (palm.0 + 40.0).min(w),
        (palm.1 - 40.0).max(0.0),
    );
    let hand_luma = luma_at(image, palm.0 as i32, palm.1 as i32);
    let bg_luma = luma_at(image, bg.0 as i32, bg.1 as i32);
    (hand_luma - bg_luma).abs() < 18.0
}

fn luma_at(image: &RgbImage, x: i32, y: i32) -> f32 {
    if x < 0 || y < 0 || x as u32 >= image.width() || y as u32 >= image.height() {
        return 128.0;
    }
    let p = image.get_pixel(x as u32, y as u32).0;
    (p[0] as f32 + p[1] as f32 + p[2] as f32) / 3.0
}
