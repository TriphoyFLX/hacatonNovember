use image::imageops::{self, FilterType};
use image::{Rgb, RgbImage};

use crate::coach::SceneHint;
use crate::gestures::finger_curl;
use crate::model::{NormRect, PreviewImage, TrackedHand};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandStyle {
    /// Full mesh — heavier, for the clean shared camera view.
    Full,
    /// Soft instrument overlay: tiny translucent tips, no neon.
    Tips,
    /// Alias of Tips — kept for call sites.
    Dots,
    /// Melody: same minimal tips, index slightly clearer.
    Aim,
    /// No hand glitter — melody already draws its own palm orb.
    Off,
}

const BONES: [(usize, usize); 21] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 4),
    (0, 5),
    (5, 6),
    (6, 7),
    (7, 8),
    (5, 9),
    (9, 10),
    (10, 11),
    (11, 12),
    (9, 13),
    (13, 14),
    (14, 15),
    (15, 16),
    (13, 17),
    (17, 18),
    (18, 19),
    (19, 20),
    (0, 17),
];

pub fn render_preview(
    jpeg: Option<&[u8]>,
    hands: &[TrackedHand],
    zone: NormRect,
    cursor: Option<(f32, f32)>,
    show_camera: bool,
    scene: SceneHint,
    decorate: impl FnMut(&mut RgbImage, u8),
) -> Option<PreviewImage> {
    let image = if show_camera {
        decode_jpeg(jpeg?)?
    } else {
        RgbImage::from_pixel(640, 480, Rgb([12, 16, 22]))
    };
    Some(finish_preview(
        image,
        hands,
        zone,
        cursor,
        scene,
        HandStyle::Full,
        decorate,
    ))
}

/// Paint HUD / stage overlays onto an already-decoded camera frame (avoids black flicker).
pub fn render_preview_rgb(
    image: RgbImage,
    hands: &[TrackedHand],
    zone: NormRect,
    cursor: Option<(f32, f32)>,
    scene: SceneHint,
    hand_style: HandStyle,
    decorate: impl FnMut(&mut RgbImage, u8),
) -> PreviewImage {
    finish_preview(image, hands, zone, cursor, scene, hand_style, decorate)
}

pub fn decode_jpeg(bytes: &[u8]) -> Option<RgbImage> {
    image::load_from_memory(bytes)
        .ok()
        .map(|image| image.to_rgb8())
}

pub fn downscale_rgb(image: RgbImage, max_width: u32) -> RgbImage {
    if image.width() <= max_width || max_width == 0 {
        return image;
    }
    let height = ((image.height() as f32) * (max_width as f32 / image.width() as f32))
        .round()
        .max(1.0) as u32;
    // Triangle looks like a real camera; Nearest made the stage feel like flat 2D pixels.
    imageops::resize(&image, max_width, height, FilterType::Triangle)
}

fn finish_preview(
    mut image: RgbImage,
    hands: &[TrackedHand],
    zone: NormRect,
    cursor: Option<(f32, f32)>,
    scene: SceneHint,
    hand_style: HandStyle,
    mut decorate: impl FnMut(&mut RgbImage, u8),
) -> PreviewImage {
    draw_zone(&mut image, zone);
    decorate(&mut image, 0);
    decorate(&mut image, 1);
    for hand in hands {
        match hand_style {
            HandStyle::Full => draw_hand(&mut image, hand),
            HandStyle::Tips => draw_hand_tips(&mut image, hand),
            HandStyle::Dots => draw_hand_dots(&mut image, hand),
            HandStyle::Aim => draw_hand_aim(&mut image, hand),
            HandStyle::Off => {}
        }
    }
    if let Some((x, y)) = cursor {
        let px = (x.clamp(0.0, 1.0) * (image.width().saturating_sub(1) as f32)).round() as i32;
        let py = (y.clamp(0.0, 1.0) * (image.height().saturating_sub(1) as f32)).round() as i32;
        disc(&mut image, px, py, 7, [61, 126, 255]);
    }
    draw_scene_mark(&mut image, scene);
    let width = image.width();
    let height = image.height();
    PreviewImage {
        width,
        height,
        rgb: image.into_raw(),
    }
}

fn draw_zone(image: &mut RgbImage, zone: NormRect) {
    let span_x = zone.max_x - zone.min_x;
    let span_y = zone.max_y - zone.min_y;
    if span_x >= 0.96 && span_y >= 0.96 {
        return;
    }
    let w = image.width() as i32;
    let h = image.height() as i32;
    let x0 = (zone.min_x * w as f32) as i32;
    let y0 = (zone.min_y * h as f32) as i32;
    let x1 = (zone.max_x * w as f32) as i32;
    let y1 = (zone.max_y * h as f32) as i32;
    for x in x0..x1 {
        plot(image, x, y0, [244, 236, 214]);
        plot(image, x, y1, [244, 236, 214]);
    }
    for y in y0..y1 {
        plot(image, x0, y, [244, 236, 214]);
        plot(image, x1, y, [244, 236, 214]);
    }
}

fn draw_hand_tips(image: &mut RgbImage, hand: &TrackedHand) {
    draw_hand_minimal(image, hand, false);
}

fn draw_hand_dots(image: &mut RgbImage, hand: &TrackedHand) {
    draw_hand_minimal(image, hand, false);
}

fn draw_hand_aim(image: &mut RgbImage, hand: &TrackedHand) {
    draw_hand_minimal(image, hand, true);
}

/// Quiet instrument overlay: no rainbow tips, no skeleton noise.
/// Small frosted dots; index is only a touch clearer for melody aim.
fn draw_hand_minimal(image: &mut RgbImage, hand: &TrackedHand, emphasize_index: bool) {
    let w = image.width().saturating_sub(1) as f32;
    let h = image.height().saturating_sub(1) as f32;
    let tip = |index: usize| {
        (
            (hand.landmarks[index][0] * w).round() as i32,
            (hand.landmarks[index][1] * h).round() as i32,
        )
    };
    // Neutral frost — reads on both bright and dark camera frames.
    let frost = [248, 250, 252];
    let mute = [226, 232, 240];
    for idx in [4usize, 12, 16, 20] {
        let (x, y) = tip(idx);
        soft_disc(image, x, y, 3, mute, 0.28);
        soft_disc(image, x, y, 1, frost, 0.45);
    }
    let (ix, iy) = tip(8);
    let (ir, ia) = if emphasize_index { (4, 0.38) } else { (3, 0.32) };
    soft_disc(image, ix, iy, ir + 1, mute, ia * 0.7);
    soft_disc(image, ix, iy, ir, frost, ia + 0.12);
    soft_ring(image, ix, iy, ir as f32 + 2.5, frost, 0.22);
}

fn soft_disc(image: &mut RgbImage, cx: i32, cy: i32, radius: i32, color: [u8; 3], alpha: f32) {
    let radius = radius.max(0);
    let r2 = radius * radius;
    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r2 {
                // Falloff toward the edge keeps markers soft.
                let t = 1.0 - ((dx * dx + dy * dy) as f32 / (r2.max(1) as f32)).sqrt();
                blend(image, x, y, color, (alpha * (0.35 + 0.65 * t)).clamp(0.0, 1.0));
            }
        }
    }
}

fn soft_ring(image: &mut RgbImage, cx: i32, cy: i32, radius: f32, color: [u8; 3], alpha: f32) {
    if radius < 1.5 {
        return;
    }
    let steps = ((radius * 4.0).round() as i32).clamp(10, 28);
    for i in 0..steps {
        let t = i as f32 / steps as f32 * std::f32::consts::TAU;
        let x = cx + (t.cos() * radius).round() as i32;
        let y = cy + (t.sin() * radius).round() as i32;
        blend(image, x, y, color, alpha);
    }
}

fn draw_hand(image: &mut RgbImage, hand: &TrackedHand) {
    let w = image.width().saturating_sub(1) as f32;
    let h = image.height().saturating_sub(1) as f32;
    let point = |index: usize| {
        (
            (hand.landmarks[index][0] * w).round() as i32,
            (hand.landmarks[index][1] * h).round() as i32,
        )
    };
    let shape = hand.world.as_ref().unwrap_or(&hand.landmarks);
    let open = [
        1.0 - finger_curl(shape, [1, 2, 3, 4]),
        1.0 - finger_curl(shape, [5, 6, 7, 8]),
        1.0 - finger_curl(shape, [9, 10, 11, 12]),
        1.0 - finger_curl(shape, [13, 14, 15, 16]),
        1.0 - finger_curl(shape, [17, 18, 19, 20]),
    ];
    let finger_of = |index: usize| -> usize {
        match index {
            1..=4 => 0,
            5..=8 => 1,
            9..=12 => 2,
            13..=16 => 3,
            17..=20 => 4,
            _ => 1,
        }
    };
    let color_of = |finger: usize| -> [u8; 3] {
        if open[finger] > 0.55 {
            [46, 184, 120]
        } else if open[finger] < 0.35 {
            [150, 148, 142]
        } else {
            [214, 196, 120]
        }
    };
    let wrist = point(0);
    let middle = point(9);
    let scale = ((wrist.0 - middle.0).pow(2) + (wrist.1 - middle.1).pow(2)) as f32;
    let radius = (scale.sqrt() * 0.13).clamp(4.0, 16.0) as i32;
    let skin = [255, 224, 196];
    for (a, b, c) in [(0, 5, 9), (0, 9, 13), (0, 13, 17), (5, 9, 13), (9, 13, 17)] {
        fill_triangle(image, point(a), point(b), point(c), skin, 0.42);
    }
    for (start, end) in BONES {
        let from = point(start);
        let to = point(end);
        let width = if matches!(end, 4 | 8 | 12 | 16 | 20) {
            (radius - 1).max(3)
        } else {
            radius
        };
        thick(image, from.0, from.1, to.0, to.1, color_of(finger_of(end)), width);
    }
    for index in 0..21 {
        let (x, y) = point(index);
        let finger = finger_of(index);
        let (color, size) = match index {
            8 => (color_of(1), radius + 2),
            4 => (color_of(0), radius + 1),
            12 | 16 | 20 => (color_of(finger), radius),
            _ => (color_of(finger), (radius - 1).max(3)),
        };
        disc(image, x, y, size, color);
    }
}

fn draw_scene_mark(image: &mut RgbImage, scene: SceneHint) {
    let color = match scene {
        SceneHint::Ok => return,
        SceneHint::Dark => [56, 120, 220],
        SceneHint::Bright => [240, 190, 48],
        SceneHint::Flat => [220, 120, 48],
        SceneHint::Soft => [180, 160, 220],
        SceneHint::Flicker => [96, 180, 200],
    };
    let h = 8u32.min(image.height());
    for y in 0..h {
        for x in 0..image.width() {
            blend(image, x as i32, y as i32, color, 0.7);
        }
    }
}

fn fill_triangle(
    image: &mut RgbImage,
    a: (i32, i32),
    b: (i32, i32),
    c: (i32, i32),
    color: [u8; 3],
    alpha: f32,
) {
    let min_x = a.0.min(b.0).min(c.0);
    let max_x = a.0.max(b.0).max(c.0);
    let min_y = a.1.min(b.1).min(c.1);
    let max_y = a.1.max(b.1).max(c.1);
    let area = edge(a, b, c);
    if area == 0 {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = (x, y);
            let w0 = edge(b, c, p);
            let w1 = edge(c, a, p);
            let w2 = edge(a, b, p);
            let inside = if area > 0 {
                w0 >= 0 && w1 >= 0 && w2 >= 0
            } else {
                w0 <= 0 && w1 <= 0 && w2 <= 0
            };
            if inside {
                blend(image, x, y, color, alpha);
            }
        }
    }
}

fn edge(a: (i32, i32), b: (i32, i32), p: (i32, i32)) -> i32 {
    (p.0 - a.0) * (b.1 - a.1) - (p.1 - a.1) * (b.0 - a.0)
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

fn thick(image: &mut RgbImage, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 3], radius: i32) {
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    let stride = radius.max(1);
    let mut step = 0;
    while step <= steps {
        let t = step as f32 / steps as f32;
        let x = x0 + ((x1 - x0) as f32 * t).round() as i32;
        let y = y0 + ((y1 - y0) as f32 * t).round() as i32;
        disc(image, x, y, radius, color);
        step += stride;
    }
    disc(image, x1, y1, radius, color);
}

fn disc(image: &mut RgbImage, cx: i32, cy: i32, radius: i32, color: [u8; 3]) {
    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            plot(image, x, y, color);
        }
    }
}

fn plot(image: &mut RgbImage, x: i32, y: i32, color: [u8; 3]) {
    if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
        image.put_pixel(x as u32, y as u32, Rgb(color));
    }
}
