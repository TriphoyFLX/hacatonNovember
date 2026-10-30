//! Geometric gesture recognition.
//!
//! Static poses use joint distances over the 21 MediaPipe landmarks. Swipes,
//! zoom, and two-hand motion use a short time window. Custom gestures are
//! matched with dynamic time warping against a recorded template. Nothing in
//! this module is trained during calibration.

use std::collections::VecDeque;

use crate::model::{CustomGesture, HandSide, TrackedHand};

pub const BUILTIN_GESTURES: &[&str] = &[
    "index_point",
    "pinch",
    "double_pinch",
    "open_palm",
    "closed_fist",
    "v_sign",
    "thumb_up",
    "thumb_down",
    "swipe_left",
    "swipe_right",
    "swipe_up",
    "swipe_down",
    "two_hand_zoom_in",
    "two_hand_zoom_out",
    "two_hand_move_left",
    "two_hand_move_right",
];

#[derive(Debug, Clone, PartialEq)]
pub struct StaticPose {
    pub id: &'static str,
    pub confidence: f32,
}

pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

pub fn classify_static(
    landmarks: &[[f32; 3]; 21],
    pinch_ratio_threshold: f32,
) -> Option<StaticPose> {
    let scale = distance(landmarks[0], landmarks[9]).max(0.02);
    let pinch_ratio = distance(landmarks[4], landmarks[8]) / scale;
    let thumb = extension(landmarks, 4, 3);
    let index = extension(landmarks, 8, 6);
    let middle = extension(landmarks, 12, 10);
    let ring = extension(landmarks, 16, 14);
    let pinky = extension(landmarks, 20, 18);
    let fingers = [thumb, index, middle, ring, pinky];
    let extended: Vec<bool> = fingers.iter().map(|value| *value > 1.15).collect();
    let curled: Vec<bool> = fingers.iter().map(|value| *value < 1.05).collect();

    if pinch_ratio < pinch_ratio_threshold && index > 1.02 {
        let confidence = (1.15 - pinch_ratio / pinch_ratio_threshold.max(0.05)).clamp(0.55, 0.98);
        return Some(StaticPose {
            id: "pinch",
            confidence,
        });
    }
    if curled.iter().all(|value| *value) {
        return Some(StaticPose {
            id: "closed_fist",
            confidence: 0.9,
        });
    }
    if extended[0] && curled[1] && curled[2] && curled[3] && curled[4] {
        let up = landmarks[4][1] + 0.02 < landmarks[3][1];
        let down = landmarks[4][1] > landmarks[3][1] + 0.02;
        if up {
            return Some(StaticPose {
                id: "thumb_up",
                confidence: 0.88,
            });
        }
        if down {
            return Some(StaticPose {
                id: "thumb_down",
                confidence: 0.88,
            });
        }
    }
    if extended[1] && extended[2] && curled[3] && curled[4] && !extended[0] {
        return Some(StaticPose {
            id: "v_sign",
            confidence: clarity(&[index, middle]),
        });
    }
    if extended[1] && curled[2] && curled[3] && curled[4] {
        return Some(StaticPose {
            id: "index_point",
            confidence: clarity(&[index]),
        });
    }
    if extended[1] && extended[2] && extended[3] && extended[4] {
        return Some(StaticPose {
            id: "open_palm",
            confidence: clarity(&[index, middle, ring, pinky]),
        });
    }
    None
}

fn extension(landmarks: &[[f32; 3]; 21], tip: usize, pip: usize) -> f32 {
    let wrist = landmarks[0];
    let tip_distance = distance(landmarks[tip], wrist);
    let pip_distance = distance(landmarks[pip], wrist).max(1e-4);
    tip_distance / pip_distance
}

fn clarity(values: &[f32]) -> f32 {
    let mean = values.iter().sum::<f32>() / values.len().max(1) as f32;
    (0.55 + (mean - 1.15).clamp(0.0, 0.8) * 0.5).clamp(0.55, 0.97)
}

pub fn edge_penalty(landmarks: &[[f32; 3]; 21]) -> f32 {
    let outside = landmarks
        .iter()
        .filter(|point| point[0] < 0.0 || point[0] > 1.0 || point[1] < 0.0 || point[1] > 1.0)
        .count();
    if outside >= 6 {
        0.25
    } else if outside >= 3 {
        0.1
    } else {
        0.0
    }
}

pub fn normalize_landmarks(landmarks: &[[f32; 3]; 21]) -> Vec<f32> {
    let wrist = landmarks[0];
    let scale = distance(landmarks[0], landmarks[9]).max(0.02);
    let mut flat = Vec::with_capacity(63);
    for point in landmarks {
        flat.push((point[0] - wrist[0]) / scale);
        flat.push((point[1] - wrist[1]) / scale);
        flat.push((point[2] - wrist[2]) / scale);
    }
    flat
}

pub fn resample(frames: &[Vec<f32>], count: usize) -> Vec<Vec<f32>> {
    if frames.is_empty() || count == 0 {
        return Vec::new();
    }
    (0..count)
        .map(|index| {
            let t = if count == 1 {
                0.0
            } else {
                index as f32 / (count - 1) as f32
            };
            let slot = (t * (frames.len() - 1) as f32).round() as usize;
            frames[slot.min(frames.len() - 1)].clone()
        })
        .collect()
}

pub fn dtw(left: &[Vec<f32>], right: &[Vec<f32>]) -> f32 {
    let n = left.len();
    let m = right.len();
    if n == 0 || m == 0 {
        return f32::MAX;
    }
    let mut previous = vec![f32::MAX / 4.0; m + 1];
    let mut current = vec![f32::MAX / 4.0; m + 1];
    previous[0] = 0.0;
    for i in 1..=n {
        current[0] = f32::MAX / 4.0;
        for j in 1..=m {
            let cost = l2(&left[i - 1], &right[j - 1]);
            let best = previous[j].min(current[j - 1]).min(previous[j - 1]);
            current[j] = cost + best;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[m] / n.max(m) as f32
}

fn l2(left: &[f32], right: &[f32]) -> f32 {
    let n = left.len().min(right.len());
    let mut total = 0.0;
    for index in 0..n {
        let delta = left[index] - right[index];
        total += delta * delta;
    }
    (total / n.max(1) as f32).sqrt()
}

pub fn match_custom(history: &[Vec<f32>], gestures: &[CustomGesture]) -> Option<(String, f32)> {
    let sample = resample(history, 12);
    if sample.len() < 12 {
        return None;
    }
    let mut best: Option<(String, f32)> = None;
    for gesture in gestures {
        let distance = dtw(&sample, &gesture.frames);
        if distance <= gesture.threshold {
            let confidence = (1.0 - distance / gesture.threshold.max(0.05)).clamp(0.0, 0.98);
            if best.as_ref().is_none_or(|(_, score)| confidence > *score) {
                best = Some((gesture.name.clone(), confidence));
            }
        }
    }
    best
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionEvent {
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
    ZoomIn,
    ZoomOut,
    MoveLeft,
    MoveRight,
}

impl MotionEvent {
    pub fn id(self) -> &'static str {
        match self {
            Self::SwipeLeft => "swipe_left",
            Self::SwipeRight => "swipe_right",
            Self::SwipeUp => "swipe_up",
            Self::SwipeDown => "swipe_down",
            Self::ZoomIn => "two_hand_zoom_in",
            Self::ZoomOut => "two_hand_zoom_out",
            Self::MoveLeft => "two_hand_move_left",
            Self::MoveRight => "two_hand_move_right",
        }
    }
}

#[derive(Debug, Clone)]
struct Sample {
    time_ms: u64,
    x: f32,
    y: f32,
    second_x: Option<f32>,
    distance: Option<f32>,
}

#[derive(Debug, Default)]
pub struct MotionTracker {
    samples: VecDeque<Sample>,
}

impl MotionTracker {
    pub fn push(&mut self, time_ms: u64, hands: &[TrackedHand]) -> Vec<MotionEvent> {
        if hands.is_empty() {
            self.samples.clear();
            return Vec::new();
        }
        let primary = &hands[0];
        let wrist = primary.landmarks[0];
        let (second_x, distance) = if hands.len() >= 2 {
            let other = &hands[1];
            let dx = wrist[0] - other.landmarks[0][0];
            let dy = wrist[1] - other.landmarks[0][1];
            (
                Some(other.landmarks[0][0]),
                Some((dx * dx + dy * dy).sqrt()),
            )
        } else {
            (None, None)
        };
        self.samples.push_back(Sample {
            time_ms,
            x: wrist[0],
            y: wrist[1],
            second_x,
            distance,
        });
        while self.samples.len() > 24 {
            self.samples.pop_front();
        }
        let window: Vec<_> = self
            .samples
            .iter()
            .filter(|sample| time_ms.saturating_sub(sample.time_ms) <= 360)
            .cloned()
            .collect();
        if window.len() < 4 {
            return Vec::new();
        }
        let mut events = Vec::new();
        if let (Some(start), Some(end)) = (
            window.iter().find(|sample| sample.distance.is_some()),
            window.last(),
        ) {
            if let (Some(a), Some(b)) = (start.distance, end.distance) {
                let change = (b - a) / a.max(0.05);
                if change > 0.18 {
                    events.push(MotionEvent::ZoomIn);
                } else if change < -0.18 {
                    events.push(MotionEvent::ZoomOut);
                }
            }
            if window.iter().all(|sample| sample.second_x.is_some()) {
                let dx_a = end.x - window[0].x;
                let dx_b =
                    end.second_x.unwrap_or(end.x) - window[0].second_x.unwrap_or(window[0].x);
                if dx_a > 0.08 && dx_b > 0.08 {
                    events.push(MotionEvent::MoveRight);
                } else if dx_a < -0.08 && dx_b < -0.08 {
                    events.push(MotionEvent::MoveLeft);
                }
            }
        }
        if events.is_empty() {
            let dx = window.last().unwrap().x - window[0].x;
            let dy = window.last().unwrap().y - window[0].y;
            if dx.abs() > 0.14 && dx.abs() > dy.abs() * 1.35 {
                events.push(if dx > 0.0 {
                    MotionEvent::SwipeRight
                } else {
                    MotionEvent::SwipeLeft
                });
            } else if dy.abs() > 0.14 && dy.abs() > dx.abs() * 1.35 {
                events.push(if dy > 0.0 {
                    MotionEvent::SwipeDown
                } else {
                    MotionEvent::SwipeUp
                });
            }
        }
        if !events.is_empty() {
            self.samples.clear();
        }
        events
    }
}

#[derive(Debug, Default)]
pub struct HandMemory {
    last: Vec<TrackedHand>,
    missing_since: Option<u64>,
    grace_ms: u64,
}

impl HandMemory {
    pub fn new(grace_ms: u64) -> Self {
        Self {
            last: Vec::new(),
            missing_since: None,
            grace_ms,
        }
    }

    pub fn update(&mut self, now_ms: u64, hands: Vec<TrackedHand>) -> Vec<TrackedHand> {
        if hands.is_empty() {
            if self.missing_since.is_none() {
                self.missing_since = Some(now_ms);
            }
            let since = self.missing_since.unwrap_or(now_ms);
            if now_ms.saturating_sub(since) <= self.grace_ms {
                self.last.clone()
            } else {
                self.last.clear();
                Vec::new()
            }
        } else {
            self.missing_since = None;
            self.last = hands.clone();
            hands
        }
    }
}

#[derive(Debug, Clone)]
struct GateState {
    active_since: Option<u64>,
    last_fire: Option<u64>,
    latched: bool,
}

#[derive(Debug, Default)]
pub struct GateSet {
    states: std::collections::HashMap<String, GateState>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GateResult {
    Idle,
    Fired,
    Rejected(&'static str),
}

impl GateSet {
    pub fn deactivate_unlisted(&mut self, active: &[String]) {
        for (id, state) in &mut self.states {
            if !active.iter().any(|item| item == id) {
                state.active_since = None;
                state.latched = false;
            }
        }
    }

    pub fn observe(
        &mut self,
        id: &str,
        active: bool,
        min_duration_ms: u64,
        cooldown_ms: u64,
        now_ms: u64,
    ) -> GateResult {
        let state = self.states.entry(id.to_string()).or_insert(GateState {
            active_since: None,
            last_fire: None,
            latched: false,
        });
        if !active {
            state.active_since = None;
            state.latched = false;
            return GateResult::Idle;
        }
        if state.latched {
            return GateResult::Idle;
        }
        if state.active_since.is_none() {
            state.active_since = Some(now_ms);
        }
        let held = now_ms.saturating_sub(state.active_since.unwrap_or(now_ms));
        if held < min_duration_ms {
            return GateResult::Rejected("too_short");
        }
        if let Some(previous) = state.last_fire {
            if now_ms.saturating_sub(previous) < cooldown_ms {
                return GateResult::Rejected("cooldown");
            }
        }
        state.last_fire = Some(now_ms);
        state.latched = true;
        GateResult::Fired
    }
}

pub fn select_hands(
    hands: &[TrackedHand],
    preference: crate::model::PrimaryHand,
) -> Vec<TrackedHand> {
    let mut ordered = hands.to_vec();
    ordered.sort_by(|left, right| {
        let rank = |side: HandSide| match preference {
            crate::model::PrimaryHand::Right => matches!(side, HandSide::Right),
            crate::model::PrimaryHand::Left => matches!(side, HandSide::Left),
            crate::model::PrimaryHand::Any => false,
        };
        rank(right.side)
            .cmp(&rank(left.side))
            .then(right.score.total_cmp(&left.score))
    });
    ordered.truncate(2);
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(kind: &str) -> [[f32; 3]; 21] {
        let mut landmarks = [[0.50, 0.78, 0.0]; 21];
        landmarks[0] = [0.50, 0.80, 0.0];
        landmarks[1] = [0.42, 0.74, 0.0];
        landmarks[2] = [0.38, 0.68, 0.0];
        landmarks[5] = [0.42, 0.62, 0.0];
        landmarks[9] = [0.50, 0.58, 0.0];
        landmarks[13] = [0.58, 0.62, 0.0];
        landmarks[17] = [0.64, 0.66, 0.0];
        let curl =
            |lm: &mut [[f32; 3]; 21], pip: usize, dip: usize, tip: usize, point: [f32; 3]| {
                lm[pip] = point;
                lm[dip] = point;
                lm[tip] = point;
            };
        let far = |lm: &mut [[f32; 3]; 21],
                   pip: usize,
                   dip: usize,
                   tip: usize,
                   tip_point: [f32; 3],
                   pip_point: [f32; 3]| {
            lm[pip] = pip_point;
            lm[dip] = tip_point;
            lm[tip] = tip_point;
        };
        match kind {
            "fist" => {
                curl(&mut landmarks, 3, 4, 4, [0.48, 0.74, 0.0]);
                curl(&mut landmarks, 6, 7, 8, [0.48, 0.74, 0.0]);
                curl(&mut landmarks, 10, 11, 12, [0.50, 0.74, 0.0]);
                curl(&mut landmarks, 14, 15, 16, [0.52, 0.74, 0.0]);
                curl(&mut landmarks, 18, 19, 20, [0.54, 0.74, 0.0]);
            }
            "index" => {
                curl(&mut landmarks, 3, 4, 4, [0.46, 0.74, 0.0]);
                far(
                    &mut landmarks,
                    6,
                    7,
                    8,
                    [0.40, 0.30, 0.0],
                    [0.42, 0.58, 0.0],
                );
                curl(&mut landmarks, 10, 11, 12, [0.50, 0.74, 0.0]);
                curl(&mut landmarks, 14, 15, 16, [0.56, 0.74, 0.0]);
                curl(&mut landmarks, 18, 19, 20, [0.60, 0.74, 0.0]);
            }
            "v" => {
                curl(&mut landmarks, 3, 4, 4, [0.46, 0.74, 0.0]);
                far(
                    &mut landmarks,
                    6,
                    7,
                    8,
                    [0.38, 0.30, 0.0],
                    [0.42, 0.58, 0.0],
                );
                far(
                    &mut landmarks,
                    10,
                    11,
                    12,
                    [0.52, 0.28, 0.0],
                    [0.50, 0.56, 0.0],
                );
                curl(&mut landmarks, 14, 15, 16, [0.58, 0.74, 0.0]);
                curl(&mut landmarks, 18, 19, 20, [0.62, 0.74, 0.0]);
            }
            "palm" => {
                far(
                    &mut landmarks,
                    3,
                    4,
                    4,
                    [0.28, 0.58, 0.0],
                    [0.36, 0.68, 0.0],
                );
                far(
                    &mut landmarks,
                    6,
                    7,
                    8,
                    [0.38, 0.28, 0.0],
                    [0.42, 0.56, 0.0],
                );
                far(
                    &mut landmarks,
                    10,
                    11,
                    12,
                    [0.50, 0.22, 0.0],
                    [0.50, 0.52, 0.0],
                );
                far(
                    &mut landmarks,
                    14,
                    15,
                    16,
                    [0.60, 0.30, 0.0],
                    [0.58, 0.56, 0.0],
                );
                far(
                    &mut landmarks,
                    18,
                    19,
                    20,
                    [0.70, 0.38, 0.0],
                    [0.64, 0.60, 0.0],
                );
            }
            "thumb_up" => {
                far(
                    &mut landmarks,
                    3,
                    4,
                    4,
                    [0.40, 0.48, 0.0],
                    [0.40, 0.68, 0.0],
                );
                curl(&mut landmarks, 6, 7, 8, [0.48, 0.74, 0.0]);
                curl(&mut landmarks, 10, 11, 12, [0.50, 0.74, 0.0]);
                curl(&mut landmarks, 14, 15, 16, [0.54, 0.74, 0.0]);
                curl(&mut landmarks, 18, 19, 20, [0.58, 0.74, 0.0]);
            }
            "thumb_down" => {
                far(
                    &mut landmarks,
                    3,
                    4,
                    4,
                    [0.40, 0.96, 0.0],
                    [0.40, 0.70, 0.0],
                );
                curl(&mut landmarks, 6, 7, 8, [0.48, 0.74, 0.0]);
                curl(&mut landmarks, 10, 11, 12, [0.50, 0.74, 0.0]);
                curl(&mut landmarks, 14, 15, 16, [0.54, 0.74, 0.0]);
                curl(&mut landmarks, 18, 19, 20, [0.58, 0.74, 0.0]);
            }
            "pinch" => {
                far(
                    &mut landmarks,
                    6,
                    7,
                    8,
                    [0.40, 0.42, 0.0],
                    [0.42, 0.62, 0.0],
                );
                landmarks[3] = [0.40, 0.55, 0.0];
                landmarks[4] = [0.405, 0.43, 0.0];
                curl(&mut landmarks, 10, 11, 12, [0.50, 0.74, 0.0]);
                curl(&mut landmarks, 14, 15, 16, [0.56, 0.74, 0.0]);
                curl(&mut landmarks, 18, 19, 20, [0.60, 0.74, 0.0]);
            }
            _ => {}
        }
        landmarks
    }

    fn hand_at(x: f32, y: f32) -> TrackedHand {
        let mut landmarks = pose("index");
        for point in &mut landmarks {
            point[0] += x - 0.5;
            point[1] += y - 0.5;
        }
        TrackedHand {
            side: HandSide::Right,
            score: 0.95,
            landmarks,
        }
    }

    #[test]
    fn static_poses_match_their_geometry() {
        assert_eq!(
            classify_static(&pose("fist"), 0.42).unwrap().id,
            "closed_fist"
        );
        assert_eq!(
            classify_static(&pose("index"), 0.42).unwrap().id,
            "index_point"
        );
        assert_eq!(classify_static(&pose("v"), 0.42).unwrap().id, "v_sign");
        assert_eq!(
            classify_static(&pose("palm"), 0.42).unwrap().id,
            "open_palm"
        );
        assert_eq!(
            classify_static(&pose("thumb_up"), 0.42).unwrap().id,
            "thumb_up"
        );
        assert_eq!(
            classify_static(&pose("thumb_down"), 0.42).unwrap().id,
            "thumb_down"
        );
        assert_eq!(classify_static(&pose("pinch"), 0.42).unwrap().id, "pinch");
    }

    #[test]
    fn swipe_uses_more_than_one_frame() {
        let mut tracker = MotionTracker::default();
        let mut fired = false;
        for step in 0..8 {
            let events = tracker.push(step * 30, &[hand_at(0.75 - step as f32 * 0.045, 0.5)]);
            fired |= events.contains(&MotionEvent::SwipeLeft);
        }
        assert!(fired);
    }

    #[test]
    fn lost_hand_is_held_briefly() {
        let mut memory = HandMemory::new(200);
        let hand = hand_at(0.5, 0.5);
        assert_eq!(memory.update(1_000, vec![hand]).len(), 1);
        assert_eq!(memory.update(1_200, Vec::new()).len(), 1);
        assert_eq!(memory.update(1_500, Vec::new()).len(), 0);
    }

    #[test]
    fn custom_template_matches_itself() {
        let frame = normalize_landmarks(&pose("v"));
        let gesture = CustomGesture {
            name: "peace".into(),
            threshold: 0.35,
            frames: resample(&vec![frame.clone(); 12], 12),
        };
        let matched = match_custom(&vec![frame; 12], &[gesture]).unwrap();
        assert_eq!(matched.0, "peace");
        assert!(matched.1 > 0.8);
        let other = normalize_landmarks(&pose("fist"));
        assert!(match_custom(
            &vec![other; 12],
            &[CustomGesture {
                name: "peace".into(),
                threshold: 0.2,
                frames: resample(&vec![normalize_landmarks(&pose("v")); 12], 12)
            }]
        )
        .is_none());
    }

    #[test]
    fn gate_requires_duration_and_cooldown() {
        let mut gates = GateSet::default();
        assert_eq!(
            gates.observe("v_sign", true, 200, 600, 0),
            GateResult::Rejected("too_short")
        );
        assert_eq!(
            gates.observe("v_sign", true, 200, 600, 220),
            GateResult::Fired
        );
        assert_eq!(
            gates.observe("v_sign", false, 200, 600, 300),
            GateResult::Idle
        );
        assert_eq!(
            gates.observe("v_sign", true, 0, 600, 400),
            GateResult::Rejected("cooldown")
        );
        assert_eq!(
            gates.observe("v_sign", true, 0, 600, 1_100),
            GateResult::Fired
        );
    }

    #[test]
    fn classify_throughput() {
        let landmarks = pose("palm");
        let start = std::time::Instant::now();
        for _ in 0..5_000 {
            assert!(classify_static(&landmarks, 0.42).is_some());
        }
        assert!(start.elapsed().as_millis() < 1_500);
    }
}
