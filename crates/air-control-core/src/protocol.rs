use serde::Deserialize;

use crate::error::CoreError;
use crate::PROTOCOL_VERSION;

#[derive(Debug, Clone)]
pub struct CameraDevice {
    pub index: i32,
    pub name: String,
    pub available: bool,
}

#[derive(Debug, Clone)]
pub struct TrackingHand {
    pub handedness: String,
    pub score: f32,
    pub landmarks: [[f32; 3]; 21],
}

#[derive(Debug, Clone)]
pub struct TrackingMessage {
    pub timestamp_ms: u64,
    pub capture_ms: f32,
    pub inference_ms: f32,
    pub frame_id: u64,
    pub width: u32,
    pub height: u32,
    pub hands: Vec<TrackingHand>,
    pub preview_jpeg: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub enum Incoming {
    Hello {
        pid: u32,
        mediapipe: String,
    },
    Cameras {
        devices: Vec<CameraDevice>,
    },
    CameraStatus {
        state: String,
        index: i32,
        width: u32,
        height: u32,
        fps: u32,
        message: String,
    },
    Heartbeat {
        timestamp_ms: u64,
    },
    Tracking(TrackingMessage),
    Error {
        code: String,
        message: String,
    },
    Goodbye,
}

#[derive(Debug)]
pub enum ParseOutcome {
    Message(Incoming),
    Ignored { reason: String },
}

pub fn parse_vision_line(line: &str) -> Result<ParseOutcome, CoreError> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(ParseOutcome::Ignored {
            reason: "empty".into(),
        });
    }
    if line.len() > 2_000_000 {
        return Err(CoreError::Protocol("message exceeds 2 MB".into()));
    }
    let value: serde_json::Value = serde_json::from_str(line)?;
    let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    if version != u64::from(PROTOCOL_VERSION) {
        return Err(CoreError::Protocol(format!(
            "unsupported protocol version {version}"
        )));
    }
    let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "hello" => {
            let pid = value
                .get("process_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let mediapipe = text_field(&value, "mediapipe", 40);
            Ok(ParseOutcome::Message(Incoming::Hello { pid, mediapipe }))
        }
        "cameras" => {
            let mut devices = Vec::new();
            if let Some(items) = value.get("devices").and_then(|v| v.as_array()) {
                for item in items.iter().take(8) {
                    let index = item.get("index").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Camera")
                        .chars()
                        .take(80)
                        .collect();
                    let available = item
                        .get("available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    devices.push(CameraDevice {
                        index,
                        name,
                        available,
                    });
                }
            }
            Ok(ParseOutcome::Message(Incoming::Cameras { devices }))
        }
        "camera_status" => Ok(ParseOutcome::Message(Incoming::CameraStatus {
            state: text_field(&value, "state", 40),
            index: value.get("index").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            width: value.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            height: value.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            fps: value.get("fps").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            message: text_field(&value, "message", 240),
        })),
        "heartbeat" => Ok(ParseOutcome::Message(Incoming::Heartbeat {
            timestamp_ms: value
                .get("timestamp_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        })),
        "tracking" => Ok(ParseOutcome::Message(Incoming::Tracking(parse_tracking(
            &value,
        )?))),
        "error" => Ok(ParseOutcome::Message(Incoming::Error {
            code: text_field(&value, "code", 60),
            message: text_field(&value, "message", 300),
        })),
        "goodbye" => Ok(ParseOutcome::Message(Incoming::Goodbye)),
        other => Ok(ParseOutcome::Ignored {
            reason: format!("unknown type {other}"),
        }),
    }
}

fn parse_tracking(value: &serde_json::Value) -> Result<TrackingMessage, CoreError> {
    #[derive(Deserialize)]
    struct RawHand {
        handedness: String,
        score: f32,
        landmarks: Vec<[f32; 3]>,
    }
    #[derive(Deserialize)]
    struct Raw {
        timestamp_ms: u64,
        capture_ms: f32,
        inference_ms: f32,
        frame_id: u64,
        width: u32,
        height: u32,
        hands: Vec<RawHand>,
        #[serde(default)]
        preview_jpeg_b64: Option<String>,
    }
    let raw: Raw = serde_json::from_value(value.clone())?;
    if raw.hands.len() > 2 {
        return Err(CoreError::Protocol("too many hands".into()));
    }
    if !raw.capture_ms.is_finite()
        || !raw.inference_ms.is_finite()
        || raw.capture_ms < 0.0
        || raw.inference_ms < 0.0
        || raw.capture_ms > 5_000.0
        || raw.inference_ms > 5_000.0
    {
        return Err(CoreError::Protocol("latency fields are invalid".into()));
    }
    let mut hands = Vec::with_capacity(raw.hands.len());
    for hand in raw.hands {
        if !(0.0..=1.0).contains(&hand.score) {
            return Err(CoreError::Protocol("hand score out of range".into()));
        }
        if hand.landmarks.len() != 21 {
            return Err(CoreError::Protocol("expected 21 landmarks".into()));
        }
        let mut landmarks = [[0.0; 3]; 21];
        for (index, point) in hand.landmarks.iter().enumerate() {
            if point.iter().any(|v| !v.is_finite())
                || !(-0.25..=1.25).contains(&point[0])
                || !(-0.25..=1.25).contains(&point[1])
                || !(-2.0..=2.0).contains(&point[2])
            {
                return Err(CoreError::Protocol("landmark coordinate is invalid".into()));
            }
            landmarks[index] = *point;
        }
        hands.push(TrackingHand {
            handedness: hand.handedness.chars().take(16).collect(),
            score: hand.score,
            landmarks,
        });
    }
    let preview_jpeg = match raw.preview_jpeg_b64 {
        Some(text) if !text.is_empty() => {
            if text.len() > 1_500_000 {
                return Err(CoreError::Protocol("preview is too large".into()));
            }
            Some(
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, text.as_bytes())
                    .map_err(|err| CoreError::Protocol(err.to_string()))?,
            )
        }
        _ => None,
    };
    Ok(TrackingMessage {
        timestamp_ms: raw.timestamp_ms,
        capture_ms: raw.capture_ms,
        inference_ms: raw.inference_ms,
        frame_id: raw.frame_id,
        width: raw.width.min(3840),
        height: raw.height.min(2160),
        hands,
        preview_jpeg,
    })
}

fn text_field(value: &serde_json::Value, key: &str, max: usize) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(max)
        .collect()
}

pub fn describe_vision_line(line: &str) -> String {
    match parse_vision_line(line) {
        Ok(ParseOutcome::Message(Incoming::Hello { .. })) => "hello".into(),
        Ok(ParseOutcome::Message(Incoming::Tracking(_))) => "tracking".into(),
        Ok(ParseOutcome::Ignored { .. }) => "ignored".into(),
        Err(_) => "error".into(),
        Ok(_) => "other".into(),
    }
}

pub fn should_restart(last_rx_ms: u64, now_ms: u64, camera_on: bool) -> bool {
    camera_on && now_ms.saturating_sub(last_rx_ms) > 8_000
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hand_json() -> String {
        let points: Vec<_> = (0..21)
            .map(|i| format!("[{:.2}, {:.2}, 0.0]", 0.4 + (i as f32) * 0.01, 0.3))
            .collect();
        format!(
            r#"{{"handedness":"Right","score":0.91,"landmarks":[{}]}}"#,
            points.join(",")
        )
    }

    #[test]
    fn parses_tracking_and_rejects_bad_payloads() {
        let line = format!(
            r#"{{"version":1,"type":"tracking","timestamp_ms":10,"capture_ms":4.0,"inference_ms":12.5,"frame_id":3,"width":640,"height":480,"hands":[{}]}}"#,
            hand_json()
        );
        match parse_vision_line(&line).unwrap() {
            ParseOutcome::Message(Incoming::Tracking(msg)) => {
                assert_eq!(msg.hands.len(), 1);
                assert_eq!(msg.hands[0].landmarks.len(), 21);
            }
            other => panic!("unexpected {other:?}"),
        }

        assert!(parse_vision_line(r#"{"version":99,"type":"heartbeat"}"#).is_err());
        assert!(parse_vision_line(r#"{"version":1,"type":"tracking","timestamp_ms":1,"capture_ms":1,"inference_ms":1,"frame_id":1,"width":1,"height":1,"hands":[{"handedness":"Right","score":0.5,"landmarks":[[0,0,0]]}]}"#).is_err());
        let nan = r#"{"version":1,"type":"tracking","timestamp_ms":1,"capture_ms":1,"inference_ms":1,"frame_id":1,"width":1,"height":1,"hands":[{"handedness":"Right","score":0.5,"landmarks":[[NaN,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0]]}]}"#;
        assert!(parse_vision_line(nan).is_err());
        match parse_vision_line(r#"{"version":1,"type":"made_up"}"#).unwrap() {
            ParseOutcome::Ignored { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn camera_loss_and_restart_policy() {
        let status = parse_vision_line(r#"{"version":1,"type":"camera_status","state":"reconnecting","index":0,"width":0,"height":0,"fps":0,"message":"device lost"}"#).unwrap();
        match status {
            ParseOutcome::Message(Incoming::CameraStatus { state, message, .. }) => {
                assert_eq!(state, "reconnecting");
                assert!(message.contains("lost"));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(!should_restart(1_000, 5_000, true));
        assert!(should_restart(1_000, 10_000, true));
        assert!(!should_restart(1_000, 20_000, false));
    }
}
