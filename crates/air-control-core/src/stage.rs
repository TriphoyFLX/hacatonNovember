//! Creative stage: air instruments, magic hands, and motion drawing.
//!
//! System cursor commands stay off while a scene is selected. Sound goes
//! through local MIDI. Effects are drawn on the camera frame.

use image::RgbImage;

use crate::filter::{ema, ema_alpha};
use crate::gestures::{finger_curl, hand_openness, tip_point, BinaryLatch};
use crate::model::TrackedHand;

mod music;

const MAX_PARTICLES: usize = 350;
const MAX_INK: usize = 900;
const TIPS: [usize; 5] = [4, 8, 12, 16, 20];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pose {
    Pinch,
    Fist,
    Palm,
    Point,
    Other,
}

#[derive(Clone, Copy)]
struct Sample {
    track_id: u64,
    at: u64,
    tips: [[f32; 2]; 5],
    /// World meters when available, else image coords — for scale-free speeds.
    tip_metric: [[f32; 3]; 5],
    tip_speed: [f32; 5],
    palm: [f32; 2],
    wrist_speed: f32,
    pose: Pose,
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    size: f32,
    color: [u8; 3],
    gravity: f32,
    drag: f32,
}

struct Ink {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    born: u64,
    width: f32,
    color: [u8; 3],
    soft: bool,
}

struct Bolt {
    pts: Vec<(f32, f32)>,
    color: [u8; 3],
}

pub struct StageView {
    pub mode: i32,
    pub instrument: i32,
    pub element: i32,
    pub brush: i32,
    pub color: i32,
    pub preset: i32,
    pub muted: bool,
    pub volume: f32,
    pub sensitivity: f32,
    pub guitar_chord: i32,
    pub guitar_mode: i32,
    pub guitar_capo: i32,
    pub thickness: f32,
    pub fade: f32,
    pub label: String,
    pub hint: String,
}

pub struct Stage {
    music: music::Music,
    mode: u8,
    element: u8,
    brush: u8,
    color: u8,
    sensitivity: f32,
    thickness: f32,
    fade: f32,
    label: String,
    prev: Vec<Sample>,
    particles: Vec<Particle>,
    ink: Vec<Ink>,
    bolts: Vec<Bolt>,
    flick_at: [u64; 2],
    last_hit: u64,
    fist_latch: [BinaryLatch; 2],
    palm_latch: [BinaryLatch; 2],
    rng: u32,
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            music: music::Music::default(),
            mode: 0,
            element: 0,
            brush: 0,
            color: 3,
            sensitivity: 1.0,
            thickness: 5.5,
            fade: 0.045,
            label: "—".into(),
            prev: Vec::new(),
            particles: Vec::new(),
            ink: Vec::new(),
            bolts: Vec::new(),
            flick_at: [0; 2],
            last_hit: 0,
            fist_latch: [BinaryLatch::default(); 2],
            palm_latch: [BinaryLatch::default(); 2],
            rng: 0xA1C0_1234,
        }
    }
}

impl Stage {
    pub fn is_live(&self) -> bool {
        self.mode != 0
    }

    pub fn view(&self) -> StageView {
        StageView {
            mode: i32::from(self.mode),
            instrument: i32::from(self.music.instrument),
            element: i32::from(self.element),
            brush: i32::from(self.brush),
            color: i32::from(self.color),
            preset: i32::from(if self.music.instrument == 2 {
                self.music.guitar_tone
            } else {
                self.music.piano_preset
            }),
            muted: self.music.muted,
            volume: self.music.volume,
            sensitivity: self.sensitivity,
            guitar_chord: self.music.chord as i32,
            guitar_mode: i32::from(self.music.guitar_mode),
            guitar_capo: i32::from(self.music.capo),
            thickness: self.thickness,
            fade: self.fade,
            label: self.label.clone(),
            hint: self.hint(),
        }
    }

    pub fn label(&self) -> String {
        self.label.clone()
    }

    pub fn detail(&self) -> String {
        if self.mode == 1
            && self.music.midi_failed
            && (self.music.instrument != 2 || !self.guitar_audio_available())
        {
            return if self.music.instrument == 2 {
                "Нет аудиовыхода".into()
            } else {
                "Нет MIDI-выхода".into()
            };
        }
        if self.mode == 1 && self.music.muted {
            return "Без звука".into();
        }
        match self.mode {
            1 => match self.music.instrument {
                0 => "Пианино",
                1 => "Барабаны",
                _ => "Гитара",
            },
            2 => match self.element {
                0 => "Огонь",
                1 => "Вода",
                2 => "Молния",
                _ => "Энергия",
            },
            3 => "Рисунок",
            _ => "—",
        }
        .into()
    }

    pub fn set_mode(&mut self, mode: i32) {
        let mode = mode.clamp(0, 3) as u8;
        if mode == self.mode {
            return;
        }
        self.silence();
        self.mode = mode;
        self.particles.clear();
        self.bolts.clear();
        self.clear_music_holds();
        self.prev.clear();
        if mode == 1 {
            self.ensure_midi();
            if self.music.instrument == 2 {
                self.ensure_guitar_audio();
            }
            self.apply_program();
            self.push_volume();
        }
        self.label = match mode {
            0 => "—".into(),
            1 => "Можно играть".into(),
            2 => "Можно колдовать".into(),
            _ => "Можно рисовать".into(),
        };
    }

    pub fn set_instrument(&mut self, index: i32) {
        self.music.instrument = index.clamp(0, 2) as u8;
        self.silence();
        self.clear_music_holds();
        if self.mode == 1 && self.music.instrument == 2 {
            self.ensure_guitar_audio();
        }
        self.apply_program();
        self.label = match self.music.instrument {
            0 => "Пианино".into(),
            1 => "Барабаны".into(),
            _ => "Гитара".into(),
        };
    }

    pub fn set_element(&mut self, index: i32) {
        self.element = index.clamp(0, 3) as u8;
        self.particles.clear();
        self.bolts.clear();
        self.label = self.detail();
    }

    pub fn set_brush(&mut self, index: i32) {
        self.brush = index.clamp(0, 2) as u8;
    }

    pub fn set_color(&mut self, index: i32) {
        self.color = index.clamp(0, 3) as u8;
    }

    pub fn set_preset(&mut self, index: i32) {
        if self.music.instrument == 2 {
            self.set_guitar_tone(index.clamp(0, 4) as u8);
        } else {
            self.music.piano_preset = index.clamp(0, 2) as u8;
        }
        self.apply_program();
    }

    pub fn set_guitar_chord(&mut self, index: i32) {
        self.music.chord = index.clamp(0, music::CHORDS.len() as i32 - 1) as usize;
        self.label = format!("Аккорд {}", music::CHORDS[self.music.chord].name);
    }

    pub fn set_guitar_mode(&mut self, mode: i32) {
        self.configure_guitar_mode(mode.clamp(0, 1) as u8);
    }

    pub fn set_guitar_capo(&mut self, capo: i32) {
        self.set_capo(capo.clamp(0, 7) as u8);
    }

    pub fn pluck_guitar_string(&mut self, string: i32, now: u64) {
        self.preview_guitar_string(string.clamp(0, 5) as usize, now);
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.music.muted = muted;
        if muted {
            self.silence();
        }
        self.label = if muted {
            "Звук выключен".into()
        } else {
            "Звук включён".into()
        };
    }

    pub fn set_volume(&mut self, value: f32) {
        self.music.volume = value.clamp(0.0, 1.0);
        self.push_volume();
    }

    pub fn set_sensitivity(&mut self, value: f32) {
        self.sensitivity = value.clamp(0.4, 2.0);
    }

    pub fn set_thickness(&mut self, value: f32) {
        self.thickness = value.clamp(2.0, 14.0);
    }

    pub fn set_fade(&mut self, value: f32) {
        self.fade = value.clamp(0.01, 0.12);
    }

    pub fn clear_art(&mut self) {
        self.ink.clear();
        self.particles.clear();
        self.label = "Рисунок стёрт".into();
    }

    pub fn release_sound(&mut self) {
        self.silence();
        self.clear_music_holds();
        self.prev.clear();
    }

    pub fn step(&mut self, now: u64, hands: &[TrackedHand], pinch_ratio: f32) {
        if self.mode == 0 {
            return;
        }
        self.flush_offs(now);
        self.bolts.clear();
        self.decay_strings(now);
        self.tick_particles();
        self.expire_ink(now);

        if hands.is_empty() {
            if !self.prev.is_empty() {
                self.silence();
                self.fist_latch = [BinaryLatch::default(); 2];
                self.palm_latch = [BinaryLatch::default(); 2];
            }
            self.prev.clear();
            if now.saturating_sub(self.last_hit) > 400 {
                self.label = "Жду руку".into();
            }
            return;
        }

        if self.switched_instrument(now, hands) {
            self.prev = self.sample_hands(now, hands, pinch_ratio);
            return;
        }

        match self.mode {
            1 => match self.music.instrument {
                0 => self.play_piano(now, hands),
                1 => self.play_drums(now, hands),
                _ => self.play_guitar(now, hands, pinch_ratio),
            },
            2 => self.play_magic(now, hands, pinch_ratio),
            _ => self.play_art(now, hands, pinch_ratio),
        }

        self.prev = self.sample_hands(now, hands, pinch_ratio);
    }

    pub fn paint_under(&mut self, image: &mut RgbImage) {
        match self.mode {
            1 => self.paint_music(image),
            2 => {
                dim(image, 0.28);
                vignette(image, 0.22);
            }
            3 => {
                dim(image, 0.26);
                vignette(image, 0.16);
                self.paint_ink(image);
            }
            _ => {}
        }
    }

    pub fn paint_over(&mut self, image: &mut RgbImage) {
        if self.mode == 0 {
            return;
        }
        for bolt in &self.bolts {
            paint_bolt(image, bolt);
        }
        for particle in &self.particles {
            let alpha = (particle.life / particle.max_life.max(0.05)).clamp(0.0, 1.0);
            let (px, py) = px(image, particle.x, particle.y);
            disc(
                image,
                px,
                py,
                particle.size.max(1.0).round() as i32,
                particle.color,
                alpha,
            );
        }
        if self.mode == 1 {
            self.paint_hits(image);
        }
    }

    fn hint(&self) -> String {
        if self.mode == 1
            && self.music.midi_failed
            && (self.music.instrument != 2 || !self.guitar_audio_available())
        {
            return "Аудиовыход не открылся. Жесты работают, звука нет.".into();
        }
        match self.mode {
            1 => match self.music.instrument {
                0 => "Опустите палец на нижний ряд клавиш. Две руки вверх меняют инструмент.".into(),
                1 => "Ударьте ладонью или пальцем по кругу.".into(),
                _ => "Аккорд выберите слева рукой или кнопкой. Справа щипайте струны, взмахните для боя; щипок на грифе включает бенд.".into(),
            },
            2 => "Кулак — заряд. Щипок — струя. Раскройте кулак — вспышка. Взмах указательным — бросок.".into(),
            3 => "Открытая ладонь или палец рисуют. Кулак переносит руку без следа.".into(),
            _ => "Один указательный палец ведёт мышь. Щипок на месте — клик. Щипок и сдвиг руки — перетаскивание.".into(),
        }
    }

    fn play_magic(&mut self, now: u64, hands: &[TrackedHand], pinch_ratio: f32) {
        let poses: Vec<Pose> = hands
            .iter()
            .enumerate()
            .map(|(index, hand)| self.pose_latched(index, hand, pinch_ratio))
            .collect();
        let two_fists = poses.len() >= 2 && poses[0] == Pose::Fist && poses[1] == Pose::Fist;
        if two_fists {
            let a = palm_of(&hands[0]);
            let b = palm_of(&hands[1]);
            self.bolts.push(Bolt {
                pts: vec![a, b],
                color: element_color(self.element),
            });
            self.spawn_along(a, b, element_color(self.element));
            self.label = "Связь двух рук".into();
            self.last_hit = now;
        }
        for (index, hand) in hands.iter().enumerate() {
            let pose = poses[index];
            let prev_sample = self.prev_for(hand);
            let prev_pose = prev_sample.map(|item| item.pose).unwrap_or(Pose::Other);
            let palm = palm_of(hand);
            let tip = tip(hand, 8);
            let color = element_color(self.element);
            match (self.element, pose) {
                (_, Pose::Fist) if !two_fists => {
                    self.spawn_charge(palm, color);
                    self.label = match self.element {
                        0 => "Заряд огня",
                        1 => "Водоворот",
                        2 => "Заряд молнии",
                        _ => "Центр",
                    }
                    .into();
                    self.last_hit = now;
                }
                (_, Pose::Pinch) => {
                    let wrist = (hand.landmarks[0][0], hand.landmarks[0][1]);
                    self.spawn_stream(wrist, tip, color);
                    if self.element == 2 {
                        let end = extend(wrist, tip, 0.28);
                        self.bolts.push(Bolt {
                            pts: jagged(wrist, end, &mut self.rng),
                            color,
                        });
                    }
                    self.label = match self.element {
                        0 => "Струя",
                        1 => "Струи",
                        2 => "Луч",
                        _ => "Поток",
                    }
                    .into();
                    self.last_hit = now;
                }
                (_, Pose::Palm) => {
                    if prev_pose == Pose::Fist {
                        self.spawn_burst(palm, color, if self.element == 3 { 22 } else { 14 });
                        self.label = if self.element == 3 {
                            "Вспышка"
                        } else {
                            "Кольцо"
                        }
                        .into();
                    } else if self.frame_gate(index) {
                        self.spawn_ring(palm, color);
                    }
                    self.last_hit = now;
                }
                (_, Pose::Point) => {
                    if let Some(sample) = prev_sample {
                        let dt = frame_dt(sample.at, now);
                        let prev = sample.tips[1];
                        let speed = sample.tip_speed[1];
                        if speed > 1.6 / self.sensitivity.max(0.4)
                            && now.saturating_sub(self.flick_at[index.min(1)]) > 280
                        {
                            self.flick_at[index.min(1)] = now;
                            let vx = (tip.0 - prev[0]) / dt;
                            let vy = (tip.1 - prev[1]) / dt;
                            self.spawn(Particle {
                                x: tip.0,
                                y: tip.1,
                                vx,
                                vy,
                                life: 0.65,
                                max_life: 0.65,
                                size: 7.0,
                                color,
                                gravity: if self.element == 0 { 0.15 } else { 0.0 },
                                drag: 0.985,
                            });
                            self.label = if self.element == 0 {
                                "Шар"
                            } else {
                                "Бросок"
                            }
                            .into();
                            self.last_hit = now;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn play_art(&mut self, now: u64, hands: &[TrackedHand], pinch_ratio: f32) {
        for (index, hand) in hands.iter().enumerate() {
            let pose = self.pose_latched(index, hand, pinch_ratio);
            let drawing = pose != Pose::Fist
                && (openness(hand) > 0.12
                    || matches!(pose, Pose::Point | Pose::Palm | Pose::Other));
            let sample = self.prev_for(hand);
            let fingers: &[usize] = if self.brush == 2 { &TIPS } else { &TIPS[1..] };
            for landmark in fingers {
                let slot = tip_index(*landmark);
                let (x, y) = tip(hand, *landmark);
                let Some(slot) = slot else { continue };
                let Some(sample) = sample else { continue };
                let prev = sample.tips[slot];
                if !drawing {
                    continue;
                }
                let speed = sample.tip_speed[slot];
                if dist2((x, y), (prev[0], prev[1])) < 0.0012 {
                    continue;
                }
                let color = stroke_color(self.color, x, speed, now, index);
                let width = self.thickness
                    * (0.7 + (speed / 2.2).min(1.8))
                    * if *landmark == 8 || *landmark == 12 {
                        1.15
                    } else {
                        0.75
                    };
                self.push_ink(Ink {
                    x0: prev[0],
                    y0: prev[1],
                    x1: x,
                    y1: y,
                    born: now,
                    width,
                    color,
                    soft: self.brush == 1,
                });
                if self.brush == 2 && speed > 0.8 {
                    for _ in 0..3 {
                        let jx = self.rand();
                        let jy = self.rand();
                        let life = 0.35 + self.rand() * 0.25;
                        let size = 2.0 + self.rand() * 2.0;
                        self.spawn(Particle {
                            x,
                            y,
                            vx: (jx - 0.5) * speed * 0.35,
                            vy: (jy - 0.5) * speed * 0.35,
                            life,
                            max_life: 0.6,
                            size,
                            color,
                            gravity: 0.05,
                            drag: 0.96,
                        });
                    }
                }
                self.label = "Рисунок".into();
                self.last_hit = now;
            }
            if drawing && self.brush == 1 {
                if let Some(sample) = sample {
                    let palm = palm_of(hand);
                    let speed = sample.wrist_speed;
                    if speed > 0.55 {
                        let color = stroke_color(self.color, palm.0, speed, now, index);
                        self.push_ink(Ink {
                            x0: palm.0,
                            y0: palm.1,
                            x1: palm.0,
                            y1: palm.1,
                            born: now,
                            width: 18.0 + openness(hand) * 28.0,
                            color,
                            soft: true,
                        });
                    }
                }
            }
        }
    }

    fn spawn(&mut self, particle: Particle) {
        if self.particles.len() >= MAX_PARTICLES {
            let extra = self.particles.len() + 1 - MAX_PARTICLES;
            self.particles.drain(0..extra);
        }
        self.particles.push(particle);
    }

    fn spawn_charge(&mut self, palm: (f32, f32), color: [u8; 3]) {
        for _ in 0..3 {
            let angle = self.rand() * std::f32::consts::TAU;
            let radius = 0.05 + self.rand() * 0.08;
            let size = 2.5 + self.rand() * 2.0;
            let tint = self.rand();
            let ox = angle.cos() * radius;
            let oy = angle.sin() * radius;
            self.spawn(Particle {
                x: palm.0 + ox,
                y: palm.1 + oy,
                vx: -ox * 3.0,
                vy: -oy * 3.0,
                life: 0.45,
                max_life: 0.45,
                size,
                color: vary(color, tint),
                gravity: 0.0,
                drag: 0.92,
            });
        }
    }

    fn spawn_stream(&mut self, from: (f32, f32), to: (f32, f32), color: [u8; 3]) {
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let gravity = if self.element == 1 { 0.35 } else { -0.05 };
        for _ in 0..4 {
            let jx = self.rand();
            let jy = self.rand();
            let life = 0.35 + self.rand() * 0.2;
            let size = 2.0 + self.rand() * 2.5;
            let tint = self.rand();
            self.spawn(Particle {
                x: to.0,
                y: to.1,
                vx: dx * 4.0 + (jx - 0.5) * 0.15,
                vy: dy * 4.0 + (jy - 0.5) * 0.15,
                life,
                max_life: 0.55,
                size,
                color: vary(color, tint),
                gravity,
                drag: 0.98,
            });
        }
    }

    fn spawn_ring(&mut self, palm: (f32, f32), color: [u8; 3]) {
        let count = 8;
        for index in 0..count {
            let angle = index as f32 / count as f32 * std::f32::consts::TAU;
            self.spawn(Particle {
                x: palm.0,
                y: palm.1,
                vx: angle.cos() * 0.35,
                vy: angle.sin() * 0.35,
                life: 0.5,
                max_life: 0.5,
                size: 3.0,
                color,
                gravity: 0.0,
                drag: 0.97,
            });
        }
    }

    fn spawn_burst(&mut self, palm: (f32, f32), color: [u8; 3], count: usize) {
        for index in 0..count {
            let angle = index as f32 / count as f32 * std::f32::consts::TAU + self.rand();
            let speed = 0.4 + self.rand() * 0.7;
            let life = 0.55 + self.rand() * 0.25;
            let size = 2.0 + self.rand() * 3.0;
            let tint = self.rand();
            self.spawn(Particle {
                x: palm.0,
                y: palm.1,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                life,
                max_life: 0.8,
                size,
                color: vary(color, tint),
                gravity: 0.02,
                drag: 0.96,
            });
        }
    }

    fn spawn_along(&mut self, a: (f32, f32), b: (f32, f32), color: [u8; 3]) {
        for step in 0..5 {
            let t = step as f32 / 4.0;
            let jx = self.rand();
            let jy = self.rand();
            self.spawn(Particle {
                x: a.0 + (b.0 - a.0) * t,
                y: a.1 + (b.1 - a.1) * t,
                vx: (jx - 0.5) * 0.1,
                vy: (jy - 0.5) * 0.1,
                life: 0.25,
                max_life: 0.25,
                size: 2.5,
                color,
                gravity: 0.0,
                drag: 0.9,
            });
        }
    }

    fn frame_gate(&mut self, hand: usize) -> bool {
        self.rng = self.rng.wrapping_add(hand as u32 + 1);
        self.rand() > 0.45
    }

    fn tick_particles(&mut self) {
        for particle in &mut self.particles {
            particle.x += particle.vx * (1.0 / 30.0);
            particle.y += particle.vy * (1.0 / 30.0);
            particle.vy += particle.gravity * (1.0 / 30.0);
            particle.vx *= particle.drag;
            particle.vy *= particle.drag;
            particle.life -= 1.0 / 30.0;
        }
        self.particles.retain(|particle| particle.life > 0.0);
    }

    fn expire_ink(&mut self, now: u64) {
        let life = ink_life_ms(self.fade);
        self.ink.retain(|item| now.saturating_sub(item.born) < life);
    }

    fn push_ink(&mut self, ink: Ink) {
        if self.ink.len() >= MAX_INK {
            let extra = self.ink.len() + 1 - MAX_INK;
            self.ink.drain(0..extra);
        }
        self.ink.push(ink);
    }

    fn paint_ink(&self, image: &mut RgbImage) {
        let now_bias = self.ink.last().map(|item| item.born).unwrap_or(0);
        let life = ink_life_ms(self.fade).max(1);
        for item in &self.ink {
            let age = now_bias.saturating_sub(item.born) as f32 / life as f32;
            let alpha = (1.0 - age).clamp(0.0, 1.0);
            if alpha < 0.04 {
                continue;
            }
            let (x0, y0) = px(image, item.x0, item.y0);
            let (x1, y1) = px(image, item.x1, item.y1);
            let radius = (item.width * 0.5).round().clamp(1.0, 28.0) as i32;
            if self.brush == 0 {
                stroke(image, x0, y0, x1, y1, item.color, alpha * 0.35, radius + 2);
                stroke(
                    image,
                    x0,
                    y0,
                    x1,
                    y1,
                    item.color,
                    alpha,
                    (radius / 2).max(1),
                );
            } else if item.soft {
                disc(image, x1, y1, radius, item.color, alpha * 0.28);
                stroke(image, x0, y0, x1, y1, item.color, alpha * 0.45, radius);
            } else {
                stroke(image, x0, y0, x1, y1, item.color, alpha, radius.max(1));
            }
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.rng >> 8) & 0x00ff_ffff) as f32 / 16_777_216.0
    }
}

impl Stage {
    fn sample_hands(&mut self, now: u64, hands: &[TrackedHand], pinch_ratio: f32) -> Vec<Sample> {
        let previous: Vec<Option<Sample>> = hands.iter().map(|hand| self.prev_for(hand)).collect();
        let poses: Vec<Pose> = hands
            .iter()
            .enumerate()
            .map(|(index, hand)| self.pose_latched(index, hand, pinch_ratio))
            .collect();
        hands
            .iter()
            .zip(previous)
            .zip(poses)
            .map(|((hand, prev), pose)| {
                let dt = prev
                    .map(|sample| frame_dt(sample.at, now))
                    .unwrap_or(1.0 / 30.0);
                let alpha = ema_alpha(dt, 0.08);
                let tip_metric = TIPS.map(|landmark| tip_point(hand, landmark));
                let tips = TIPS.map(|landmark| {
                    let (x, y) = tip(hand, landmark);
                    [x, y]
                });
                let tip_speed = [0usize, 1, 2, 3, 4].map(|finger| {
                    let raw = if let Some(sample) = prev {
                        let a = tip_metric[finger];
                        let b = sample.tip_metric[finger];
                        let dx = a[0] - b[0];
                        let dy = a[1] - b[1];
                        let dz = a[2] - b[2];
                        (dx * dx + dy * dy + dz * dz).sqrt() / dt
                    } else {
                        0.0
                    };
                    let earlier = prev.map(|sample| sample.tip_speed[finger]).unwrap_or(raw);
                    ema(earlier, raw, alpha)
                });
                let palm = palm_of(hand);
                let wrist_raw = prev
                    .map(|sample| dist2(palm, (sample.palm[0], sample.palm[1])) / dt)
                    .unwrap_or(0.0);
                let wrist_speed = ema(
                    prev.map(|sample| sample.wrist_speed).unwrap_or(wrist_raw),
                    wrist_raw,
                    alpha,
                );
                Sample {
                    track_id: hand.track_id,
                    at: now,
                    tips,
                    tip_metric,
                    tip_speed,
                    palm: [palm.0, palm.1],
                    wrist_speed,
                    pose,
                }
            })
            .collect()
    }

    fn prev_for(&self, hand: &TrackedHand) -> Option<Sample> {
        if hand.track_id != 0 {
            if let Some(sample) = self
                .prev
                .iter()
                .copied()
                .find(|sample| sample.track_id == hand.track_id)
            {
                return Some(sample);
            }
        }
        // Fallback: nearest previous palm when ids are not yet assigned.
        let palm = palm_of(hand);
        self.prev
            .iter()
            .copied()
            .min_by(|a, b| {
                dist2(palm, (a.palm[0], a.palm[1])).total_cmp(&dist2(palm, (b.palm[0], b.palm[1])))
            })
            .filter(|sample| dist2(palm, (sample.palm[0], sample.palm[1])) < 0.28)
    }

    fn pose_latched(&mut self, index: usize, hand: &TrackedHand, pinch_ratio: f32) -> Pose {
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
        let index_open = 1.0 - curls[0];
        let middle_open = 1.0 - curls[1];
        let ring_open = 1.0 - curls[2];
        let pinky_open = 1.0 - curls[3];
        let latch = index.min(1);
        let fist = self.fist_latch[latch].low(open, 0.25, 0.35)
            || (curls.iter().all(|value| *value > 0.62) && open < 0.40);
        if fist {
            return Pose::Fist;
        }
        let palm = self.palm_latch[latch].high(open, 0.55, 0.42)
            || (index_open > 0.55 && middle_open > 0.55 && ring_open > 0.55 && pinky_open > 0.55);
        if palm {
            return Pose::Palm;
        }
        if index_open > 0.55 && middle_open < 0.35 && ring_open < 0.35 && pinky_open < 0.35 {
            return Pose::Point;
        }
        Pose::Other
    }
}

fn tip(hand: &TrackedHand, landmark: usize) -> (f32, f32) {
    (hand.landmarks[landmark][0], hand.landmarks[landmark][1])
}

fn tip_index(landmark: usize) -> Option<usize> {
    TIPS.iter().position(|item| *item == landmark)
}

/// Right-hand palm resting across the string band (picking-hand mute).

fn palm_of(hand: &TrackedHand) -> (f32, f32) {
    let x = (hand.landmarks[0][0] + hand.landmarks[5][0] + hand.landmarks[17][0]) / 3.0;
    let y = (hand.landmarks[0][1] + hand.landmarks[5][1] + hand.landmarks[17][1]) / 3.0;
    (x, y)
}

fn frame_dt(prev: u64, now: u64) -> f32 {
    if now <= prev {
        return 1.0 / 30.0;
    }
    ((now - prev) as f32 / 1000.0).clamp(1.0 / 60.0, 0.1)
}

fn dist2(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn openness(hand: &TrackedHand) -> f32 {
    let shape = hand.world.as_ref().unwrap_or(&hand.landmarks);
    hand_openness(shape)
}

/// String index plus fraction of the frame [0..1] when the tip crossed the string.

fn element_color(element: u8) -> [u8; 3] {
    match element {
        0 => [251, 120, 40],
        1 => [56, 189, 248],
        2 => [253, 224, 71],
        _ => [244, 114, 182],
    }
}

fn vary(color: [u8; 3], unit: f32) -> [u8; 3] {
    let shift = (unit - 0.5) * 50.0;
    [
        (color[0] as f32 + shift).clamp(0.0, 255.0) as u8,
        (color[1] as f32 + shift * 0.4).clamp(0.0, 255.0) as u8,
        (color[2] as f32 - shift * 0.2).clamp(0.0, 255.0) as u8,
    ]
}

fn extend(from: (f32, f32), to: (f32, f32), extra: f32) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len = dx.hypot(dy).max(0.001);
    (to.0 + dx / len * extra, to.1 + dy / len * extra)
}

fn jagged(from: (f32, f32), to: (f32, f32), rng: &mut u32) -> Vec<(f32, f32)> {
    let mut pts = vec![from];
    for step in 1..7 {
        let t = step as f32 / 7.0;
        *rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let unit = ((*rng >> 8) & 0x00ff_ffff) as f32 / 16_777_216.0;
        pts.push((
            from.0 + (to.0 - from.0) * t + (unit - 0.5) * 0.03,
            from.1 + (to.1 - from.1) * t + (unit - 0.5) * 0.02,
        ));
    }
    pts.push(to);
    pts
}

fn stroke_color(mode: u8, x: f32, speed: f32, now: u64, hand: usize) -> [u8; 3] {
    let hue = match mode {
        0 => x * 360.0,
        1 => 140.0 - (speed * 0.5).clamp(0.0, 1.0) * 140.0,
        2 => 150.0 + x * 30.0,
        _ => (now as f32 * 0.06 + x * 90.0 + hand as f32 * 40.0) % 360.0,
    };
    hsv(hue, if mode == 2 { 0.65 } else { 0.78 }, 1.0)
}

fn ink_life_ms(fade: f32) -> u64 {
    let fade = fade.clamp(0.01, 0.2);
    let frames = (0.05_f32).ln() / (1.0 - fade).ln();
    (frames / 30.0 * 1000.0).clamp(280.0, 4000.0) as u64
}

fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}

fn px(image: &RgbImage, x: f32, y: f32) -> (i32, i32) {
    let w = image.width().saturating_sub(1) as f32;
    let h = image.height().saturating_sub(1) as f32;
    (
        (x.clamp(0.0, 1.0) * w).round() as i32,
        (y.clamp(0.0, 1.0) * h).round() as i32,
    )
}

fn blend(image: &mut RgbImage, x: i32, y: i32, color: [u8; 3], alpha: f32) {
    if x < 0 || y < 0 || x as u32 >= image.width() || y as u32 >= image.height() {
        return;
    }
    let pixel = image.get_pixel_mut(x as u32, y as u32);
    let alpha = alpha.clamp(0.0, 1.0);
    for channel in 0..3 {
        let mixed = pixel.0[channel] as f32 * (1.0 - alpha) + color[channel] as f32 * alpha;
        pixel.0[channel] = mixed.round().clamp(0.0, 255.0) as u8;
    }
}

fn disc(image: &mut RgbImage, cx: i32, cy: i32, radius: i32, color: [u8; 3], alpha: f32) {
    let radius = radius.max(0);
    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= radius * radius {
                blend(image, x, y, color, alpha);
            }
        }
    }
}

fn ring(image: &mut RgbImage, cx: i32, cy: i32, radius: f32, color: [u8; 3], alpha: f32) {
    if radius < 2.0 {
        return;
    }
    let mut angle = 0.0;
    let step = 1.4 / radius;
    while angle < std::f32::consts::TAU {
        let x = cx as f32 + radius * angle.cos();
        let y = cy as f32 + radius * angle.sin();
        blend(image, x.round() as i32, y.round() as i32, color, alpha);
        angle += step;
    }
}

fn fill_rect(image: &mut RgbImage, x: f32, y: f32, w: f32, h: f32, color: [u8; 3], alpha: f32) {
    let (x0, y0) = px(image, x, y);
    let (x1, y1) = px(image, x + w, y + h);
    for py in y0.min(y1)..=y0.max(y1) {
        for px in x0.min(x1)..=x0.max(x1) {
            blend(image, px, py, color, alpha);
        }
    }
}

fn thick_hline(
    image: &mut RgbImage,
    x0: f32,
    x1: f32,
    y: f32,
    color: [u8; 3],
    alpha: f32,
    thickness: i32,
) {
    let (a, py) = px(image, x0, y);
    let (b, _) = px(image, x1, y);
    let half = thickness.max(1) / 2;
    for x in a.min(b)..=a.max(b) {
        for dy in -half..=half {
            blend(image, x, py + dy, color, alpha);
        }
    }
}

fn vline(image: &mut RgbImage, x: f32, y0: f32, y1: f32, color: [u8; 3], alpha: f32) {
    let (px0, a) = px(image, x, y0);
    let (_, b) = px(image, x, y1);
    for y in a.min(b)..=a.max(b) {
        blend(image, px0, y, color, alpha);
    }
}

fn stroke(
    image: &mut RgbImage,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 3],
    alpha: f32,
    radius: i32,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        disc(image, x0, y0, radius, color, alpha);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let doubled = 2 * err;
        if doubled >= dy {
            err += dy;
            x0 += sx;
        }
        if doubled <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn dim(image: &mut RgbImage, alpha: f32) {
    for pixel in image.pixels_mut() {
        for channel in 0..3 {
            pixel.0[channel] = (pixel.0[channel] as f32 * (1.0 - alpha)).round() as u8;
        }
    }
}

fn vignette(image: &mut RgbImage, strength: f32) {
    // Sample every other pixel so the dark rim stays cheap on the live preview.
    let w = image.width().max(1) as f32;
    let h = image.height().max(1) as f32;
    let cx = w * 0.5;
    let cy = h * 0.5;
    let max_r = (cx * cx + cy * cy).sqrt().max(1.0);
    let strength = strength.clamp(0.0, 0.5);
    for y in (0..image.height()).step_by(2) {
        for x in (0..image.width()).step_by(2) {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let t = ((dx * dx + dy * dy).sqrt() / max_r).clamp(0.0, 1.0);
            let shade = 1.0 - strength * t * t;
            for oy in 0..2 {
                for ox in 0..2 {
                    let px = x + ox;
                    let py = y + oy;
                    if px < image.width() && py < image.height() {
                        let pixel = image.get_pixel_mut(px, py);
                        for channel in 0..3 {
                            pixel.0[channel] = (pixel.0[channel] as f32 * shade).round() as u8;
                        }
                    }
                }
            }
        }
    }
}

fn paint_bolt(image: &mut RgbImage, bolt: &Bolt) {
    for pair in bolt.pts.windows(2) {
        let (x0, y0) = px(image, pair[0].0, pair[0].1);
        let (x1, y1) = px(image, pair[1].0, pair[1].1);
        stroke(image, x0, y0, x1, y1, [255, 255, 255], 0.35, 5);
        stroke(image, x0, y0, x1, y1, bolt.color, 0.95, 2);
        stroke(image, x0, y0, x1, y1, [255, 255, 240], 0.55, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::HandSide;

    fn hand_with(mut place: impl FnMut(&mut [[f32; 3]; 21])) -> TrackedHand {
        let mut landmarks = [[0.5, 0.55, 0.0]; 21];
        landmarks[0] = [0.50, 0.70, 0.0];
        landmarks[9] = [0.52, 0.58, 0.0];
        landmarks[5] = [0.48, 0.60, 0.0];
        landmarks[17] = [0.56, 0.62, 0.0];
        for (tip, pip, x, y) in [
            (4, 3, 0.46, 0.50),
            (8, 6, 0.50, 0.40),
            (12, 10, 0.54, 0.40),
            (16, 14, 0.58, 0.46),
            (20, 18, 0.60, 0.50),
        ] {
            landmarks[tip] = [x, y, 0.0];
            landmarks[pip] = [x, y + 0.08, 0.0];
        }
        place(&mut landmarks);
        TrackedHand {
            track_id: 1,
            side: HandSide::Right,
            score: 0.95,
            landmarks,
            world: None,
        }
    }

    #[test]
    fn fist_does_not_leave_ink() {
        let mut stage = Stage::default();
        stage.set_mode(3);
        let fist = |x: f32| {
            hand_with(|lm| {
                lm[0] = [x, 0.6, 0.0];
                for index in 0..21 {
                    lm[index] = [x, 0.6, 0.0];
                }
                lm[9] = [x + 0.02, 0.58, 0.0];
            })
        };
        stage.step(4_000, &[fist(0.4)], 0.45);
        stage.step(4_040, &[fist(0.7)], 0.45);
        assert!(stage.ink.is_empty());
    }

    #[test]
    fn open_hand_leaves_a_trail() {
        let mut stage = Stage::default();
        stage.set_mode(3);
        let open = |x: f32| {
            hand_with(|lm| {
                lm[0] = [x, 0.7, 0.0];
                lm[9] = [x, 0.55, 0.0];
                lm[8] = [x, 0.40, 0.0];
                lm[6] = [x, 0.52, 0.0];
                lm[12] = [x + 0.04, 0.40, 0.0];
                lm[10] = [x + 0.04, 0.52, 0.0];
                lm[16] = [x + 0.08, 0.44, 0.0];
                lm[14] = [x + 0.08, 0.54, 0.0];
                lm[20] = [x + 0.11, 0.48, 0.0];
                lm[18] = [x + 0.11, 0.56, 0.0];
            })
        };
        stage.step(5_000, &[open(0.30)], 0.45);
        stage.step(5_040, &[open(0.55)], 0.45);
        assert!(!stage.ink.is_empty());
    }

    #[test]
    fn particles_stay_under_the_cap() {
        let mut stage = Stage::default();
        stage.set_mode(2);
        let fist = hand_with(|lm| {
            for index in 0..21 {
                lm[index] = [0.5, 0.5, 0.0];
            }
            lm[9] = [0.52, 0.48, 0.0];
        });
        for frame in 0..80 {
            stage.step(6_000 + frame * 30, std::slice::from_ref(&fist), 0.45);
        }
        assert!(stage.particles.len() <= MAX_PARTICLES);
        assert!(!stage.particles.is_empty());
    }
}
