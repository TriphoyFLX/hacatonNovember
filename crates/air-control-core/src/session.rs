//! Session recorder: dump tracking frames to JSONL for offline replay / regression.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::protocol::TrackingHand;
use crate::store::app_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFrame {
    pub ts: u64,
    pub frame_ms: u64,
    pub sharpness: f32,
    pub brightness: f32,
    pub flicker: f32,
    pub hands: Vec<SessionHand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHand {
    pub handedness: String,
    pub score: f32,
    pub landmarks: [[f32; 3]; 21],
    #[serde(default)]
    pub world: Option<[[f32; 3]; 21]>,
}

impl From<&TrackingHand> for SessionHand {
    fn from(hand: &TrackingHand) -> Self {
        Self {
            handedness: hand.handedness.clone(),
            score: hand.score,
            landmarks: hand.landmarks,
            world: hand.world,
        }
    }
}

pub struct SessionRecorder {
    writer: BufWriter<File>,
    path: PathBuf,
    frames: u64,
}

impl SessionRecorder {
    pub fn start() -> Result<Self, CoreError> {
        let dir = app_dir().join("sessions");
        fs::create_dir_all(&dir).map_err(|err| CoreError::Storage(err.to_string()))?;
        let stamp = chrono_stamp();
        let path = dir.join(format!("session-{stamp}.jsonl"));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|err| CoreError::Storage(err.to_string()))?;
        Ok(Self {
            writer: BufWriter::new(file),
            path,
            frames: 0,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn push(&mut self, frame: &SessionFrame) -> Result<(), CoreError> {
        let line = serde_json::to_string(frame)?;
        self.writer
            .write_all(line.as_bytes())
            .map_err(|err| CoreError::Storage(err.to_string()))?;
        self.writer
            .write_all(b"\n")
            .map_err(|err| CoreError::Storage(err.to_string()))?;
        self.frames = self.frames.saturating_add(1);
        if self.frames % 30 == 0 {
            self.writer
                .flush()
                .map_err(|err| CoreError::Storage(err.to_string()))?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<PathBuf, CoreError> {
        self.writer
            .flush()
            .map_err(|err| CoreError::Storage(err.to_string()))?;
        Ok(self.path)
    }
}

pub fn read_session(path: &Path) -> Result<Vec<SessionFrame>, CoreError> {
    let file = File::open(path).map_err(|err| CoreError::Storage(err.to_string()))?;
    let reader = BufReader::new(file);
    let mut frames = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|err| CoreError::Storage(err.to_string()))?;
        if line.trim().is_empty() {
            continue;
        }
        frames.push(serde_json::from_str(&line)?);
    }
    Ok(frames)
}

/// Cheap offline check used by regression tests: palm travel and tip span stay finite.
pub fn replay_sanity(frames: &[SessionFrame]) -> Result<(usize, f32), CoreError> {
    if frames.is_empty() {
        return Err(CoreError::Rejected("empty session".into()));
    }
    let mut travel = 0.0_f32;
    let mut last_palm: Option<(f32, f32)> = None;
    let mut hands = 0usize;
    for frame in frames {
        for hand in &frame.hands {
            hands = hands.saturating_add(1);
            if hand.landmarks.iter().any(|p| p.iter().any(|v| !v.is_finite())) {
                return Err(CoreError::Rejected("non-finite landmark".into()));
            }
            let palm = (
                (hand.landmarks[0][0] + hand.landmarks[5][0] + hand.landmarks[17][0]) / 3.0,
                (hand.landmarks[0][1] + hand.landmarks[5][1] + hand.landmarks[17][1]) / 3.0,
            );
            if let Some(prev) = last_palm {
                travel += (palm.0 - prev.0).hypot(palm.1 - prev.1);
            }
            last_palm = Some(palm);
        }
    }
    Ok((hands, travel))
}

pub fn latest_session_path() -> Option<PathBuf> {
    let dir = app_dir().join("sessions");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let modified = entry.metadata().ok()?.modified().ok()?;
        if best.as_ref().is_none_or(|(time, _)| modified > *time) {
            best = Some((modified, path));
        }
    }
    best.map(|(_, path)| path)
}

/// Suggest confidence / dwell tweaks from residual tip jitter in a recording.
/// Returns (human text, optional confidence to apply).
pub fn suggest_thresholds(frames: &[SessionFrame], current_confidence: f32) -> (String, Option<f32>) {
    if frames.len() < 30 {
        return (
            "Мало кадров в записи — подержите руку дольше.".into(),
            None,
        );
    }
    let mut tip_steps = Vec::new();
    let mut last: Option<(f32, f32)> = None;
    let mut soft = 0u32;
    for frame in frames {
        if frame.sharpness < 50.0 || frame.brightness < 45.0 {
            soft = soft.saturating_add(1);
        }
        let Some(hand) = frame.hands.first() else { continue };
        let tip = (hand.landmarks[8][0], hand.landmarks[8][1]);
        if let Some(prev) = last {
            tip_steps.push((tip.0 - prev.0).hypot(tip.1 - prev.1));
        }
        last = Some(tip);
    }
    if tip_steps.len() < 20 {
        return ("В записи почти нет руки.".into(), None);
    }
    let mean = tip_steps.iter().sum::<f32>() / tip_steps.len() as f32;
    let var = tip_steps
        .iter()
        .map(|v| {
            let d = *v - mean;
            d * d
        })
        .sum::<f32>()
        / tip_steps.len() as f32;
    let sigma = var.sqrt();
    let soft_ratio = soft as f32 / frames.len() as f32;
    let mut conf = current_confidence;
    let mut notes = Vec::new();
    if sigma > 0.012 {
        conf = (conf + 0.04).min(0.9);
        notes.push("дрожь высокая → поднять уверенность");
    } else if sigma < 0.004 && soft_ratio < 0.2 {
        conf = (conf - 0.03).max(0.55);
        notes.push("рука спокойная → можно чуть снизить порог");
    }
    if soft_ratio > 0.35 {
        notes.push("много смазанных/тёмных кадров");
    }
    if notes.is_empty() {
        (
            format!(
                "Пороги в норме (σ={sigma:.4}). Уверенность оставить {current_confidence:.2}."
            ),
            None,
        )
    } else {
        (
            format!(
                "Предложение: уверенность {conf:.2} (сейчас {current_confidence:.2}). {}",
                notes.join("; ")
            ),
            Some(conf),
        )
    }
}

fn chrono_stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{now}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_rejects_empty_and_accepts_fixture() {
        assert!(replay_sanity(&[]).is_err());
        let mut landmarks = [[0.0; 3]; 21];
        for (i, point) in landmarks.iter_mut().enumerate() {
            point[0] = 0.4 + i as f32 * 0.01;
            point[1] = 0.5;
        }
        let frames = vec![
            SessionFrame {
                ts: 1,
                frame_ms: 16,
                sharpness: 90.0,
                brightness: 120.0,
                flicker: 2.0,
                hands: vec![SessionHand {
                    handedness: "Right".into(),
                    score: 0.9,
                    landmarks,
                    world: None,
                }],
            },
            SessionFrame {
                ts: 2,
                frame_ms: 33,
                sharpness: 88.0,
                brightness: 118.0,
                flicker: 3.0,
                hands: vec![SessionHand {
                    handedness: "Right".into(),
                    score: 0.91,
                    landmarks: {
                        let mut next = landmarks;
                        next[0][0] += 0.02;
                        next
                    },
                    world: None,
                }],
            },
        ];
        let (hands, travel) = replay_sanity(&frames).unwrap();
        assert_eq!(hands, 2);
        assert!(travel > 0.0);
    }
}
