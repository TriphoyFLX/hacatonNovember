use image::{Rgb, RgbImage};

use crate::model::{NormRect, PreviewImage, TrackedHand};

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
) -> Option<PreviewImage> {
    let mut image = if show_camera {
        match jpeg.and_then(decode_jpeg) {
            Some(image) => image,
            None => return None,
        }
    } else {
        RgbImage::from_pixel(640, 480, Rgb([12, 16, 22]))
    };
    draw_zone(&mut image, zone);
    for hand in hands {
        draw_hand(&mut image, hand);
    }
    if let Some((x, y)) = cursor {
        let px = (x.clamp(0.0, 1.0) * (image.width().saturating_sub(1) as f32)).round() as i32;
        let py = (y.clamp(0.0, 1.0) * (image.height().saturating_sub(1) as f32)).round() as i32;
        disc(&mut image, px, py, 7, [61, 126, 255]);
    }
    let width = image.width();
    let height = image.height();
    Some(PreviewImage {
        width,
        height,
        rgb: image.into_raw(),
    })
}

fn decode_jpeg(bytes: &[u8]) -> Option<RgbImage> {
    image::load_from_memory(bytes)
        .ok()
        .map(|image| image.to_rgb8())
}

fn draw_zone(image: &mut RgbImage, zone: NormRect) {
    let w = image.width() as i32;
    let h = image.height() as i32;
    let x0 = (zone.min_x * w as f32) as i32;
    let y0 = (zone.min_y * h as f32) as i32;
    let x1 = (zone.max_x * w as f32) as i32;
    let y1 = (zone.max_y * h as f32) as i32;
    for x in x0..x1 {
        plot(image, x, y0, [62, 224, 255]);
        plot(image, x, y1, [62, 224, 255]);
    }
    for y in y0..y1 {
        plot(image, x0, y, [62, 224, 255]);
        plot(image, x1, y, [62, 224, 255]);
    }
}

fn draw_hand(image: &mut RgbImage, hand: &TrackedHand) {
    let w = image.width().saturating_sub(1) as f32;
    let h = image.height().saturating_sub(1) as f32;
    let px = |index: usize| (hand.landmarks[index][0] * w).round() as i32;
    let py = |index: usize| (hand.landmarks[index][1] * h).round() as i32;
    for (start, end) in BONES {
        line(
            image,
            px(start),
            py(start),
            px(end),
            py(end),
            [62, 224, 255],
        );
    }
    for index in 0..21 {
        let color = match index {
            8 => [61, 126, 255],
            4 => [80, 230, 200],
            _ => [232, 244, 255],
        };
        disc(
            image,
            px(index),
            py(index),
            if index == 8 { 5 } else { 3 },
            color,
        );
    }
}

fn line(image: &mut RgbImage, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: [u8; 3]) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        disc(image, x0, y0, 1, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
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
