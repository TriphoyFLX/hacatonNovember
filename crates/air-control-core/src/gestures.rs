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

pub fn pinch_ratio(landmarks: &[[f32; 3]; 21]) -> f32 {
    let scale = distance(landmarks[0], landmarks[9]).max(0.02);
    distance(landmarks[4], landmarks[8]) / scale
}

/// Keeps a pinch through one noisy frame, and lets go only after the fingers open.
pub fn pinch_latched(held: bool, ratio: f32, threshold: f32) -> bool {
    let close = threshold.clamp(0.12, 0.75);
    if held {
        ratio < close * 1.6
    } else {
        ratio < close
    }
}

/// A pinch is the thumb meeting the index fingertip. Both the distance and the
/// gap have to be small: fingers that are merely near each other do not count.
pub fn pinch_active(landmarks: &[[f32; 3]; 21], held: bool, threshold: f32) -> bool {
    pinch_active_gap(landmarks, held, threshold, 0.0)
}

/// `gap_limit` is this person's boundary between a touch and a thumbs-up.
/// Zero keeps the general rule.
pub fn pinch_active_gap(
    landmarks: &[[f32; 3]; 21],
    held: bool,
    threshold: f32,
    gap_limit: f32,
) -> bool {
    let index_open = finger_open(landmarks, 5, 6, 8);
    // While dragging, the tip often reads as curled against the thumb. Do not
    // drop a held pinch for that.
    if !held && index_open < 0.22 {
        return false;
    }
    // A thumbs-up keeps the index folded into the palm. A pinch reaches the thumb
    // with that finger, so a raised thumb cannot click.
    if !held && !index_reaches(landmarks) {
        return false;
    }
    if !held && thumb_way(landmarks) != 0 {
        return false;
    }
    let ratio = pinch_ratio(landmarks);
    let gap = distance(landmarks[4], landmarks[8]) / distance(landmarks[5], landmarks[8]).max(0.02);
    let close = threshold.clamp(0.18, 0.42);
    let gap_close = if gap_limit > 0.0 {
        gap_limit.clamp(0.16, 0.42)
    } else {
        (close * 0.95).clamp(0.18, 0.34)
    };
    if !held {
        let others_down = finger_open(landmarks, 9, 10, 12) < 0.28
            && finger_open(landmarks, 13, 14, 16) < 0.28
            && finger_open(landmarks, 17, 18, 20) < 0.28;
        if others_down && index_open < 0.45 && gap > 0.22 {
            return false;
        }
        let xy = (landmarks[4][0] - landmarks[8][0]).hypot(landmarks[4][1] - landmarks[8][1]);
        let depth = (landmarks[4][2] - landmarks[8][2]).abs();
        if depth > 0.05 && depth > xy * 1.4 {
            return false;
        }
        let to_tip = distance(landmarks[4], landmarks[8]);
        let to_wrist = distance(landmarks[4], landmarks[0]).max(0.02);
        if to_tip > to_wrist * 0.48 {
            return false;
        }
        return ratio < close && gap < gap_close;
    }
    // Held pinches stay sticky: camera noise while moving must not drop the drag.
    ratio < close * 1.75 && gap < gap_close * 1.85
}

/// Image overlap is not a pinch when the metric hand still has the fingers apart.
#[cfg_attr(not(test), allow(dead_code))]
pub fn pinch_active_hand(hand: &TrackedHand, held: bool, threshold: f32) -> bool {
    pinch_active_hand_gap(hand, held, threshold, 0.0)
}

pub fn pinch_active_hand_gap(
    hand: &TrackedHand,
    held: bool,
    threshold: f32,
    gap_limit: f32,
) -> bool {
    let image = pinch_active_gap(&hand.landmarks, held, threshold, gap_limit);
    let Some(world) = hand.world.as_ref() else {
        return image;
    };
    if !image {
        return false;
    }
    if held {
        return true;
    }
    pinch_ratio(world) < threshold.clamp(0.18, 0.42) * 1.2
}

fn pointer_pose(landmarks: &[[f32; 3]; 21]) -> bool {
    let index = finger_open(landmarks, 5, 6, 8);
    let middle = finger_open(landmarks, 9, 10, 12);
    let ring = finger_open(landmarks, 13, 14, 16);
    let pinky = finger_open(landmarks, 17, 18, 20);
    // Only a clearly pointing finger blocks a swipe. A palm with slightly
    // curled fingers must still be able to wave.
    index > 0.34 && middle < 0.28 && ring < 0.28 && pinky < 0.28
}

fn open_fingers(landmarks: &[[f32; 3]; 21]) -> usize {
    let chains = [(5, 6, 8), (9, 10, 12), (13, 14, 16), (17, 18, 20)];
    chains
        .iter()
        .filter(|(mcp, pip, tip)| finger_open(landmarks, *mcp, *pip, *tip) > 0.14)
        .count()
}

/// Fingertip, steadied by the joint so camera noise does not shake the cursor.
pub fn aim_point(landmarks: &[[f32; 3]; 21]) -> (f32, f32) {
    let tip = landmarks[8];
    let joint = landmarks[6];
    (
        tip[0] * 0.72 + joint[0] * 0.28,
        tip[1] * 0.72 + joint[1] * 0.28,
    )
}

/// Image aim with a light depth parallax pull toward the knuckle when the tip
/// is closer to the camera than the MCP (reduces "lean in" drift).
pub fn aim_point_depth(
    landmarks: &[[f32; 3]; 21],
    world: Option<&[[f32; 3]; 21]>,
    strength: f32,
) -> (f32, f32) {
    let base = aim_point(landmarks);
    let strength = strength.clamp(0.0, 1.5);
    if strength <= 0.01 {
        return base;
    }
    let tip_z = landmarks[8][2];
    let mcp_z = landmarks[5][2];
    let mut rel = (tip_z - mcp_z).clamp(-0.15, 0.15);
    if let Some(w) = world {
        // World meters: tip closer than MCP → more negative relative z in many setups.
        let wz = (w[8][2] - w[5][2]).clamp(-0.08, 0.08);
        rel = rel * 0.45 + wz * 0.55;
    }
    // Tip closer (rel < 0) → blend aim toward MCP so closing/leaning does not shove XY.
    let pull = ((-rel) * strength * 1.8).clamp(0.0, 0.35);
    let mcp = (landmarks[5][0], landmarks[5][1]);
    (
        base.0 + (mcp.0 - base.0) * pull,
        base.1 + (mcp.1 - base.1) * pull,
    )
}

/// Cursor speed multiplier from palm size and optional world wrist depth.
/// Closer hand (larger image palm / smaller |z|) yields gain below 1.
pub fn cursor_distance_gain(
    live_scale: f32,
    calib_scale: f32,
    world_wrist_z: Option<f32>,
    strength: f32,
) -> f32 {
    let strength = strength.clamp(0.3, 1.5);
    let mut raw = 1.0_f32;
    if calib_scale > 0.04 && live_scale > 0.02 {
        let ratio = (calib_scale / live_scale).clamp(0.55, 1.85);
        raw = ratio.powf(0.62);
    }
    if let Some(z) = world_wrist_z {
        let dist = z.abs().clamp(0.08, 0.95);
        let z_gain = (0.30 / dist).clamp(0.68, 1.38);
        raw = raw * 0.55 + z_gain * 0.45;
    }
    let blended = 1.0 + (raw - 1.0) * strength;
    blended.clamp(0.55, 1.45)
}

/// Knuckle stays put when the fingertip closes into a pinch.
pub fn pinch_anchor(landmarks: &[[f32; 3]; 21]) -> (f32, f32) {
    (landmarks[5][0], landmarks[5][1])
}

/// Tips collapsed onto one blob: MediaPipe lost the fingers.
pub fn hand_collapsed(landmarks: &[[f32; 3]; 21], calibrated_span: f32) -> bool {
    let span = distance(landmarks[8], landmarks[20]);
    if calibrated_span > 0.04 && span < calibrated_span * 0.35 {
        return true;
    }
    let tips = [4usize, 8, 12, 16, 20];
    let mut farthest: f32 = 0.0;
    for (index, &left) in tips.iter().enumerate() {
        for &right in tips.iter().skip(index + 1) {
            farthest = farthest.max(distance(landmarks[left], landmarks[right]));
        }
    }
    let scale = distance(landmarks[0], landmarks[9]).max(0.02);
    farthest < scale * 0.22
}

fn pointing_for_tap(landmarks: &[[f32; 3]; 21]) -> bool {
    let index = finger_open(landmarks, 5, 6, 8);
    let middle = finger_open(landmarks, 9, 10, 12);
    let ring = finger_open(landmarks, 13, 14, 16);
    let pinky = finger_open(landmarks, 17, 18, 20);
    middle < 0.42 && ring < 0.42 && pinky < 0.42 && index > 0.10
}

fn index_reach(landmarks: &[[f32; 3]; 21]) -> f32 {
    let scale = distance(landmarks[0], landmarks[9]).max(0.02);
    distance(landmarks[8], landmarks[5]) / scale
}

/// Learns how long the pointing finger looks, then treats a small bend as a click.
#[derive(Debug, Clone)]
pub struct IndexTapTracker {
    /// Recent straight length, not the tallest frame. A tall frame used to
    /// latch the mouse button until the 8 second safety release.
    peak: f32,
    prev: f32,
    warm: u8,
    hold: u8,
    /// After a click, ignore the finger until it is straight again.
    cool: bool,
    /// Frames where the index stopped leading. A short blip must not forget the straight length.
    miss: u8,
}

impl Default for IndexTapTracker {
    fn default() -> Self {
        Self {
            peak: 0.0,
            prev: 0.0,
            warm: 0,
            hold: 0,
            cool: false,
            miss: 0,
        }
    }
}

impl IndexTapTracker {
    pub fn reset(&mut self) {
        self.peak = 0.0;
        self.prev = 0.0;
        self.warm = 0;
        self.hold = 0;
        self.cool = false;
        self.miss = 0;
    }

    pub fn press(&mut self, hand: &TrackedHand, held: bool) -> bool {
        if hand_collapsed(&hand.landmarks, 0.0) {
            self.warm = self.warm.saturating_sub(1);
            return false;
        }
        if !held && pinch_active(&hand.landmarks, false, 0.22) {
            self.warm = self.warm.saturating_sub(1);
            return false;
        }
        let leading = index_leads(&hand.landmarks);
        if !leading && self.warm < 4 {
            self.miss = self.miss.saturating_add(1);
            if self.miss >= 6 {
                self.reset();
            }
            return false;
        }
        if leading {
            self.miss = 0;
        } else {
            self.miss = self.miss.saturating_add(1);
            if self.miss >= 8 {
                self.reset();
                return false;
            }
        }
        let reach = index_reach(&hand.landmarks);
        if self.cool {
            if reach + 0.02 >= self.peak * 0.96 {
                self.cool = false;
                self.hold = 0;
            } else {
                self.peak = self.peak * 0.8 + reach * 0.2;
                self.prev = reach;
                return false;
            }
        }
        if self.warm < 4 {
            self.peak = if self.warm == 0 {
                reach
            } else {
                self.peak * 0.5 + reach * 0.5
            };
            self.prev = reach;
            self.warm += 1;
            return false;
        }
        let drop = self.peak - reach;
        let sudden = self.prev - reach;
        let clicking = if held {
            drop > (self.peak * 0.16).max(0.07)
        } else {
            sudden > (self.peak * 0.10).max(0.055) && drop > (self.peak * 0.12).max(0.07)
        };
        if clicking {
            self.hold = self.hold.saturating_add(1);
            if self.hold > 5 && drop < self.peak * 0.30 {
                self.cool = true;
                self.hold = 0;
                self.peak = reach;
                self.prev = reach;
                return false;
            }
        } else {
            self.hold = 0;
            if leading {
                self.peak = self.peak * 0.65 + reach * 0.35;
            }
        }
        self.prev = reach;
        clicking
    }
}

/// A sharp close of an open hand. A slow curl and an index poke do not count.
#[derive(Debug, Default)]
pub struct SqueezeTracker {
    samples: VecDeque<(u64, f32)>,
    cool_until: u64,
}

impl SqueezeTracker {
    pub fn push(&mut self, time_ms: u64, landmarks: &[[f32; 3]; 21]) -> bool {
        let open = hand_openness(landmarks);
        self.samples.push_back((time_ms, open));
        while self
            .samples
            .front()
            .is_some_and(|sample| time_ms.saturating_sub(sample.0) > 280)
        {
            self.samples.pop_front();
        }
        if time_ms < self.cool_until {
            return false;
        }
        let peak = self
            .samples
            .iter()
            .map(|sample| sample.1)
            .fold(0.0_f32, f32::max);
        if peak >= 0.45 && open <= 0.22 && peak - open >= 0.28 {
            self.cool_until = time_ms.saturating_add(800);
            self.samples.clear();
            return true;
        }
        false
    }
}

fn index_leads(landmarks: &[[f32; 3]; 21]) -> bool {
    let index = finger_open(landmarks, 5, 6, 8);
    let middle = finger_open(landmarks, 9, 10, 12);
    index > 0.18 && index + 0.02 >= middle
}

/// Pointing finger for the cursor. An open palm is not this: the other fingers are up too.
fn index_is_cursor(landmarks: &[[f32; 3]; 21]) -> bool {
    let index = finger_open(landmarks, 5, 6, 8);
    let middle = finger_open(landmarks, 9, 10, 12);
    let ring = finger_open(landmarks, 13, 14, 16);
    let pinky = finger_open(landmarks, 17, 18, 20);
    index > 0.22 && middle < 0.45 && ring < 0.45 && pinky < 0.45 && index + 0.04 >= middle
}

/// Open hand, not the pointing finger. A palm wave may scroll; a point may not.
pub fn showing_palm(landmarks: &[[f32; 3]; 21]) -> bool {
    !index_is_cursor(landmarks) && open_fingers(landmarks) >= 3 && hand_openness(landmarks) >= 0.34
}

/// Short index poke: the pointing finger curls a little, thumb stays away.
pub fn index_press_active(hand: &TrackedHand, held: bool) -> bool {
    if hand_collapsed(&hand.landmarks, 0.0) {
        return false;
    }
    if pinch_active(&hand.landmarks, false, 0.42) {
        return false;
    }
    if !pointing_for_tap(&hand.landmarks) {
        return false;
    }
    // Image ratio only. A straight finger in world meters is often below 1.05,
    // and treating that as a poke freezes the cursor for the whole point.
    let reach = index_reach(&hand.landmarks);
    if held {
        reach < 1.28
    } else {
        reach < 1.05
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn classify_static(
    landmarks: &[[f32; 3]; 21],
    pinch_ratio_threshold: f32,
) -> Option<StaticPose> {
    classify_from(
        landmarks,
        landmarks,
        pinch_ratio_threshold,
        0.0,
        None,
        crate::model::PoseGates::default(),
    )
}

/// Prefers the metric hand when MediaPipe provides one, and keeps screen
/// direction (thumb up, the V gap) on the camera landmarks.
pub fn classify_hand(hand: &TrackedHand, pinch_ratio_threshold: f32) -> Option<StaticPose> {
    classify_hand_gap(hand, pinch_ratio_threshold, 0.0)
}

pub fn classify_hand_gap(
    hand: &TrackedHand,
    pinch_ratio_threshold: f32,
    gap_limit: f32,
) -> Option<StaticPose> {
    classify_hand_person(hand, pinch_ratio_threshold, gap_limit, None)
}

/// This person's palm, pointing finger and thumbs-up, as four openness values.
#[derive(Clone, Copy, Debug)]
pub struct RememberedPoses {
    pub palm: [f32; 4],
    pub point: [f32; 4],
    pub thumb: [f32; 4],
}

pub fn classify_hand_person(
    hand: &TrackedHand,
    pinch_ratio_threshold: f32,
    gap_limit: f32,
    remembered: Option<RememberedPoses>,
) -> Option<StaticPose> {
    classify_hand_gated(
        hand,
        pinch_ratio_threshold,
        gap_limit,
        remembered,
        crate::model::PoseGates::default(),
    )
}

pub fn classify_hand_gated(
    hand: &TrackedHand,
    pinch_ratio_threshold: f32,
    gap_limit: f32,
    remembered: Option<RememberedPoses>,
    gates: crate::model::PoseGates,
) -> Option<StaticPose> {
    let shape = hand.world.as_ref().unwrap_or(&hand.landmarks);
    classify_from(
        shape,
        &hand.landmarks,
        pinch_ratio_threshold,
        gap_limit,
        remembered,
        gates,
    )
}

fn classify_from(
    shape: &[[f32; 3]; 21],
    image: &[[f32; 3]; 21],
    pinch_ratio_threshold: f32,
    gap_limit: f32,
    remembered: Option<RememberedPoses>,
    gates: crate::model::PoseGates,
) -> Option<StaticPose> {
    let open = [
        openness(shape, image, 2, 3, 4),
        openness(shape, image, 5, 6, 8),
        openness(shape, image, 9, 10, 12),
        openness(shape, image, 13, 14, 16),
        openness(shape, image, 17, 18, 20),
    ];
    let extend = (gates.extend / gates.distance.max(0.8)).clamp(0.26, 0.42);
    let curl = (gates.curl * gates.distance.max(0.8)).clamp(0.28, 0.50);
    let v_need = (gates.v_spread / gates.distance.max(0.8)).clamp(0.12, 0.34);
    let extended = open.map(|value| value > extend);
    let curled = open.map(|value| value < curl);
    let scale = distance(image[0], image[9]).max(0.02);

    if pinch_active_gap(image, false, pinch_ratio_threshold, gap_limit) {
        let metric = pinch_ratio(shape);
        let limit = pinch_ratio_threshold.clamp(0.18, 0.42) * 1.2;
        if metric < limit {
            let ratio = pinch_ratio(image);
            let confidence = (1.15 - ratio / pinch_ratio_threshold.max(0.05)).clamp(0.55, 0.98);
            return Some(StaticPose {
                id: "pinch",
                confidence,
            });
        }
    }
    if index_is_cursor(image) {
        return Some(StaticPose {
            id: "index_point",
            confidence: clarity(&[open[1]]),
        });
    }
    if let Some(pose) = remembered.and_then(|poses| nearest_pose(&open, image, poses)) {
        return Some(pose);
    }
    if curled.iter().all(|value| *value) {
        let open_hand = hand_openness(shape);
        return Some(StaticPose {
            id: "closed_fist",
            confidence: (0.95 - open_hand * 0.25).clamp(0.8, 0.95),
        });
    }
    if open[1] < curl + 0.08
        && open[2] < curl + 0.08
        && open[3] < curl + 0.08
        && open[4] < curl + 0.08
        && !index_reaches(image)
    {
        match thumb_way(image) {
            1 => {
                return Some(StaticPose {
                    id: "thumb_up",
                    confidence: 0.9,
                });
            }
            -1 => {
                return Some(StaticPose {
                    id: "thumb_down",
                    confidence: 0.9,
                });
            }
            _ => {}
        }
    }
    let spread = distance(image[8], image[12]) / scale;
    if extended[1] && extended[2] && curled[3] && curled[4] && spread > v_need {
        return Some(StaticPose {
            id: "v_sign",
            confidence: clarity(&[open[1], open[2]]),
        });
    }
    if extended[1] && curled[2] && curled[3] && curled[4] {
        return Some(StaticPose {
            id: "index_point",
            confidence: clarity(&[open[1]]),
        });
    }
    if extended[1] && extended[2] && extended[3] && extended[4] {
        return Some(StaticPose {
            id: "open_palm",
            confidence: clarity(&[open[1], open[2], open[3], open[4]]),
        });
    }
    // A real hand is rarely a textbook pose. Accept it when one finger is
    // clearly the odd one out, instead of returning nothing.
    if open[1] > extend + 0.04
        && open[1] > open[2] + 0.10
        && open[1] > open[3] + 0.10
        && open[1] > open[4] + 0.10
        && open[2] < extend + 0.12
    {
        return Some(StaticPose {
            id: "index_point",
            confidence: clarity(&[open[1]]),
        });
    }
    if open[1] > extend - 0.02
        && open[2] > extend - 0.02
        && open[3] < curl + 0.10
        && open[4] < curl + 0.10
        && spread > v_need * 0.85
    {
        return Some(StaticPose {
            id: "v_sign",
            confidence: clarity(&[open[1], open[2]]),
        });
    }
    let raised = [open[1], open[2], open[3], open[4]]
        .iter()
        .filter(|value| **value > extend - 0.04)
        .count();
    let mean = (open[1] + open[2] + open[3] + open[4]) / 4.0;
    if raised >= 3 && mean > extend + 0.04 {
        return Some(StaticPose {
            id: "open_palm",
            confidence: clarity(&[open[1], open[2], open[3], open[4]]),
        });
    }
    None
}

fn nearest_pose(
    open: &[f32; 5],
    image: &[[f32; 3]; 21],
    poses: RememberedPoses,
) -> Option<StaticPose> {
    let now = [open[1], open[2], open[3], open[4]];
    let mut best_name: Option<&'static str> = None;
    let mut best = f32::MAX;
    let mut second = f32::MAX;
    for (name, template) in [
        ("open_palm", poses.palm),
        ("index_point", poses.point),
        ("thumb_up", poses.thumb),
    ] {
        if template.iter().copied().sum::<f32>() < 0.15 {
            continue;
        }
        let dist = now
            .iter()
            .zip(template)
            .map(|(left, right)| (left - right) * (left - right))
            .sum::<f32>()
            .sqrt();
        if dist < best {
            second = best;
            best = dist;
            best_name = Some(name);
        } else if dist < second {
            second = dist;
        }
    }
    let name = best_name?;
    if best > 0.28 || second - best < 0.06 {
        return None;
    }
    let id = if name == "thumb_up" {
        match thumb_way(image) {
            1 => "thumb_up",
            -1 => "thumb_down",
            _ => return None,
        }
    } else {
        name
    };
    Some(StaticPose {
        id,
        confidence: (0.95 - best).clamp(0.7, 0.96),
    })
}

pub fn finger_shape(hand: &TrackedHand) -> [f32; 4] {
    let shape = hand.world.as_ref().unwrap_or(&hand.landmarks);
    [
        openness(shape, &hand.landmarks, 5, 6, 8),
        openness(shape, &hand.landmarks, 9, 10, 12),
        openness(shape, &hand.landmarks, 13, 14, 16),
        openness(shape, &hand.landmarks, 17, 18, 20),
    ]
}

/// 0 is a folded finger, 1 is a straight one. The joint angle still sees a
/// finger that points at the camera, where the flat picture looks folded.
fn openness(
    shape: &[[f32; 3]; 21],
    image: &[[f32; 3]; 21],
    mcp: usize,
    pip: usize,
    tip: usize,
) -> f32 {
    let metric = finger_open(shape, mcp, pip, tip);
    if std::ptr::eq(shape, image) {
        return metric;
    }
    // Prefer the metric hand: the flat picture folds fingers that point at the camera.
    let flat = finger_open(image, mcp, pip, tip);
    metric * 0.82 + flat * 0.18
}

/// +1 when the thumb stands clearly above the other fingertips, −1 when it
/// stands clearly below. A pinch keeps the thumb on the index tip, so it is 0.
pub fn thumb_way(landmarks: &[[f32; 3]; 21]) -> i8 {
    let scale = distance(landmarks[0], landmarks[9]).max(0.02);
    let margin = scale * 0.12;
    let tip_y = landmarks[4][1];
    let knuckle_y = landmarks[2][1];
    let above = [8usize, 12, 16, 20]
        .iter()
        .all(|&index| tip_y + margin < landmarks[index][1]);
    let below = [8usize, 12, 16, 20]
        .iter()
        .all(|&index| tip_y - margin > landmarks[index][1]);
    if tip_y + margin < knuckle_y && above {
        1
    } else if tip_y - margin > knuckle_y && below {
        -1
    } else {
        0
    }
}

/// The index is out toward the thumb, not folded back into the fist.
fn index_reaches(landmarks: &[[f32; 3]; 21]) -> bool {
    let wrist = landmarks[0];
    let mcp = landmarks[5];
    let pip = landmarks[6];
    let tip = landmarks[8];
    let pip_out = distance(pip, wrist) > distance(mcp, wrist) * 0.92;
    let tip_out = distance(tip, wrist) > distance(pip, wrist).max(0.02) * 1.05;
    pip_out && tip_out
}

pub fn finger_open(landmarks: &[[f32; 3]; 21], mcp: usize, pip: usize, tip: usize) -> f32 {
    let scale = distance(landmarks[0], landmarks[9]).max(1e-4);
    let ratio = ((extension(landmarks, tip, pip) - 1.0) / 0.45).clamp(0.0, 1.0);
    let angled = hinge(landmarks[mcp], landmarks[pip], landmarks[tip], scale * 0.08)
        .map(|radians| ((radians - 1.15) / (std::f32::consts::PI - 1.15)).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    ratio.max(angled)
}

/// 0 = straight finger, 1 = curled shut. Scale-free via joint angles.
pub fn finger_curl(landmarks: &[[f32; 3]; 21], joints: [usize; 4]) -> f32 {
    let [mcp, pip, dip, tip] = joints.map(|index| landmarks[index]);
    let scale = distance(landmarks[0], landmarks[9]).max(1e-4);
    let a = hinge(mcp, pip, dip, scale * 0.05).unwrap_or(std::f32::consts::PI * 0.85);
    let b = hinge(pip, dip, tip, scale * 0.05).unwrap_or(std::f32::consts::PI * 0.85);
    (1.0 - (a + b) / std::f32::consts::PI).clamp(0.0, 1.0)
}

/// Whole-hand openness invariant to camera distance: tip reach / wrist→middle MCP.
/// Fist ≈ 0, open palm ≈ 1.
pub fn hand_openness(landmarks: &[[f32; 3]; 21]) -> f32 {
    let hand_size = distance(landmarks[9], landmarks[0]).max(1e-6);
    let tips = [4usize, 8, 12, 16, 20];
    let avg = tips
        .iter()
        .map(|&index| distance(landmarks[index], landmarks[0]))
        .sum::<f32>()
        / tips.len() as f32;
    let ratio = avg / hand_size;
    ((ratio - 1.35) / 1.0).clamp(0.0, 1.0)
}

/// Binary gesture latch with hysteresis: enter / exit thresholds stop flicker.
#[derive(Debug, Default, Clone, Copy)]
pub struct BinaryLatch {
    on: bool,
}

impl BinaryLatch {
    /// True while "high" state is held. Enter above `enter`, leave below `exit`.
    pub fn high(&mut self, value: f32, enter: f32, exit: f32) -> bool {
        if self.on {
            if value < exit {
                self.on = false;
            }
        } else if value > enter {
            self.on = true;
        }
        self.on
    }

    /// True while "low" state is held (fist). Enter below `enter`, leave above `exit`.
    pub fn low(&mut self, value: f32, enter: f32, exit: f32) -> bool {
        if self.on {
            if value > exit {
                self.on = false;
            }
        } else if value < enter {
            self.on = true;
        }
        self.on
    }

    pub fn clear(&mut self) {
        self.on = false;
    }

    pub fn is_on(&self) -> bool {
        self.on
    }
}

/// Keeps MediaPipe hand slots stable across frames by nearest-palm matching.
#[derive(Debug, Default)]
pub struct HandLinker {
    next_id: u64,
    prev: Vec<(u64, f32, f32)>,
}

impl HandLinker {
    pub fn link(&mut self, hands: Vec<TrackedHand>) -> Vec<TrackedHand> {
        if hands.is_empty() {
            self.prev.clear();
            return hands;
        }
        let mut available: Vec<Option<TrackedHand>> = hands.into_iter().map(Some).collect();
        let mut linked = Vec::with_capacity(available.len());

        // Prefer previous order so stage / glide slots do not swap on overlap.
        for &(id, px, py) in &self.prev {
            let mut best: Option<(usize, f32)> = None;
            for (index, slot) in available.iter().enumerate() {
                let Some(hand) = slot else { continue };
                let (x, y) = palm_xy(hand);
                let dist = (x - px).hypot(y - py);
                if dist < 0.28 && best.is_none_or(|(_, best_d)| dist < best_d) {
                    best = Some((index, dist));
                }
            }
            if let Some((index, _)) = best {
                let mut hand = available[index].take().unwrap();
                hand.track_id = id;
                linked.push(hand);
            }
        }

        for slot in available.into_iter().flatten() {
            let mut hand = slot;
            self.next_id = self.next_id.saturating_add(1).max(1);
            hand.track_id = self.next_id;
            linked.push(hand);
        }

        self.prev = linked
            .iter()
            .map(|hand| {
                let (x, y) = palm_xy(hand);
                (hand.track_id, x, y)
            })
            .collect();
        linked
    }
}

fn palm_xy(hand: &TrackedHand) -> (f32, f32) {
    let x = (hand.landmarks[0][0] + hand.landmarks[5][0] + hand.landmarks[17][0]) / 3.0;
    let y = (hand.landmarks[0][1] + hand.landmarks[5][1] + hand.landmarks[17][1]) / 3.0;
    (x, y)
}

/// World-space tip speed in m/s when world landmarks exist, else image units / s.
pub fn tip_speed(
    hand: &TrackedHand,
    landmark: usize,
    previous: Option<([f32; 3], u64)>,
    now_ms: u64,
) -> f32 {
    let point = tip_point(hand, landmark);
    let Some((prev, at)) = previous else {
        return 0.0;
    };
    let dt = ((now_ms.saturating_sub(at) as f32) / 1000.0).clamp(1.0 / 60.0, 0.12);
    distance(point, prev) / dt
}

pub fn tip_point(hand: &TrackedHand, landmark: usize) -> [f32; 3] {
    hand.world
        .as_ref()
        .map(|world| world[landmark])
        .unwrap_or(hand.landmarks[landmark])
}

fn hinge(mcp: [f32; 3], pip: [f32; 3], tip: [f32; 3], min_bone: f32) -> Option<f32> {
    let toward_mcp = [mcp[0] - pip[0], mcp[1] - pip[1], mcp[2] - pip[2]];
    let toward_tip = [tip[0] - pip[0], tip[1] - pip[1], tip[2] - pip[2]];
    let left = (toward_mcp[0] * toward_mcp[0]
        + toward_mcp[1] * toward_mcp[1]
        + toward_mcp[2] * toward_mcp[2])
        .sqrt();
    let right = (toward_tip[0] * toward_tip[0]
        + toward_tip[1] * toward_tip[1]
        + toward_tip[2] * toward_tip[2])
        .sqrt();
    if left < min_bone || right < min_bone {
        return None;
    }
    let dot = toward_mcp[0] * toward_tip[0]
        + toward_mcp[1] * toward_tip[1]
        + toward_mcp[2] * toward_tip[2];
    let cosine = (dot / (left * right)).clamp(-1.0, 1.0);
    Some(cosine.acos())
}

fn extension(landmarks: &[[f32; 3]; 21], tip: usize, pip: usize) -> f32 {
    let wrist = landmarks[0];
    let tip_distance = distance(landmarks[tip], wrist);
    let pip_distance = distance(landmarks[pip], wrist).max(1e-4);
    tip_distance / pip_distance
}

fn clarity(values: &[f32]) -> f32 {
    let mean = values.iter().sum::<f32>() / values.len().max(1) as f32;
    (0.62 + mean * 0.34).clamp(0.62, 0.97)
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

/// LB_Keogh lower bound — skip full DTW when the envelope already exceeds the threshold.
pub fn lb_keogh(sample: &[Vec<f32>], template: &[Vec<f32>], radius: usize) -> f32 {
    let n = sample.len().min(template.len());
    if n == 0 {
        return f32::MAX;
    }
    let mut total = 0.0;
    for i in 0..n {
        let lo = i.saturating_sub(radius);
        let hi = (i + radius + 1).min(n);
        let dims = sample[i].len().min(template[0].len());
        if dims == 0 {
            continue;
        }
        let mut cost = 0.0;
        for d in 0..dims {
            let mut mn = f32::MAX;
            let mut mx = f32::MIN;
            for j in lo..hi {
                let v = template[j][d];
                mn = mn.min(v);
                mx = mx.max(v);
            }
            let v = sample[i][d];
            if v > mx {
                let delta = v - mx;
                cost += delta * delta;
            } else if v < mn {
                let delta = mn - v;
                cost += delta * delta;
            }
        }
        total += (cost / dims as f32).sqrt();
    }
    total / n as f32
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

/// Ambient-style adaptive threshold from accepted / rejected DTW distances.
#[derive(Debug, Default, Clone)]
pub struct CustomMatchStats {
    accepted: VecDeque<f32>,
    rejected: VecDeque<f32>,
}

impl CustomMatchStats {
    pub fn note(&mut self, distance: f32, accepted: bool) {
        if !distance.is_finite() {
            return;
        }
        let bucket = if accepted {
            &mut self.accepted
        } else {
            &mut self.rejected
        };
        bucket.push_back(distance);
        while bucket.len() > 40 {
            bucket.pop_front();
        }
    }

    pub fn threshold_for(&self, base: f32) -> f32 {
        let base = base.clamp(0.05, 2.0);
        let accept_mean = mean_queue(&self.accepted);
        let reject_mean = mean_queue(&self.rejected);
        let mut adjusted = base;
        if let (Some(ok), Some(bad)) = (accept_mean, reject_mean) {
            // Keep a gap between typical accepts and rejects.
            let mid = (ok + bad) * 0.5;
            adjusted = (0.65 * base + 0.35 * mid).clamp(base * 0.75, base * 1.35);
        } else if let Some(ok) = accept_mean {
            adjusted = (0.8 * base + 0.2 * (ok * 1.15)).clamp(base * 0.8, base * 1.25);
        }
        adjusted
    }
}

fn mean_queue(values: &VecDeque<f32>) -> Option<f32> {
    if values.len() < 6 {
        return None;
    }
    Some(values.iter().sum::<f32>() / values.len() as f32)
}

fn gesture_templates(gesture: &CustomGesture) -> Vec<&[Vec<f32>]> {
    let mut out = Vec::with_capacity(1 + gesture.templates.len());
    out.push(gesture.frames.as_slice());
    for template in &gesture.templates {
        if template.len() >= 8 {
            out.push(template.as_slice());
        }
    }
    out
}

pub fn match_custom(
    history: &[Vec<f32>],
    gestures: &[CustomGesture],
    stats: Option<&mut CustomMatchStats>,
) -> Option<(String, f32)> {
    let sample = resample(history, 12);
    if sample.len() < 12 {
        return None;
    }
    let mut best: Option<(String, f32, f32)> = None;
    let mut nearest_reject: Option<(f32, f32)> = None;
    for gesture in gestures {
        let threshold = stats
            .as_ref()
            .map(|s| s.threshold_for(gesture.threshold))
            .unwrap_or(gesture.threshold);
        for template in gesture_templates(gesture) {
            let bound = lb_keogh(&sample, template, 2);
            if bound > threshold {
                nearest_reject = Some(match nearest_reject {
                    Some((d, t)) if bound < d => (bound, t),
                    Some(other) => other,
                    None => (bound, threshold),
                });
                continue;
            }
            let distance = dtw(&sample, template);
            if distance <= threshold {
                let confidence = (1.0 - distance / threshold.max(0.05)).clamp(0.0, 0.98);
                if best
                    .as_ref()
                    .is_none_or(|(_, score, _)| confidence > *score)
                {
                    best = Some((gesture.name.clone(), confidence, distance));
                }
            } else {
                nearest_reject = Some(match nearest_reject {
                    Some((d, t)) if distance < d => (distance, t),
                    Some(other) => other,
                    None => (distance, threshold),
                });
            }
        }
    }
    if let Some(stats) = stats {
        if let Some((_, _, distance)) = &best {
            stats.note(*distance, true);
        } else if let Some((distance, _)) = nearest_reject {
            stats.note(distance, false);
        }
    }
    best.map(|(name, score, _)| (name, score))
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
        if index_is_cursor(&primary.landmarks) || hands.iter().any(|hand| pointer_pose(&hand.landmarks))
        {
            self.samples.clear();
            return Vec::new();
        }
        if !showing_palm(&primary.landmarks) {
            return Vec::new();
        }
        let palm = primary.landmarks;
        // Mostly the middle knuckle, so a wave of the palm counts and a finger twitch does not.
        let wrist = [
            palm[0][0] * 0.35 + palm[9][0] * 0.65,
            palm[0][1] * 0.35 + palm[9][1] * 0.65,
            palm[0][2],
        ];
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
            .filter(|sample| time_ms.saturating_sub(sample.time_ms) <= 560)
            .cloned()
            .collect();
        if window.len() < 3 {
            return Vec::new();
        }
        let mut events = Vec::new();
        if let (Some(start), Some(end)) = (
            window.iter().find(|sample| sample.distance.is_some()),
            window.last(),
        ) {
            if let (Some(a), Some(b)) = (start.distance, end.distance) {
                let change = (b - a) / a.max(0.05);
                let distances: Vec<f32> = window.iter().filter_map(|sample| sample.distance).collect();
                let monotonic = distances.windows(2).filter(|pair| (pair[1] - pair[0]).abs() > 0.004).all(|pair| {
                    if change > 0.0 {
                        pair[1] + 0.01 >= pair[0]
                    } else {
                        pair[1] <= pair[0] + 0.01
                    }
                });
                if monotonic && change > 0.2 {
                    events.push(MotionEvent::ZoomIn);
                } else if monotonic && change < -0.2 {
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
            let start = &window[0];
            let end = window.last().unwrap();
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let dt = (end.time_ms.saturating_sub(start.time_ms) as f32 / 1000.0).max(0.05);
            let straight = dx.abs() > dy.abs() * 1.25 || dy.abs() > dx.abs() * 1.25;
            let mut path = 0.0;
            let mut forward = 0;
            let mut steps = 0;
            for pair in window.windows(2) {
                let step_x = pair[1].x - pair[0].x;
                let step_y = pair[1].y - pair[0].y;
                let step = step_x.hypot(step_y);
                path += step;
                if step > 0.004 {
                    steps += 1;
                    if step_x * dx + step_y * dy > 0.0 {
                        forward += 1;
                    }
                }
            }
            let net = dx.hypot(dy);
            let direct = path > 0.0 && net / path >= 0.55 && (steps < 3 || forward * 3 >= steps * 2);
            if direct && straight && dx.abs() > 0.08 && dx.abs() / dt > 0.16 && dx.abs() > dy.abs() {
                events.push(if dx > 0.0 {
                    MotionEvent::SwipeRight
                } else {
                    MotionEvent::SwipeLeft
                });
            } else if direct
                && straight
                && dy.abs() > 0.08
                && dy.abs() / dt > 0.16
                && dy.abs() > dx.abs()
            {
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

    /// The pose was recognized but the command was refused. Try again while it is still held.
    pub fn rearm(&mut self, id: &str) {
        if let Some(state) = self.states.get_mut(id) {
            state.latched = false;
            state.last_fire = None;
        }
    }
}

pub fn select_hands(
    hands: &[TrackedHand],
    preference: crate::model::PrimaryHand,
    anchor: Option<(f32, f32)>,
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
    if let Some((x, y)) = anchor {
        if let Some(index) = ordered.iter().position(|hand| {
            (hand.landmarks[0][0] - x).hypot(hand.landmarks[0][1] - y) < 0.18
        }) {
            if index != 0 {
                ordered.swap(0, index);
            }
        }
    }
    ordered
}

/// Re-associates hands to previous palm slots and drops teleport jumps.
pub fn steady_hands(
    previous: &[(f32, f32)],
    hands: Vec<TrackedHand>,
    dt_ms: u64,
) -> Vec<TrackedHand> {
    if previous.is_empty() || dt_ms > 140 || hands.is_empty() {
        return hands;
    }
    let mut available: Vec<Option<TrackedHand>> = hands.into_iter().map(Some).collect();
    let mut ordered = Vec::with_capacity(available.len());
    for &(px, py) in previous {
        let mut best: Option<(usize, f32)> = None;
        for (index, slot) in available.iter().enumerate() {
            let Some(hand) = slot else { continue };
            let (x, y) = palm_xy(hand);
            let dist = (x - px).hypot(y - py);
            if dist < 0.32 && best.is_none_or(|(_, best_d)| dist < best_d) {
                best = Some((index, dist));
            }
        }
        if let Some((index, _)) = best {
            ordered.push(available[index].take().unwrap());
        }
    }
    for hand in available.into_iter().flatten() {
        ordered.push(hand);
    }
    ordered
}

/// A command pose has to agree with itself for a few frames. One noisy frame
/// no longer changes the action. Pinch still settles on the second frame.
#[derive(Debug, Default, Clone)]
pub struct PoseHold {
    name: Option<&'static str>,
    run: u8,
    miss: u8,
    current: Option<StaticPose>,
}

impl PoseHold {
    pub fn settle(&mut self, next: Option<StaticPose>) -> Option<StaticPose> {
        if next.is_none() {
            if self.current.is_some() && self.miss < 2 {
                self.miss = self.miss.saturating_add(1);
                return self.current.clone();
            }
            self.miss = 0;
            self.name = None;
            self.run = 0;
            self.current = None;
            return None;
        }
        self.miss = 0;
        let name = next.as_ref().map(|pose| pose.id);
        if name == self.name {
            self.run = self.run.saturating_add(1);
        } else {
            self.name = name;
            self.run = 1;
        }
        let needed: u8 = 2;
        if self.run >= needed {
            self.current = next;
        }
        self.current.clone()
    }

    pub fn confirmed(&self) -> bool {
        match (&self.current, self.name) {
            (Some(pose), Some(name)) => pose.id == name && self.run >= 2,
            _ => false,
        }
    }
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
            "index_dip" => {
                curl(&mut landmarks, 3, 4, 4, [0.46, 0.74, 0.0]);
                landmarks[6] = [0.42, 0.58, 0.0];
                landmarks[7] = [0.43, 0.54, 0.0];
                landmarks[8] = [0.44, 0.50, 0.0];
                curl(&mut landmarks, 10, 11, 12, [0.50, 0.74, 0.0]);
                curl(&mut landmarks, 14, 15, 16, [0.56, 0.74, 0.0]);
                curl(&mut landmarks, 18, 19, 20, [0.60, 0.74, 0.0]);
            }
            "blob" => {
                for index in 1..21 {
                    landmarks[index] = [0.50, 0.70, 0.0];
                }
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
        shifted("index", x, y)
    }

    fn palm_at(x: f32, y: f32) -> TrackedHand {
        shifted("palm", x, y)
    }

    fn shifted(kind: &str, x: f32, y: f32) -> TrackedHand {
        let mut landmarks = pose(kind);
        for point in &mut landmarks {
            point[0] += x - 0.5;
            point[1] += y - 0.5;
        }
        TrackedHand {
            track_id: 0,
            side: HandSide::Right,
            score: 0.95,
            landmarks,
            world: None,
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
        assert!(pinch_active(&pose("pinch"), false, 0.5));
        assert!(!pinch_active(&pose("index"), false, 0.5));
        assert!(pinch_latched(false, 0.30, 0.42));
        assert!(!pinch_latched(false, 0.50, 0.42));
        assert!(pinch_latched(true, 0.50, 0.42));
        assert!(!pinch_latched(true, 0.80, 0.42));
    }

    #[test]
    fn metric_hand_keeps_a_point_the_picture_flattens() {
        let image = pose("fist");
        let mut world = pose("index");
        for point in &mut world {
            point[0] = (point[0] - 0.5) * 0.25;
            point[1] = (point[1] - 0.5) * 0.25;
        }
        let hand = TrackedHand {
            track_id: 0,
            side: HandSide::Right,
            score: 0.95,
            landmarks: image,
            world: Some(world),
        };
        assert_eq!(classify_hand(&hand, 0.42).unwrap().id, "index_point");
    }

    #[test]
    fn separated_fingertips_are_not_a_pinch() {
        let mut landmarks = pose("pinch");
        landmarks[4][2] = 0.25;
        landmarks[8][2] = -0.25;
        assert!(!pinch_active(&landmarks, false, 0.5));
    }

    #[test]
    fn a_remembered_point_wins_over_a_messy_hand() {
        let point = pose("index");
        let hand = TrackedHand {
            track_id: 0,
            side: HandSide::Right,
            score: 0.95,
            landmarks: point,
            world: None,
        };
        let shape = finger_shape(&hand);
        let remembered = RememberedPoses {
            palm: [0.9, 0.9, 0.9, 0.9],
            point: shape,
            thumb: [0.05, 0.05, 0.05, 0.05],
        };
        assert_eq!(
            classify_hand_person(&hand, 0.42, 0.0, Some(remembered))
                .unwrap()
                .id,
            "index_point"
        );
    }

    #[test]
    fn one_empty_frame_keeps_the_pose() {
        let mut hold = PoseHold::default();
        let point = classify_static(&pose("index"), 0.42);
        hold.settle(point.clone());
        hold.settle(point);
        assert_eq!(hold.settle(None).unwrap().id, "index_point");
        assert_eq!(hold.settle(None).unwrap().id, "index_point");
        assert!(hold.settle(None).is_none());
    }

    #[test]
    fn a_thumb_up_is_not_a_pinch() {
        assert!(!pinch_active(&pose("thumb_up"), false, 0.42));
        assert!(!pinch_active(&pose("thumb_down"), false, 0.42));
        assert_eq!(thumb_way(&pose("thumb_up")), 1);
        assert_eq!(thumb_way(&pose("thumb_down")), -1);
        assert_eq!(thumb_way(&pose("pinch")), 0);
        assert_eq!(classify_static(&pose("pinch"), 0.42).unwrap().id, "pinch");
    }

    #[test]
    fn a_sideways_thumb_does_not_change_the_volume() {
        let mut landmarks = pose("thumb_up");
        landmarks[2] = [0.40, 0.68, 0.0];
        landmarks[3] = [0.30, 0.68, 0.0];
        landmarks[4] = [0.18, 0.68, 0.0];
        let pose = classify_static(&landmarks, 0.42);
        assert!(pose.is_none_or(|item| item.id != "thumb_up" && item.id != "thumb_down"));
    }

    #[test]
    fn one_noisy_frame_does_not_change_the_action() {
        let mut hold = PoseHold::default();
        let point = classify_static(&pose("index"), 0.42);
        assert!(hold.settle(point.clone()).is_none());
        assert_eq!(hold.settle(point).unwrap().id, "index_point");
        let fist = classify_static(&pose("fist"), 0.42);
        assert_eq!(hold.settle(fist.clone()).unwrap().id, "index_point");
        assert_eq!(hold.settle(fist).unwrap().id, "closed_fist");
    }

    #[test]
    fn a_wiggle_is_not_a_swipe() {
        let mut tracker = MotionTracker::default();
        for step in 0..8 {
            let x = if step % 2 == 0 { 0.35 } else { 0.62 };
            let events = tracker.push(step * 30, &[palm_at(x, 0.5)]);
            assert!(
                events.iter().all(|event| !matches!(
                    event,
                    MotionEvent::SwipeLeft
                        | MotionEvent::SwipeRight
                        | MotionEvent::SwipeUp
                        | MotionEvent::SwipeDown
                )),
                "a wobble must not scroll: {events:?}"
            );
        }
    }

    #[test]
    fn swipe_uses_more_than_one_frame() {
        let mut tracker = MotionTracker::default();
        let mut fired = false;
        for step in 0..8 {
            let events = tracker.push(step * 30, &[palm_at(0.75 - step as f32 * 0.045, 0.5)]);
            fired |= events.contains(&MotionEvent::SwipeLeft);
        }
        assert!(fired);

        let mut tracker = MotionTracker::default();
        for step in 0..8 {
            let events = tracker.push(step * 30, &[hand_at(0.75 - step as f32 * 0.045, 0.5)]);
            assert!(
                !events.iter().any(|event| matches!(
                    event,
                    MotionEvent::SwipeLeft
                        | MotionEvent::SwipeRight
                        | MotionEvent::SwipeUp
                        | MotionEvent::SwipeDown
                )),
                "pointing must not scroll: {events:?}"
            );
        }
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
    fn index_press_follows_a_short_dip() {
        let straight = TrackedHand {
            track_id: 0,
            side: HandSide::Right,
            score: 0.95,
            landmarks: pose("index"),
            world: None,
        };
        let dipped = TrackedHand {
            track_id: 0,
            side: HandSide::Right,
            score: 0.95,
            landmarks: pose("index_dip"),
            world: None,
        };
        assert!(!index_press_active(&straight, false));
        assert!(index_press_active(&dipped, false));
        assert!(index_press_active(&dipped, true));
        assert!(!index_press_active(&straight, true));
        let mut world = pose("index");
        world[8] = [0.42, 0.55, 0.0];
        assert!(!index_press_active(
            &TrackedHand {
                track_id: 0,
                side: HandSide::Right,
                score: 0.95,
                landmarks: pose("index"),
                world: Some(world),
            },
            false
        ));
        assert!(!index_press_active(
            &TrackedHand {
                track_id: 0,
                side: HandSide::Right,
                score: 0.95,
                landmarks: pose("pinch"),
                world: None,
            },
            false
        ));
    }

    #[test]
    fn small_bend_from_the_current_point_is_a_click() {
        let straight = TrackedHand {
            track_id: 0,
            side: HandSide::Right,
            score: 0.95,
            landmarks: pose("index"),
            world: None,
        };
        let mut bent_marks = pose("index");
        bent_marks[7] = [0.415, 0.50, 0.0];
        bent_marks[8] = [0.41, 0.44, 0.0];
        let bent = TrackedHand {
            track_id: 0,
            side: HandSide::Right,
            score: 0.95,
            landmarks: bent_marks,
            world: None,
        };
        let mut tracker = IndexTapTracker::default();
        for _ in 0..5 {
            assert!(!tracker.press(&straight, false));
        }
        assert!(tracker.press(&bent, false));
        assert!(tracker.press(&bent, true));
        assert!(!tracker.press(&straight, false));
    }

    #[test]
    fn a_sharp_squeeze_clears_and_a_poke_does_not() {
        let mut tracker = SqueezeTracker::default();
        let open = pose("palm");
        let shut = pose("fist");
        assert!(!tracker.push(0, &open));
        assert!(!tracker.push(40, &open));
        assert!(tracker.push(80, &shut));
        assert!(!tracker.push(120, &shut));
        let mut poke = SqueezeTracker::default();
        let point = pose("index");
        assert!(!poke.push(0, &point));
        assert!(!poke.push(40, &point));
        assert!(!poke.push(80, &point));
    }

    #[test]
    fn collapsed_tips_are_a_broken_frame() {
        let blob = pose("blob");
        assert!(hand_collapsed(&blob, 0.20));
        assert!(hand_collapsed(&blob, 0.0));
        assert!(!hand_collapsed(&pose("index"), 0.20));
        assert!(!index_press_active(
            &TrackedHand {
                track_id: 0,
                side: HandSide::Right,
                score: 0.95,
                landmarks: blob,
                world: None,
            },
            false
        ));
    }

    #[test]
    fn hand_openness_is_scale_invariant() {
        let near = pose("palm");
        let mut far = near;
        for point in &mut far {
            point[0] = 0.5 + (point[0] - 0.5) * 0.45;
            point[1] = 0.5 + (point[1] - 0.5) * 0.45;
        }
        let a = hand_openness(&near);
        let b = hand_openness(&far);
        assert!((a - b).abs() < 0.05, "near={a} far={b}");
        assert!(a > 0.55, "open palm should score high, got {a}");
        assert!(hand_openness(&pose("fist")) < 0.35);
    }

    #[test]
    fn linker_keeps_ids_when_array_order_swaps() {
        let mut linker = HandLinker::default();
        let left = palm_at(0.30, 0.50);
        let right = palm_at(0.70, 0.50);
        let first = linker.link(vec![left.clone(), right.clone()]);
        assert_eq!(first[0].track_id, 1);
        assert_eq!(first[1].track_id, 2);
        let swapped = linker.link(vec![right, left]);
        assert_eq!(swapped[0].track_id, 1);
        assert_eq!(swapped[1].track_id, 2);
        assert!(swapped[0].landmarks[0][0] < swapped[1].landmarks[0][0]);
    }

    #[test]
    fn binary_latch_has_hysteresis() {
        let mut latch = BinaryLatch::default();
        assert!(!latch.low(0.40, 0.25, 0.35));
        assert!(latch.low(0.20, 0.25, 0.35));
        assert!(latch.low(0.30, 0.25, 0.35));
        assert!(!latch.low(0.40, 0.25, 0.35));
    }

    #[test]
    fn custom_template_matches_itself() {
        let frame = normalize_landmarks(&pose("v"));
        let gesture = CustomGesture {
            name: "peace".into(),
            threshold: 0.35,
            frames: resample(&vec![frame.clone(); 12], 12),
            templates: Vec::new(),
        };
        let matched = match_custom(&vec![frame; 12], &[gesture], None).unwrap();
        assert_eq!(matched.0, "peace");
        assert!(matched.1 > 0.8);
        let other = normalize_landmarks(&pose("fist"));
        assert!(match_custom(
            &vec![other; 12],
            &[CustomGesture {
                name: "peace".into(),
                threshold: 0.2,
                frames: resample(&vec![normalize_landmarks(&pose("v")); 12], 12),
                templates: Vec::new(),
            }],
            None
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

    #[test]
    fn nearer_palm_lowers_cursor_gain() {
        let far = cursor_distance_gain(0.08, 0.16, None, 1.0);
        let near = cursor_distance_gain(0.28, 0.16, None, 1.0);
        assert!(near < far, "near={near} far={far}");
        assert!(near < 1.0);
        assert!(far > 1.0);
    }

    #[test]
    fn depth_aim_pulls_toward_mcp_when_tip_closer() {
        let mut landmarks = pose("index");
        landmarks[8][2] = landmarks[5][2] - 0.08;
        let base = aim_point(&landmarks);
        let depth = aim_point_depth(&landmarks, None, 1.0);
        let mcp = (landmarks[5][0], landmarks[5][1]);
        let base_dist = (base.0 - mcp.0).hypot(base.1 - mcp.1);
        let depth_dist = (depth.0 - mcp.0).hypot(depth.1 - mcp.1);
        assert!(depth_dist < base_dist + 1e-4);
    }
}
