//! Absolute cursor mapping.
//!
//! A critically damped follow keeps the cursor on the fingertip without
//! copying camera noise. A pinch stays where it started, so closing the
//! fingers clicks instead of dragging.

use crate::model::{NormRect, ScreenRect, SensitivityProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEvent {
    Move(i32, i32),
    PressLeft,
    ReleaseLeft,
    ClickLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerMode {
    Idle,
    Hover,
    Armed,
    Clicked,
    Dragging,
}

#[derive(Debug, Clone)]
pub struct PointerInput {
    pub now_ms: u64,
    pub tracking: bool,
    pub point: Option<(f32, f32)>,
    pub pinch: bool,
    pub click_allowed: bool,
    /// Stable hand point. While pinching, the cursor follows this instead of the fingertip.
    pub anchor: Option<(f32, f32)>,
    pub screen: ScreenRect,
    pub range: NormRect,
    pub speed: f32,
    pub acceleration: f32,
    pub deadzone: f32,
    pub smoothness: f32,
    pub profile: SensitivityProfile,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointerUpdate {
    pub events: Vec<PointerEvent>,
    pub norm: Option<(f32, f32)>,
    pub double_pinch: bool,
    /// Where the first click landed, so a second pinch can hit the same pixel.
    pub repeat_at: Option<(i32, i32)>,
    /// The second pinch arrived while Windows still treats it as one double click.
    pub quick_pair: bool,
    /// Recent cursor travel in pixels. A moving hand should not fire extra commands.
    pub travel: f32,
    /// True when a held press was released without turning into a drag.
    pub tap: bool,
}

#[derive(Debug)]
pub struct PointerController {
    mode: PointerMode,
    pinch_started: Option<u64>,
    origin: Option<(i32, i32)>,
    last_sent: Option<(i32, i32)>,
    last_click: Option<u64>,
    last_click_at: Option<(i32, i32)>,
    quick_pair: bool,
    gap_started: Option<u64>,
    /// First moment the pinch looked open while still dragging/holding.
    pinch_lost_at: Option<u64>,
    last_aim: Option<(f32, f32)>,
    pinch_base: Option<(f32, f32)>,
    drag_from: Option<(f32, f32)>,
    follow_x: Option<f32>,
    follow_y: Option<f32>,
    recent_travel: f32,
}

impl Default for PointerController {
    fn default() -> Self {
        Self {
            mode: PointerMode::Idle,
            pinch_started: None,
            origin: None,
            last_sent: None,
            last_click: None,
            last_click_at: None,
            quick_pair: false,
            gap_started: None,
            pinch_lost_at: None,
            last_aim: None,
            pinch_base: None,
            drag_from: None,
            follow_x: None,
            follow_y: None,
            recent_travel: 0.0,
        }
    }
}

impl PointerController {
    pub fn update(&mut self, input: &PointerInput) -> PointerUpdate {
        let mut events = Vec::new();
        // One noisy frame must not drop a drag the user is still holding.
        const HOLD_GRACE_MS: u64 = 120;
        const DRAG_PX: f32 = 48.0;

        let holding = matches!(
            self.mode,
            PointerMode::Clicked | PointerMode::Dragging | PointerMode::Armed
        );
        let tracking_now = input.tracking && input.point.is_some();

        if !tracking_now {
            let mut double_pinch = false;
            let mut tap = false;
            if holding {
                if self.pinch_lost_at.is_none() {
                    self.pinch_lost_at = Some(input.now_ms);
                }
                let lost_for = input
                    .now_ms
                    .saturating_sub(self.pinch_lost_at.unwrap_or(input.now_ms));
                if lost_for < HOLD_GRACE_MS {
                    // Keep the button down and freeze the last cursor pixel.
                    self.recent_travel *= 0.5;
                    return PointerUpdate {
                        events,
                        norm: None,
                        double_pinch: false,
                        repeat_at: self.last_click_at,
                        quick_pair: self.quick_pair,
                        travel: self.recent_travel,
                        tap: false,
                    };
                }
            }
            if self.mode == PointerMode::Dragging {
                events.push(PointerEvent::ReleaseLeft);
                self.mode = PointerMode::Hover;
                self.clear_pinch();
            } else if self.mode == PointerMode::Armed {
                self.finish_click(&mut events, input.now_ms, &mut double_pinch);
                tap = events.iter().any(|event| matches!(event, PointerEvent::ClickLeft));
            } else if self.mode == PointerMode::Clicked {
                events.push(PointerEvent::ReleaseLeft);
                tap = true;
                self.mode = PointerMode::Hover;
                self.clear_pinch();
            }
            if self.gap_started.is_none() {
                self.gap_started = Some(input.now_ms);
            }
            if input
                .now_ms
                .saturating_sub(self.gap_started.unwrap_or(input.now_ms))
                > 280
            {
                self.reset_motion();
            }
            self.recent_travel *= 0.5;
            return PointerUpdate {
                events,
                norm: None,
                double_pinch,
                repeat_at: self.last_click_at,
                quick_pair: self.quick_pair,
                travel: self.recent_travel,
                tap,
            };
        }
        self.gap_started = None;

        self.quick_pair = false;
        let mut tap = false;
        let aim = input.point.unwrap();
        let anchor = input.anchor.unwrap_or(aim);
        let saw_pinch = input.pinch && input.click_allowed;
        let sticky = matches!(self.mode, PointerMode::Clicked | PointerMode::Dragging);
        let live = if saw_pinch {
            self.pinch_lost_at = None;
            true
        } else if sticky {
            if self.pinch_lost_at.is_none() {
                self.pinch_lost_at = Some(input.now_ms);
            }
            input
                .now_ms
                .saturating_sub(self.pinch_lost_at.unwrap_or(input.now_ms))
                < HOLD_GRACE_MS
        } else {
            self.pinch_lost_at = None;
            false
        };
        let already = matches!(
            self.mode,
            PointerMode::Armed | PointerMode::Clicked | PointerMode::Dragging
        );
        if live && !already {
            self.pinch_base = self.last_aim.or(Some(aim));
            self.drag_from = Some(anchor);
        }
        let held_ms = input
            .now_ms
            .saturating_sub(self.pinch_started.unwrap_or(input.now_ms));
        let knuckle_travel = self
            .drag_from
            .map(|origin| {
                let (ox, oy) = self.project(origin.0, origin.1, input);
                let (nx, ny) = self.project(anchor.0, anchor.1, input);
                let dx = nx - ox;
                let dy = ny - oy;
                (dx * dx + dy * dy).sqrt()
            })
            .unwrap_or(0.0);
        let mut double_pinch = false;
        // Hold the button down: short release = click, move while held = drag.
        if live && self.mode == PointerMode::Armed {
            if knuckle_travel >= DRAG_PX {
                self.mode = PointerMode::Dragging;
                events.push(PointerEvent::PressLeft);
                self.mark_click(input.now_ms, &mut double_pinch);
            } else if held_ms >= 48 {
                self.confirm_press(&mut events, input.now_ms, &mut double_pinch);
            }
        } else if live && self.mode == PointerMode::Clicked && knuckle_travel >= DRAG_PX {
            self.mode = PointerMode::Dragging;
        }

        let releasing = !live && matches!(self.mode, PointerMode::Armed | PointerMode::Dragging);
        let (raw_x, raw_y) = if (live || sticky) && self.mode == PointerMode::Dragging {
            let origin = self.drag_from.unwrap_or(anchor);
            let base = self.pinch_base.unwrap_or(aim);
            (
                base.0 + (anchor.0 - origin.0),
                base.1 + (anchor.1 - origin.1),
            )
        } else if live || releasing || sticky {
            self.pinch_base.unwrap_or(self.last_aim.unwrap_or(aim))
        } else {
            self.pinch_base = None;
            self.drag_from = None;
            self.last_aim = Some(aim);
            aim
        };
        let (steady_x, steady_y) = if input.smoothness <= 0.01 {
            self.follow_x = Some(raw_x);
            self.follow_y = Some(raw_y);
            (raw_x, raw_y)
        } else {
            self.steady(raw_x, raw_y, input.smoothness, input.acceleration)
        };
        let (mut pixel_x, mut pixel_y) = self.project(steady_x, steady_y, input);
        let max_x = input.screen.x + input.screen.w.saturating_sub(1);
        let max_y = input.screen.y + input.screen.h.saturating_sub(1);
        pixel_x = pixel_x.clamp(input.screen.x as f32, max_x as f32);
        pixel_y = pixel_y.clamp(input.screen.y as f32, max_y as f32);
        let point = (pixel_x.round() as i32, pixel_y.round() as i32);

        let step = match self.last_sent {
            Some(previous) => {
                let dx = (point.0 - previous.0) as f32;
                let dy = (point.1 - previous.1) as f32;
                (dx * dx + dy * dy).sqrt()
            }
            None => 0.0,
        };
        self.recent_travel = self.recent_travel * 0.72 + step;
        if self.last_sent.is_none() || step >= input.deadzone.clamp(0.5, 1.5) {
            events.push(PointerEvent::Move(point.0, point.1));
            self.last_sent = Some(point);
        }

        match (self.mode, live) {
            (_, true) if self.mode == PointerMode::Idle || self.mode == PointerMode::Hover => {
                self.mode = PointerMode::Armed;
                self.pinch_started = Some(input.now_ms);
                self.origin = Some(point);
            }
            (PointerMode::Armed, false) => {
                self.finish_click(&mut events, input.now_ms, &mut double_pinch);
            }
            (PointerMode::Clicked, false) => {
                events.push(PointerEvent::ReleaseLeft);
                tap = true;
                self.mode = PointerMode::Hover;
                self.clear_pinch();
            }
            (PointerMode::Dragging, false) => {
                events.push(PointerEvent::ReleaseLeft);
                self.mode = PointerMode::Hover;
                self.clear_pinch();
            }
            (PointerMode::Idle, false) => self.mode = PointerMode::Hover,
            _ => {}
        }
        if events
            .iter()
            .any(|event| matches!(event, PointerEvent::ClickLeft))
        {
            tap = true;
        }

        let norm_x = normalize_axis(
            raw_x,
            input.range.min_x,
            input.range.max_x,
            input.speed * input.profile.speed_scale(),
        );
        let norm_y = normalize_axis(
            raw_y,
            input.range.min_y,
            input.range.max_y,
            input.speed * input.profile.speed_scale(),
        );
        PointerUpdate {
            events,
            norm: Some((norm_x, norm_y)),
            double_pinch,
            repeat_at: self.last_click_at,
            quick_pair: self.quick_pair,
            travel: self.recent_travel,
            tap,
        }
    }

    fn project(&self, x: f32, y: f32, input: &PointerInput) -> (f32, f32) {
        let norm_x = normalize_axis(
            x,
            input.range.min_x,
            input.range.max_x,
            input.speed * input.profile.speed_scale(),
        );
        let norm_y = normalize_axis(
            y,
            input.range.min_y,
            input.range.max_y,
            input.speed * input.profile.speed_scale(),
        );
        let px = input.screen.x as f32
            + norm_x.clamp(0.0, 1.0) * (input.screen.w.saturating_sub(1).max(1) as f32);
        let py = input.screen.y as f32
            + norm_y.clamp(0.0, 1.0) * (input.screen.h.saturating_sub(1).max(1) as f32);
        (px, py)
    }

    /// Moves toward the finger and stops there. No stored speed, so it cannot swing past.
    fn steady(&mut self, x: f32, y: f32, smoothness: f32, acceleration: f32) -> (f32, f32) {
        let Some(px) = self.follow_x else {
            self.follow_x = Some(x);
            self.follow_y = Some(y);
            return (x, y);
        };
        let py = self.follow_y.unwrap_or(y);
        let dx = x - px;
        let dy = y - py;
        let dist = (dx * dx + dy * dy).sqrt();
        let smooth = smoothness.clamp(0.0, 1.0);
        let calm = 0.14 + (1.0 - smooth) * 0.08;
        let moving = ((0.38 + (1.0 - smooth) * 0.32) * (1.0 + acceleration.clamp(0.0, 1.0) * 0.12))
            .min(0.82);
        let alpha = if dist < 0.012 {
            calm
        } else if dist > 0.055 {
            moving
        } else {
            let blend = (dist - 0.012) / 0.043;
            calm + (moving - calm) * blend
        };
        let nx = px + dx * alpha;
        let ny = py + dy * alpha;
        self.follow_x = Some(nx);
        self.follow_y = Some(ny);
        (nx, ny)
    }

    fn reset_motion(&mut self) {
        self.mode = PointerMode::Idle;
        self.pinch_started = None;
        self.origin = None;
        self.gap_started = None;
        self.pinch_lost_at = None;
        self.last_aim = None;
        self.pinch_base = None;
        self.drag_from = None;
        self.follow_x = None;
        self.follow_y = None;
        self.recent_travel = 0.0;
    }

    fn clear_pinch(&mut self) {
        self.pinch_started = None;
        self.origin = None;
        self.pinch_base = None;
        self.drag_from = None;
        self.pinch_lost_at = None;
    }

    fn finish_click(&mut self, events: &mut Vec<PointerEvent>, now_ms: u64, double_pinch: &mut bool) {
        let held = now_ms.saturating_sub(self.pinch_started.unwrap_or(now_ms));
        if (8..1200).contains(&held) {
            events.push(PointerEvent::ClickLeft);
            if let Some(previous) = self.last_click {
                let gap = now_ms.saturating_sub(previous);
                if gap <= 700 {
                    *double_pinch = true;
                    self.quick_pair = gap <= 420;
                }
            }
            self.last_click = Some(now_ms);
            self.last_click_at = self.last_sent;
        }
        self.mode = PointerMode::Hover;
        self.clear_pinch();
    }

    fn confirm_press(&mut self, events: &mut Vec<PointerEvent>, now_ms: u64, double_pinch: &mut bool) {
        events.push(PointerEvent::PressLeft);
        self.mark_click(now_ms, double_pinch);
        self.mode = PointerMode::Clicked;
    }

    fn mark_click(&mut self, now_ms: u64, double_pinch: &mut bool) {
        if let Some(previous) = self.last_click {
            let gap = now_ms.saturating_sub(previous);
            if gap <= 700 {
                *double_pinch = true;
                self.quick_pair = gap <= 420;
            }
        }
        self.last_click = Some(now_ms);
        self.last_click_at = self.last_sent;
    }
}

pub fn normalize_axis(value: f32, min: f32, max: f32, speed: f32) -> f32 {
    let span = (max - min).max(0.05);
    let normalized = ((value - min) / span).clamp(0.0, 1.0);
    (0.5 + (normalized - 0.5) * speed.max(0.2)).clamp(0.0, 1.0)
}

pub fn map_to_screen(norm_x: f32, norm_y: f32, screen: ScreenRect) -> (i32, i32) {
    let x = screen.x
        + (norm_x.clamp(0.0, 1.0) * (screen.w.saturating_sub(1).max(0) as f32)).round() as i32;
    let y = screen.y
        + (norm_y.clamp(0.0, 1.0) * (screen.h.saturating_sub(1).max(0) as f32)).round() as i32;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(now: u64, point: Option<(f32, f32)>, pinch: bool) -> PointerInput {
        PointerInput {
            now_ms: now,
            tracking: point.is_some(),
            point,
            pinch,
            click_allowed: true,
            anchor: None,
            screen: ScreenRect {
                x: 0,
                y: 0,
                w: 1000,
                h: 1000,
            },
            range: NormRect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 1.0,
                max_y: 1.0,
            },
            speed: 1.0,
            acceleration: 0.0,
            deadzone: 0.0,
            smoothness: 0.0,
            profile: SensitivityProfile::Balanced,
        }
    }

    #[test]
    fn center_maps_near_screen_center() {
        let mut pointer = PointerController::default();
        let update = pointer.update(&input(0, Some((0.5, 0.5)), false));
        let move_to = update.events.iter().find_map(|event| match event {
            PointerEvent::Move(x, y) => Some((*x, *y)),
            _ => None,
        });
        let (x, y) = move_to.unwrap();
        assert!((x - 500).abs() < 30, "{x}");
        assert!((y - 500).abs() < 30, "{y}");
    }

    #[test]
    fn virtual_screen_can_start_left_of_primary() {
        let screen = ScreenRect {
            x: -1920,
            y: 0,
            w: 3840,
            h: 1080,
        };
        assert_eq!(map_to_screen(0.0, 0.0, screen).0, -1920);
        assert_eq!(map_to_screen(1.0, 1.0, screen).0, -1920 + 3839);
    }

    #[test]
    fn short_pinch_clicks_and_movement_drags() {
        let mut pointer = PointerController::default();
        pointer.update(&input(0, Some((0.5, 0.5)), false));
        pointer.update(&input(40, Some((0.5, 0.5)), true));
        let released = pointer.update(&input(180, Some((0.5, 0.5)), false));
        assert!(released.events.contains(&PointerEvent::ClickLeft));

        let mut pointer = PointerController::default();
        pointer.update(&input(0, Some((0.40, 0.50)), false));
        pointer.update(&input(40, Some((0.40, 0.50)), true));
        let drag = pointer.update(&input(120, Some((0.70, 0.50)), true));
        assert!(
            drag.events.contains(&PointerEvent::PressLeft),
            "{:?}",
            drag.events
        );
        pointer.update(&input(200, Some((0.70, 0.50)), false));
        let end = pointer.update(&input(340, Some((0.70, 0.50)), false));
        assert!(end.events.contains(&PointerEvent::ReleaseLeft));
    }

    #[test]
    fn pinch_follows_the_hand_not_the_closing_fingertip() {
        let mut pointer = PointerController::default();
        let mut sample = input(0, Some((0.50, 0.40)), false);
        sample.anchor = Some((0.48, 0.55));
        pointer.update(&sample);
        sample.now_ms = 40;
        sample.pinch = true;
        sample.point = Some((0.42, 0.52));
        pointer.update(&sample);
        sample.now_ms = 160;
        sample.point = Some((0.30, 0.62));
        let released = pointer.update(&mut {
            let mut end = sample.clone();
            end.pinch = false;
            end.now_ms = 160;
            end
        });
        assert!(
            released.events.contains(&PointerEvent::ClickLeft),
            "{:?}",
            released.events
        );
        assert!(!released.events.contains(&PointerEvent::PressLeft));
    }

    #[test]
    fn a_short_gap_does_not_throw_away_the_smoother() {
        let mut pointer = PointerController::default();
        let mut sample = input(0, Some((0.40, 0.40)), false);
        sample.smoothness = 0.9;
        pointer.update(&sample);
        sample.now_ms = 40;
        sample.tracking = false;
        sample.point = None;
        let gap = pointer.update(&sample);
        assert!(gap.events.is_empty());
        sample.now_ms = 80;
        sample.tracking = true;
        sample.point = Some((0.41, 0.40));
        let back = pointer.update(&sample);
        let moved = back.events.iter().any(|event| matches!(event, PointerEvent::Move(x, _) if *x > 700));
        assert!(!moved, "{:?}", back.events);
    }

    #[test]
    fn a_held_pinch_that_moves_starts_a_drag() {
        let mut pointer = PointerController::default();
        pointer.update(&input(0, Some((0.50, 0.50)), false));
        pointer.update(&input(30, Some((0.50, 0.50)), true));
        let pressed = pointer.update(&input(100, Some((0.50, 0.50)), true));
        assert!(
            pressed.events.contains(&PointerEvent::PressLeft),
            "{:?}",
            pressed.events
        );
        let drag = pointer.update(&input(160, Some((0.85, 0.50)), true));
        assert!(
            !drag.events.contains(&PointerEvent::PressLeft),
            "button is already down: {:?}",
            drag.events
        );
        assert!(pointer.mode == PointerMode::Dragging);
        pointer.update(&input(220, Some((0.85, 0.50)), false));
        let end = pointer.update(&input(360, Some((0.85, 0.50)), false));
        assert!(end.events.contains(&PointerEvent::ReleaseLeft));
    }

    #[test]
    fn pinch_clicks_on_the_first_hold() {
        let mut pointer = PointerController::default();
        pointer.update(&input(0, Some((0.5, 0.5)), false));
        pointer.update(&input(30, Some((0.5, 0.5)), true));
        let held = pointer.update(&input(90, Some((0.5, 0.5)), true));
        assert!(
            held.events.contains(&PointerEvent::PressLeft),
            "{:?}",
            held.events
        );
        let still = pointer.update(&input(150, Some((0.5, 0.5)), true));
        assert!(!still.events.contains(&PointerEvent::PressLeft));
        pointer.update(&input(200, Some((0.5, 0.5)), false));
        let released = pointer.update(&input(340, Some((0.5, 0.5)), false));
        assert!(released.events.contains(&PointerEvent::ReleaseLeft));
    }

    #[test]
    fn a_fast_move_does_not_swing_past_the_finger() {
        let mut pointer = PointerController::default();
        let mut sample = input(0, Some((0.20, 0.50)), false);
        sample.smoothness = 0.45;
        sample.acceleration = 0.35;
        pointer.update(&sample);
        let mut furthest = 0;
        for step in 1..30 {
            sample.now_ms = step * 33;
            sample.point = Some((0.80, 0.50));
            let update = pointer.update(&sample);
            for event in update.events {
                if let PointerEvent::Move(x, _) = event {
                    furthest = furthest.max(x);
                }
            }
        }
        assert!(furthest <= 820, "cursor swung to {furthest}");
        assert!(furthest >= 700, "cursor never arrived, stopped at {furthest}");
    }

    #[test]
    fn lost_hand_releases_drag() {
        let mut pointer = PointerController::default();
        pointer.update(&input(0, Some((0.3, 0.3)), true));
        pointer.update(&input(80, Some((0.8, 0.3)), true));
        let brief = pointer.update(&input(120, None, false));
        assert!(
            !brief.events.contains(&PointerEvent::ReleaseLeft),
            "a short gap must keep the drag: {:?}",
            brief.events
        );
        let lost = pointer.update(&input(260, None, false));
        assert!(lost.events.contains(&PointerEvent::ReleaseLeft));
    }

    #[test]
    fn a_flicker_does_not_drop_an_active_drag() {
        let mut pointer = PointerController::default();
        pointer.update(&input(0, Some((0.50, 0.50)), false));
        pointer.update(&input(30, Some((0.50, 0.50)), true));
        pointer.update(&input(100, Some((0.50, 0.50)), true));
        pointer.update(&input(160, Some((0.85, 0.50)), true));
        assert_eq!(pointer.mode, PointerMode::Dragging);
        let flicker = pointer.update(&input(200, Some((0.88, 0.50)), false));
        assert!(
            !flicker.events.contains(&PointerEvent::ReleaseLeft),
            "{:?}",
            flicker.events
        );
        assert_eq!(pointer.mode, PointerMode::Dragging);
        let back = pointer.update(&input(240, Some((0.90, 0.50)), true));
        assert!(!back.events.contains(&PointerEvent::ReleaseLeft));
        assert_eq!(pointer.mode, PointerMode::Dragging);
    }
}
