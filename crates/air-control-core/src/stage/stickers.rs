//! Telegram-style reaction stickers (Fluent Emoji 3D, MIT).

use std::sync::OnceLock;

use image::{RgbaImage, RgbImage};

const HEART: &[u8] = include_bytes!("../../assets/stickers/heart.png");
const LIKE: &[u8] = include_bytes!("../../assets/stickers/like.png");
const DISLIKE: &[u8] = include_bytes!("../../assets/stickers/dislike.png");
const PEACE: &[u8] = include_bytes!("../../assets/stickers/peace.png");
const SPARK: &[u8] = include_bytes!("../../assets/stickers/spark.png");
const FINGER: &[u8] = include_bytes!("../../assets/stickers/finger.png");

pub fn paint(
    image: &mut RgbImage,
    kind: u8,
    cx: i32,
    cy: i32,
    size: f32,
    alpha: f32,
    spin: f32,
) {
    let Some(sticker) = sheet(kind) else {
        return;
    };
    blit(image, sticker, cx, cy, size.max(8.0), alpha.clamp(0.0, 1.0), spin);
}

fn sheet(kind: u8) -> Option<&'static RgbaImage> {
    let slot: &'static OnceLock<Option<RgbaImage>> = match kind {
        1 => {
            static CELL: OnceLock<Option<RgbaImage>> = OnceLock::new();
            &CELL
        }
        2 => {
            static CELL: OnceLock<Option<RgbaImage>> = OnceLock::new();
            &CELL
        }
        3 => {
            static CELL: OnceLock<Option<RgbaImage>> = OnceLock::new();
            &CELL
        }
        4 => {
            static CELL: OnceLock<Option<RgbaImage>> = OnceLock::new();
            &CELL
        }
        5 => {
            static CELL: OnceLock<Option<RgbaImage>> = OnceLock::new();
            &CELL
        }
        6 => {
            static CELL: OnceLock<Option<RgbaImage>> = OnceLock::new();
            &CELL
        }
        _ => return None,
    };
    slot.get_or_init(|| decode(bytes(kind))).as_ref()
}

fn bytes(kind: u8) -> &'static [u8] {
    match kind {
        1 => HEART,
        2 => LIKE,
        3 => DISLIKE,
        4 => PEACE,
        5 => SPARK,
        6 => FINGER,
        _ => SPARK,
    }
}

fn decode(raw: &[u8]) -> Option<RgbaImage> {
    image::load_from_memory(raw).ok().map(|image| image.to_rgba8())
}

fn blit(
    image: &mut RgbImage,
    sticker: &RgbaImage,
    cx: i32,
    cy: i32,
    size: f32,
    alpha: f32,
    spin: f32,
) {
    let sw = sticker.width().max(1) as f32;
    let sh = sticker.height().max(1) as f32;
    let scale = size / sw.max(sh);
    if scale <= 0.0 || alpha <= 0.01 {
        return;
    }
    let cos = spin.cos();
    let sin = spin.sin();
    let extent = (size * 0.78 + 4.0).ceil() as i32;
    for oy in -extent..=extent {
        for ox in -extent..=extent {
            let rx = ox as f32 * cos + oy as f32 * sin;
            let ry = -ox as f32 * sin + oy as f32 * cos;
            let sx = rx / scale + sw * 0.5;
            let sy = ry / scale + sh * 0.5;
            let pixel = sample(sticker, sx, sy);
            if pixel[3] < 10 {
                continue;
            }
            let cover = (pixel[3] as f32 / 255.0) * alpha;
            if cover > 0.18 {
                blend(
                    image,
                    cx + ox + 2,
                    cy + oy + 3,
                    [20, 16, 12],
                    cover * 0.22,
                );
            }
            blend(image, cx + ox, cy + oy, [pixel[0], pixel[1], pixel[2]], cover);
        }
    }
}

fn sample(sticker: &RgbaImage, x: f32, y: f32) -> [u8; 4] {
    let w = sticker.width();
    let h = sticker.height();
    if w == 0 || h == 0 || x < 0.0 || y < 0.0 || x > (w - 1) as f32 || y > (h - 1) as f32 {
        return [0, 0, 0, 0];
    }
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let a = sticker.get_pixel(x0, y0).0;
    let b = sticker.get_pixel(x1, y0).0;
    let c = sticker.get_pixel(x0, y1).0;
    let d = sticker.get_pixel(x1, y1).0;
    [
        lerp4(a[0], b[0], c[0], d[0], tx, ty),
        lerp4(a[1], b[1], c[1], d[1], tx, ty),
        lerp4(a[2], b[2], c[2], d[2], tx, ty),
        lerp4(a[3], b[3], c[3], d[3], tx, ty),
    ]
}

fn lerp4(a: u8, b: u8, c: u8, d: u8, tx: f32, ty: f32) -> u8 {
    let top = a as f32 + (b as f32 - a as f32) * tx;
    let bottom = c as f32 + (d as f32 - c as f32) * tx;
    (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stickers_decode() {
        for kind in 1..=6 {
            assert!(sheet(kind).is_some(), "sticker {kind}");
            assert!(sheet(kind).unwrap().width() >= 64);
        }
    }
}
