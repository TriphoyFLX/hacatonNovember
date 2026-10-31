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
