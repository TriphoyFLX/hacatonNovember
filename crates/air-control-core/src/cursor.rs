//! Absolute cursor mapping with One Euro smoothing and a small velocity boost.
//!
//! The resting position stays tied to the hand. Acceleration adds a capped,
//! decaying offset so a fast flick feels quicker without drifting away from
//! the finger.

use crate::filter::OneEuroFilter;
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
    Dragging,
}

#[derive(Debug, Clone)]
pub struct PointerInput {
    pub now_ms: u64,
    pub tracking: bool,
    pub point: Option<(f32, f32)>,
    pub pinch: bool,
    pub click_allowed: bool,
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
}

#[derive(Debug)]
pub struct PointerController {
    mode: PointerMode,
    pinch_started: Option<u64>,
    origin: Option<(i32, i32)>,
    last_sent: Option<(i32, i32)>,
    last_click: Option<u64>,
    filter_x: OneEuroFilter,
    filter_y: OneEuroFilter,
    boost_x: f32,
    boost_y: f32,
    previous_abs: Option<(f32, f32, u64)>,
    params: (f32, f32),
}

impl Default for PointerController {
    fn default() -> Self {
        Self {
            mode: PointerMode::Idle,
            pinch_started: None,
            origin: None,
            last_sent: None,
            last_click: None,
            filter_x: OneEuroFilter::new(1.7, 0.04),
            filter_y: OneEuroFilter::new(1.7, 0.04),
            boost_x: 0.0,
            boost_y: 0.0,
            previous_abs: None,
            params: (1.7, 0.04),
        }
    }
}

impl PointerController {
    pub fn update(&mut self, input: &PointerInput) -> PointerUpdate {
        let mut events = Vec::new();
        let (cutoff, beta) = if input.smoothness <= 0.01 {
            (8.0, 0.0)
        } else {
            input.profile.euro(input.smoothness)
        };
        if (cutoff, beta) != self.params {
            self.filter_x.set_params(cutoff, beta);
            self.filter_y.set_params(cutoff, beta);
            self.params = (cutoff, beta);
        }

        if !input.tracking || input.point.is_none() {
            if self.mode == PointerMode::Dragging {
                events.push(PointerEvent::ReleaseLeft);
            }
            self.reset_motion();
            return PointerUpdate {
                events,
                norm: None,
                double_pinch: false,
            };
        }

        let (raw_x, raw_y) = input.point.unwrap();
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
        let time_s = input.now_ms as f32 / 1000.0;
        let filtered_x = self.filter_x.filter(time_s, norm_x);
        let filtered_y = self.filter_y.filter(time_s, norm_y);
        let abs_x = input.screen.x as f32
            + filtered_x.clamp(0.0, 1.0) * (input.screen.w.saturating_sub(1).max(1) as f32);
        let abs_y = input.screen.y as f32
            + filtered_y.clamp(0.0, 1.0) * (input.screen.h.saturating_sub(1).max(1) as f32);
        let (boost_x, boost_y) = self.boost(abs_x, abs_y, input.now_ms, input.acceleration);
        let mut pixel_x = abs_x + boost_x;
        let mut pixel_y = abs_y + boost_y;
        let max_x = input.screen.x + input.screen.w.saturating_sub(1);
        let max_y = input.screen.y + input.screen.h.saturating_sub(1);
        pixel_x = pixel_x.clamp(input.screen.x as f32, max_x as f32);
        pixel_y = pixel_y.clamp(input.screen.y as f32, max_y as f32);
        let point = (pixel_x.round() as i32, pixel_y.round() as i32);

        let moved = match self.last_sent {
            Some(previous) => {
                let dx = (point.0 - previous.0) as f32;
                let dy = (point.1 - previous.1) as f32;
                (dx * dx + dy * dy).sqrt() >= input.deadzone
            }
            None => true,
        };
        if moved {
            events.push(PointerEvent::Move(point.0, point.1));
            self.last_sent = Some(point);
        }

        let mut double_pinch = false;
        match (self.mode, input.pinch && input.click_allowed) {
            (_, true) if self.mode == PointerMode::Idle || self.mode == PointerMode::Hover => {
                self.mode = PointerMode::Armed;
                self.pinch_started = Some(input.now_ms);
                self.origin = Some(point);
            }
            (PointerMode::Armed, true) => {
                if let Some(origin) = self.origin {
                    let dx = (point.0 - origin.0) as f32;
                    let dy = (point.1 - origin.1) as f32;
                    let held = input
                        .now_ms
                        .saturating_sub(self.pinch_started.unwrap_or(input.now_ms));
                    if held >= 40 && (dx * dx + dy * dy).sqrt() >= 18.0 {
                        self.mode = PointerMode::Dragging;
                        events.push(PointerEvent::PressLeft);
                    }
                }
            }
            (PointerMode::Armed, false) => {
                let held = input
                    .now_ms
                    .saturating_sub(self.pinch_started.unwrap_or(input.now_ms));
                if (40..500).contains(&held) {
                    events.push(PointerEvent::ClickLeft);
                    if let Some(previous) = self.last_click {
                        if input.now_ms.saturating_sub(previous) <= 450 {
                            double_pinch = true;
                        }
                    }
                    self.last_click = Some(input.now_ms);
                }
                self.mode = PointerMode::Hover;
                self.pinch_started = None;
                self.origin = None;
            }
            (PointerMode::Dragging, false) => {
                events.push(PointerEvent::ReleaseLeft);
                self.mode = PointerMode::Hover;
                self.pinch_started = None;
                self.origin = None;
            }
            (PointerMode::Idle, false) => self.mode = PointerMode::Hover,
            _ => {}
        }

        PointerUpdate {
            events,
            norm: Some((filtered_x, filtered_y)),
            double_pinch,
        }
    }

    fn boost(&mut self, x: f32, y: f32, now_ms: u64, acceleration: f32) -> (f32, f32) {
        if let Some((px, py, pt)) = self.previous_abs {
            let dt = (now_ms.saturating_sub(pt) as f32 / 1000.0).clamp(0.001, 0.1);
            let vx = (x - px) / dt;
            let vy = (y - py) / dt;
            let accel = acceleration.clamp(0.0, 1.0);
            self.boost_x = self.boost_x * (-dt * 8.0).exp() + vx * accel * 0.012;
            self.boost_y = self.boost_y * (-dt * 8.0).exp() + vy * accel * 0.012;
            self.boost_x = self.boost_x.clamp(-120.0, 120.0);
            self.boost_y = self.boost_y.clamp(-120.0, 120.0);
        }
        self.previous_abs = Some((x, y, now_ms));
        (self.boost_x, self.boost_y)
    }

    fn reset_motion(&mut self) {
        self.mode = PointerMode::Idle;
        self.pinch_started = None;
        self.origin = None;
        self.previous_abs = None;
        self.boost_x = 0.0;
        self.boost_y = 0.0;
        self.filter_x.reset();
        self.filter_y.reset();
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
        let end = pointer.update(&input(200, Some((0.70, 0.50)), false));
        assert!(end.events.contains(&PointerEvent::ReleaseLeft));
    }

    #[test]
    fn lost_hand_releases_drag() {
        let mut pointer = PointerController::default();
        pointer.update(&input(0, Some((0.3, 0.3)), true));
        pointer.update(&input(80, Some((0.8, 0.3)), true));
        let lost = pointer.update(&input(120, None, false));
        assert!(lost.events.contains(&PointerEvent::ReleaseLeft));
    }
}
