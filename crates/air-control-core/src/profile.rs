//! Optional pipeline profiler (`AIR_CONTROL_PROFILE=1`).
//!
//! Samples go to JSONL files under `AIR_CONTROL_PROFILE_DIR` (default
//! `/tmp/air-control-profile`). Never writes to the vision stdout protocol.
//! When the env flag is unset, every helper is a cheap OnceLock check.

use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static ENABLED: OnceLock<bool> = OnceLock::new();
static WRITE_BROKEN: AtomicBool = AtomicBool::new(false);
static RUST_WRITER: OnceLock<Mutex<ProfileWriter>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();
thread_local! {
    static LAST_B64_MS: std::cell::Cell<f32> = const { std::cell::Cell::new(-1.0) };
}

pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| {
        std::env::var_os("AIR_CONTROL_PROFILE")
            .map(|value| value == "1")
            .unwrap_or(false)
    })
}

pub fn dir() -> PathBuf {
    std::env::var_os("AIR_CONTROL_PROFILE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/air-control-profile"))
}

pub fn mono_ms() -> f64 {
    let start = START.get_or_init(Instant::now);
    duration_ms(start.elapsed())
}

pub fn set_b64_ms(ms: f32) {
    if enabled() {
        LAST_B64_MS.with(|cell| cell.set(ms));
    }
}

pub fn take_b64_ms() -> Option<f32> {
    if !enabled() {
        return None;
    }
    let value = LAST_B64_MS.with(|cell| cell.replace(-1.0));
    (value >= 0.0).then_some(value)
}

pub fn ms_f32(duration: Duration) -> f32 {
    (duration.as_secs_f64() * 1000.0) as f32
}

pub fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

pub fn write_parse_sample(
    parse_ms: f32,
    b64_ms: Option<f32>,
    json_bytes: usize,
    jpeg_bytes: Option<usize>,
    had_preview: bool,
) {
    if !enabled() || WRITE_BROKEN.load(Ordering::Relaxed) {
        return;
    }
    let value = serde_json::json!({
        "type": "parse",
        "mono_ms": mono_ms(),
        "parse_ms": parse_ms,
        "b64_ms": b64_ms,
        "json_bytes": json_bytes,
        "jpeg_bytes": jpeg_bytes,
        "had_preview": had_preview,
    });
    write_line("rust.jsonl", &value);
}

pub fn write_render_sample(
    jpeg_decode_ms: Option<f32>,
    downscale_ms: Option<f32>,
    paint_ms: Option<f32>,
    had_preview: bool,
) {
    if !enabled() || WRITE_BROKEN.load(Ordering::Relaxed) {
        return;
    }
    let value = serde_json::json!({
        "type": "render",
        "mono_ms": mono_ms(),
        "jpeg_decode_ms": jpeg_decode_ms,
        "downscale_ms": downscale_ms,
        "paint_ms": paint_ms,
        "had_preview": had_preview,
    });
    write_line("rust.jsonl", &value);
}

pub fn write_ui_sample(frame_image_ms: f32, kind: &str, width: u32, height: u32) {
    if !enabled() || WRITE_BROKEN.load(Ordering::Relaxed) {
        return;
    }
    let value = serde_json::json!({
        "type": "ui_refresh",
        "mono_ms": mono_ms(),
        "frame_image_ms": frame_image_ms,
        "kind": kind,
        "width": width,
        "height": height,
    });
    write_line("ui.jsonl", &value);
}

fn write_line(file_name: &str, value: &serde_json::Value) {
    let Some(writer) = rust_writer() else {
        return;
    };
    let Ok(mut guard) = writer.lock() else {
        return;
    };
    if let Err(error) = guard.write_named(file_name, value) {
        mark_broken(&error);
    }
}

fn rust_writer() -> Option<&'static Mutex<ProfileWriter>> {
    if !enabled() || WRITE_BROKEN.load(Ordering::Relaxed) {
        return None;
    }
    Some(RUST_WRITER.get_or_init(|| Mutex::new(ProfileWriter::new())))
}

struct ProfileWriter {
    dir_ok: bool,
    rust: Option<BufWriter<File>>,
    ui: Option<BufWriter<File>>,
    rust_lines: u32,
    ui_lines: u32,
}

impl ProfileWriter {
    fn new() -> Self {
        let path = dir();
        let dir_ok = match create_dir_all(&path) {
            Ok(()) => true,
            Err(error) => {
                mark_broken(&error);
                false
            }
        };
        Self {
            dir_ok,
            rust: None,
            ui: None,
            rust_lines: 0,
            ui_lines: 0,
        }
    }

    fn write_named(&mut self, file_name: &str, value: &serde_json::Value) -> std::io::Result<()> {
        if !self.dir_ok {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "profile dir unavailable",
            ));
        }
        let line = serde_json::to_vec(value)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        match file_name {
            "ui.jsonl" => {
                self.ui_lines = self.ui_lines.wrapping_add(1);
                let flush = self.ui_lines % 32 == 0;
                let writer = self.ensure_ui()?;
                writer.write_all(&line)?;
                writer.write_all(b"\n")?;
                if flush {
                    writer.flush()?;
                }
            }
            _ => {
                self.rust_lines = self.rust_lines.wrapping_add(1);
                let flush = self.rust_lines % 32 == 0;
                let writer = self.ensure_rust()?;
                writer.write_all(&line)?;
                writer.write_all(b"\n")?;
                if flush {
                    writer.flush()?;
                }
            }
        }
        Ok(())
    }

    fn ensure_rust(&mut self) -> std::io::Result<&mut BufWriter<File>> {
        if self.rust.is_none() {
            self.rust = Some(open_jsonl("rust.jsonl")?);
        }
        Ok(self.rust.as_mut().expect("rust writer"))
    }

    fn ensure_ui(&mut self) -> std::io::Result<&mut BufWriter<File>> {
        if self.ui.is_none() {
            self.ui = Some(open_jsonl("ui.jsonl")?);
        }
        Ok(self.ui.as_mut().expect("ui writer"))
    }
}

fn open_jsonl(file_name: &str) -> std::io::Result<BufWriter<File>> {
    let path = dir().join(file_name);
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    Ok(BufWriter::with_capacity(16 * 1024, file))
}

fn mark_broken(error: &dyn std::fmt::Display) {
    if !WRITE_BROKEN.swap(true, Ordering::Relaxed) {
        tracing::warn!("AIR_CONTROL_PROFILE: запись диагностики отключена: {error}");
    }
}
