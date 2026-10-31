//! Live coaching: light checks, hand stillness, tremor, and mistake memory.

use std::collections::VecDeque;

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

    /// Residual wrist jitter in normalized units (std of successive steps).
    pub fn residual_std(&self) -> f32 {
        if self.samples.len() < 4 {
            return 0.0;
        }
        let mut steps = Vec::with_capacity(self.samples.len());
        for pair in self.samples.windows(2) {
            let dx = pair[1].0 - pair[0].0;
            let dy = pair[1].1 - pair[0].1;
            steps.push((dx * dx + dy * dy).sqrt());
        }
        let mean = steps.iter().sum::<f32>() / steps.len() as f32;
        let var = steps
            .iter()
            .map(|v| {
                let d = *v - mean;
                d * d
            })
            .sum::<f32>()
            / steps.len() as f32;
        var.sqrt()
    }
}

/// Running wrist tremor over ~10 minutes (1 Hz samples).
#[derive(Debug, Default, Clone)]
pub struct TremorMonitor {
    samples: VecDeque<(u64, f32, f32)>,
    last_push_ms: u64,
    tired_until: u64,
}

impl TremorMonitor {
    pub fn push(&mut self, now_ms: u64, wrist: (f32, f32)) {
        if now_ms.saturating_sub(self.last_push_ms) < 900 {
            return;
        }
        self.last_push_ms = now_ms;
        self.samples.push_back((now_ms, wrist.0, wrist.1));
        let cutoff = now_ms.saturating_sub(600_000);
        while self.samples.front().is_some_and(|(t, _, _)| *t < cutoff) {
            self.samples.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
        self.last_push_ms = 0;
    }

    pub fn sigma(&self) -> f32 {
        if self.samples.len() < 40 {
            return 0.0;
        }
        let mut steps = Vec::with_capacity(self.samples.len());
        for pair in self.samples.iter().collect::<Vec<_>>().windows(2) {
            let dx = pair[1].1 - pair[0].1;
            let dy = pair[1].2 - pair[0].2;
            steps.push((dx * dx + dy * dy).sqrt());
        }
        let mean = steps.iter().sum::<f32>() / steps.len() as f32;
        let var = steps
            .iter()
            .map(|v| {
                let d = *v - mean;
                d * d
            })
            .sum::<f32>()
            / steps.len() as f32;
        var.sqrt()
    }

    /// Returns a coach line when tremor is high; also stamps a rest window.
    pub fn check(&mut self, now_ms: u64) -> Option<&'static str> {
        if now_ms < self.tired_until {
            return Some("Дай руке отдохнуть — дрожь выросла.");
        }
        let sigma = self.sigma();
        // Calm residual is ~0.002–0.008; fatigue pushes past ~0.018 over a long window.
        if sigma > 0.018 && self.samples.len() >= 80 {
            self.tired_until = now_ms.saturating_add(20_000);
            return Some("Дай руке отдохнуть — дрожь выросла.");
        }
        None
    }

    pub fn resting(&self, now_ms: u64) -> bool {
        now_ms < self.tired_until
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneHint {
    Ok,
    Dark,
    Bright,
    Flat,
    Soft,
    Flicker,
}

/// Frame-quality gates from the vision process (preferred over RGB heuristics).
pub fn quality_gate(sharpness: f32, brightness: f32, flicker: f32) -> (f32, SceneHint) {
    let mut gate = 1.0_f32;
    let mut hint = SceneHint::Ok;
    if brightness < 40.0 {
        gate *= 0.45;
        hint = SceneHint::Dark;
    } else if brightness < 55.0 {
        gate *= 0.72;
        if hint == SceneHint::Ok {
            hint = SceneHint::Dark;
        }
    } else if brightness > 220.0 {
        gate *= 0.55;
        hint = SceneHint::Bright;
    } else if brightness > 200.0 {
        gate *= 0.8;
        if hint == SceneHint::Ok {
            hint = SceneHint::Bright;
        }
    }
    if sharpness < 35.0 {
        gate *= 0.4;
        hint = SceneHint::Soft;
    } else if sharpness < 55.0 {
        gate *= 0.7;
        if hint == SceneHint::Ok {
            hint = SceneHint::Soft;
        }
    }
    if flicker > 28.0 {
        gate *= 0.55;
        hint = SceneHint::Flicker;
    } else if flicker > 18.0 {
        gate *= 0.82;
        if hint == SceneHint::Ok {
            hint = SceneHint::Flicker;
        }
    }
    (gate.clamp(0.15, 1.0), hint)
}

pub fn tracking_health(sharpness: f32, brightness: f32, flicker: f32, fps: f32) -> String {
    let mut parts = Vec::new();
    if brightness < 40.0 {
        parts.push("камера тёмная");
    } else if brightness > 220.0 {
        parts.push("пересвет");
    }
    if sharpness < 50.0 {
        parts.push("кадр смазан");
    }
    if flicker > 22.0 {
        parts.push("мерцание");
    }
    if fps > 0.0 && fps < 20.0 {
        parts.push("FPS просел");
    }
    if parts.is_empty() {
        "трекинг в норме".into()
    } else {
        format!("плохой трекинг: {}", parts.join(", "))
    }
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
        SceneHint::Dark => "Сцена слишком тёмная: включите свет или повернитесь к окну.",
        SceneHint::Bright => "Слишком яркий свет сзади. Встаньте лицом к свету.",
        SceneHint::Flat => "Рука сливается с фоном. Смените фон или одежду.",
        SceneHint::Soft => "Смаз кадра: зафиксируйте фокус или увеличьте выдержку.",
        SceneHint::Flicker => "Мерцание света. Уберите ШИМ-лампы или смените частоту.",
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
    let bg = ((palm.0 + 40.0).min(w), (palm.1 - 40.0).max(0.0));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_gate_dark_and_soft() {
        let (gate, hint) = quality_gate(20.0, 30.0, 2.0);
        assert!(gate < 0.5);
        assert_eq!(hint, SceneHint::Soft);
        assert!(tracking_health(20.0, 30.0, 2.0, 15.0).contains("трекинг"));
    }
}
