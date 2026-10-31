use serde::{Deserialize, Serialize};

use crate::commands::CommandSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityPreset {
    Performance,
    Balanced,
    Quality,
}

impl QualityPreset {
    pub fn params(self) -> (u32, u32, u32, u32) {
        match self {
            Self::Performance => (640, 480, 20, 4),
            Self::Balanced => (640, 480, 24, 3),
            Self::Quality => (960, 540, 30, 2),
        }
    }

    pub fn from_index(index: i32) -> Self {
        match index {
            0 => Self::Performance,
            2 => Self::Quality,
            _ => Self::Balanced,
        }
    }

    pub fn index(self) -> i32 {
        match self {
            Self::Performance => 0,
            Self::Balanced => 1,
            Self::Quality => 2,
        }
    }
}

#[cfg(test)]
mod quality_tests {
    use super::QualityPreset;

    #[test]
    fn quality_presets_balance_frame_rate_and_preview_work() {
        assert_eq!(QualityPreset::Performance.params(), (640, 480, 20, 4));
        assert_eq!(QualityPreset::Balanced.params(), (640, 480, 24, 3));
        assert_eq!(QualityPreset::Quality.params(), (960, 540, 30, 2));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityProfile {
    Precision,
    Balanced,
    Fast,
}

impl SensitivityProfile {
    pub fn from_index(index: i32) -> Self {
        match index {
            0 => Self::Precision,
            2 => Self::Fast,
            _ => Self::Balanced,
        }
    }

    pub fn index(self) -> i32 {
        match self {
            Self::Precision => 0,
            Self::Balanced => 1,
            Self::Fast => 2,
        }
    }

    #[allow(dead_code)]
    pub fn euro(self, smoothness: f32) -> (f32, f32) {
        let (cutoff, beta) = match self {
            Self::Precision => (0.85, 0.18),
            Self::Balanced => (1.15, 0.4),
            Self::Fast => (1.7, 0.7),
        };
        let smooth = smoothness.clamp(0.0, 1.0);
        let min_cutoff = (cutoff * (1.35 - smooth * 0.7)).max(0.35);
        let beta = (beta * (1.15 - smooth * 0.35)).max(0.0);
        (min_cutoff, beta)
    }

    pub fn speed_scale(self) -> f32 {
        match self {
            Self::Precision => 0.92,
            Self::Balanced => 1.0,
            Self::Fast => 1.12,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrimaryHand {
    Right,
    Left,
    Any,
}

impl PrimaryHand {
    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Left,
            2 => Self::Any,
            _ => Self::Right,
        }
    }

    pub fn index(self) -> i32 {
        match self {
            Self::Right => 0,
            Self::Left => 1,
            Self::Any => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandSide {
    Left,
    Right,
    Unknown,
}

impl HandSide {
    pub fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "left" => Self::Left,
            "right" => Self::Right,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Левая",
            Self::Right => "Правая",
            Self::Unknown => "Рука",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default)]
    pub camera_index: i32,
    #[serde(default)]
    pub quality: QualityPreset,
    #[serde(default = "yes")]
    pub mirror: bool,
    #[serde(default = "yes")]
    pub show_camera: bool,
    #[serde(default)]
    pub sensitivity: SensitivityProfile,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default = "default_smooth")]
    pub smoothness: f32,
    #[serde(default = "default_accel")]
    pub acceleration: f32,
    #[serde(default = "default_deadzone")]
    pub deadzone: f32,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default)]
    pub primary_hand: PrimaryHand,
    #[serde(default = "yes")]
    pub auto_context: bool,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default)]
    pub air_keyboard: bool,
    /// Hold the index tip still ~600 ms to click (accessibility).
    #[serde(default)]
    pub dwell_click: bool,
}

fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}
fn default_speed() -> f32 {
    1.2
}
fn default_smooth() -> f32 {
    0.45
}
fn default_accel() -> f32 {
    0.35
}
fn default_deadzone() -> f32 {
    2.0
}
fn default_confidence() -> f32 {
    0.72
}
fn default_profile() -> String {
    "desktop".to_string()
}

impl Default for QualityPreset {
    fn default() -> Self {
        Self::Balanced
    }
}
impl Default for SensitivityProfile {
    fn default() -> Self {
        Self::Balanced
    }
}
impl Default for PrimaryHand {
    fn default() -> Self {
        Self::Right
    }
}
impl Default for UserSettings {
    fn default() -> Self {
        Self {
            version: 1,
            camera_index: 0,
            quality: QualityPreset::Balanced,
            mirror: true,
            show_camera: true,
            sensitivity: SensitivityProfile::Balanced,
            speed: default_speed(),
            smoothness: default_smooth(),
            acceleration: default_accel(),
            deadzone: default_deadzone(),
            confidence: default_confidence(),
            primary_hand: PrimaryHand::Right,
            auto_context: true,
            profile: default_profile(),
            air_keyboard: false,
            dwell_click: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NormRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl NormRect {
    #[allow(dead_code)]
    pub fn standard() -> Self {
        Self {
            min_x: 0.15,
            min_y: 0.15,
            max_x: 0.85,
            max_y: 0.85,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationProfile {
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default = "inset_min")]
    pub min_x: f32,
    #[serde(default = "inset_min")]
    pub min_y: f32,
    #[serde(default = "inset_max")]
    pub max_x: f32,
    #[serde(default = "inset_max")]
    pub max_y: f32,
    #[serde(default = "default_pinch")]
    pub pinch_ratio: f32,
    /// Thumb-to-index distance while this person's palm is open.
    #[serde(default)]
    pub open_ratio: f32,
    /// Thumb-to-index gap at the moment their fingers actually meet.
    #[serde(default)]
    pub pinch_gap: f32,
    /// Thumb-to-index gap while this person holds a thumbs-up.
    #[serde(default)]
    pub thumb_gap: f32,
    #[serde(default)]
    pub hand_scale: f32,
    #[serde(default)]
    pub finger_span: f32,
    /// How open index, middle, ring and little finger were on this person's palm.
    #[serde(default)]
    pub palm_open: [f32; 4],
    /// The same four fingers while they point with the index.
    #[serde(default)]
    pub point_open: [f32; 4],
    /// The same four fingers while they hold a thumbs-up.
    #[serde(default)]
    pub thumb_open: [f32; 4],
    /// Personal V-sign fingertip spread.
    #[serde(default)]
    pub v_spread: f32,
    /// How open a finger must be to count as extended for this person.
    #[serde(default)]
    pub extend_min: f32,
    /// How curled a finger must be for fist / thumb poses.
    #[serde(default)]
    pub curl_max: f32,
    #[serde(default)]
    pub valid: bool,
}

fn inset_min() -> f32 {
    0.15
}
fn inset_max() -> f32 {
    0.85
}
fn default_pinch() -> f32 {
    0.5
}

impl Default for CalibrationProfile {
    fn default() -> Self {
        Self {
            version: 1,
            min_x: 0.15,
            min_y: 0.15,
            max_x: 0.85,
            max_y: 0.85,
            pinch_ratio: default_pinch(),
            open_ratio: 0.0,
            pinch_gap: 0.0,
            thumb_gap: 0.0,
            hand_scale: 0.0,
            finger_span: 0.0,
            palm_open: [0.0; 4],
            point_open: [0.0; 4],
            thumb_open: [0.0; 4],
            v_spread: 0.0,
            extend_min: 0.0,
            curl_max: 0.0,
            valid: false,
        }
    }
}

impl CalibrationProfile {
    /// How close the thumb and index must be before a pinch counts.
    /// Before calibration this is strict, so hovering fingers do not click.
    pub fn pinch_limit(&self) -> f32 {
        if self.valid && self.pinch_ratio > 0.0 {
            // The saved number is the tightest touch. A little room past it still
            // counts as their pinch, and stays far from the open hand.
            let slack = if self.open_ratio > self.pinch_ratio + 0.12 {
                ((self.open_ratio - self.pinch_ratio) * 0.15).clamp(0.04, 0.10)
            } else {
                0.04
            };
            (self.pinch_ratio + slack).clamp(0.18, 0.42)
        } else {
            0.36
        }
    }

    /// Gap below which a pinch counts for this person.
    /// Zero means the general rule, before their thumb and pinch were both measured.
    pub fn gap_limit(&self) -> f32 {
        if self.valid && self.pinch_gap > 0.05 && self.thumb_gap > self.pinch_gap + 0.12 {
            let room = (self.thumb_gap - self.pinch_gap).clamp(0.04, 0.08);
            (self.pinch_gap + room).clamp(0.16, 0.42)
        } else {
            0.0
        }
    }

    pub fn has_shapes(&self) -> bool {
        self.valid && self.point_open.iter().sum::<f32>() > 0.2
    }

    pub fn extend_gate(&self) -> f32 {
        if self.valid && self.extend_min > 0.05 {
            self.extend_min.clamp(0.28, 0.42)
        } else {
            0.34
        }
    }

    pub fn curl_gate(&self) -> f32 {
        if self.valid && self.curl_max > 0.05 {
            self.curl_max.clamp(0.28, 0.48)
        } else {
            0.34
        }
    }

    pub fn v_gate(&self) -> f32 {
        if self.valid && self.v_spread > 0.05 {
            (self.v_spread * 0.85).clamp(0.14, 0.32)
        } else {
            0.18
        }
    }

    /// Far hands look smaller. Open the gates a little so the same pose still counts.
    pub fn distance_boost(&self, live_scale: f32) -> f32 {
        if !self.valid || self.hand_scale < 0.04 || live_scale <= 0.0 {
            return 1.0;
        }
        let ratio = (self.hand_scale / live_scale.max(0.02)).clamp(0.5, 2.0);
        if ratio > 1.12 {
            (1.0 + (ratio - 1.0) * 0.22).min(1.35)
        } else if ratio < 0.88 {
            (1.0 - (1.0 - ratio) * 0.14).max(0.84)
        } else {
            1.0
        }
    }

    /// Cursor area is the whole picture, so a small traced box cannot trap the pointer.
    pub fn range(&self) -> NormRect {
        let _ = self.valid;
        NormRect {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 1.0,
            max_y: 1.0,
        }
    }
}

/// Personal pose gates passed into the classifier.
#[derive(Clone, Copy, Debug)]
pub struct PoseGates {
    pub extend: f32,
    pub curl: f32,
    pub v_spread: f32,
    pub distance: f32,
}

impl Default for PoseGates {
    fn default() -> Self {
        Self {
            extend: 0.34,
            curl: 0.34,
            v_spread: 0.18,
            distance: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GestureBinding {
    pub command: CommandSpec,
    #[serde(default = "default_confidence")]
    pub min_confidence: f32,
    #[serde(default)]
    pub min_duration_ms: u64,
    #[serde(default = "default_cooldown")]
    pub cooldown_ms: u64,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn default_cooldown() -> u64 {
    450
}

impl GestureBinding {
    pub fn new(
        command: CommandSpec,
        min_confidence: f32,
        min_duration_ms: u64,
        cooldown_ms: u64,
    ) -> Self {
        Self {
            command,
            min_confidence,
            min_duration_ms,
            cooldown_ms,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub title: String,
    pub mappings: Vec<(String, GestureBinding)>,
}

impl Profile {
    pub fn binding(&self, id: &str) -> Option<&GestureBinding> {
        self.mappings
            .iter()
            .find(|(name, _)| name == id)
            .map(|(_, binding)| binding)
    }

    pub fn binding_mut(&mut self, id: &str) -> Option<&mut GestureBinding> {
        self.mappings
            .iter_mut()
            .find(|(name, _)| name == id)
            .map(|(_, binding)| binding)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomGesture {
    pub name: String,
    pub threshold: f32,
    pub frames: Vec<Vec<f32>>,
    /// Extra takes of the same gesture (best-of match).
    #[serde(default)]
    pub templates: Vec<Vec<Vec<f32>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStatistics {
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default)]
    pub gestures_recognized: u64,
    #[serde(default)]
    pub commands_dispatched: u64,
    #[serde(default)]
    pub clicks: u64,
    #[serde(default)]
    pub scrolls: u64,
    #[serde(default)]
    pub rejected: u64,
    #[serde(default)]
    pub camera_reconnects: u64,
    #[serde(default)]
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct TrackedHand {
    /// Stable across frames: matched by palm proximity, not MediaPipe array order.
    pub track_id: u64,
    pub side: HandSide,
    pub score: f32,
    pub landmarks: [[f32; 3]; 21],
    /// Metric landmarks in meters, when the camera model provides them.
    pub world: Option<[[f32; 3]; 21]>,
}

#[derive(Debug, Clone)]
pub struct CameraInfo {
    pub index: i32,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct GestureInfo {
    pub id: String,
    pub title: String,
    pub command: String,
    pub detail: String,
    pub custom: bool,
}

#[derive(Debug, Clone)]
pub struct ProfileInfo {
    pub id: String,
    pub title: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub revision: u64,
    pub settings_revision: u64,
    pub gestures_revision: u64,
    pub cameras_revision: u64,
    pub profiles_revision: u64,
    pub frame_revision: u64,
    pub system_status: String,
    pub camera_status: String,
    pub hand_status: String,
    pub gesture: String,
    pub confidence: f32,
    pub command: String,
    pub command_status: String,
    pub rejection: String,
    pub profile: String,
    pub context_line: String,
    pub fps: f32,
    pub latency_ms: f32,
    pub capture_ms: f32,
    pub inference_ms: f32,
    pub ipc_ms: f32,
    pub gesture_ms: f32,
    pub dispatch_ms: f32,
    pub control_on: bool,
    pub camera_on: bool,
    pub paused: bool,
    pub ipc_ok: bool,
    pub cpu: f32,
    pub ram_mb: f32,
    pub vision_pid: u32,
    pub error: String,
    pub calibration_title: String,
    pub calibration_hint: String,
    pub calibration_progress: f32,
    pub calibration_active: bool,
    pub coach_line: String,
    pub tracking_health: String,
    pub dwell_click: bool,
    pub session_recording: bool,
    pub stats_gestures: u64,
    pub stats_commands: u64,
    pub stats_clicks: u64,
    pub stats_scrolls: u64,
    pub stats_rejected: u64,
    pub stats_reconnects: u64,
    pub stats_uptime: u64,
    pub log_text: String,
    pub show_camera: bool,
    pub mirror: bool,
    pub auto_context: bool,
    pub air_keyboard: bool,
    pub play_mode: i32,
    pub play_instrument: i32,
    pub play_element: i32,
    pub play_brush: i32,
    pub play_color: i32,
    pub play_preset: i32,
    pub play_guitar_chord: i32,
    pub play_guitar_mode: i32,
    pub play_guitar_capo: i32,
    pub play_muted: bool,
    pub play_volume: f32,
    pub play_sensitivity: f32,
    pub play_thickness: f32,
    pub play_fade: f32,
    pub play_label: String,
    pub play_hint: String,
    pub play_revision: u64,
    pub speed: f32,
    pub smoothness: f32,
    pub acceleration: f32,
    pub deadzone: f32,
    pub confidence_threshold: f32,
    pub quality_index: i32,
    pub hand_index: i32,
    pub sensitivity_index: i32,
    pub cameras: Vec<CameraInfo>,
    pub gestures: Vec<GestureInfo>,
    pub profiles: Vec<ProfileInfo>,
    pub preview: Option<std::sync::Arc<PreviewImage>>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            settings_revision: 1,
            gestures_revision: 1,
            cameras_revision: 1,
            profiles_revision: 1,
            frame_revision: 0,
            system_status: "Ожидание".into(),
            camera_status: "Камера выключена".into(),
            hand_status: "Рука не видна".into(),
            gesture: "—".into(),
            confidence: 0.0,
            command: "—".into(),
            command_status: "ожидание".into(),
            rejection: String::new(),
            profile: "Рабочий стол".into(),
            context_line: "Автовыбор выключен".into(),
            fps: 0.0,
            latency_ms: 0.0,
            capture_ms: 0.0,
            inference_ms: 0.0,
            ipc_ms: 0.0,
            gesture_ms: 0.0,
            dispatch_ms: 0.0,
            control_on: false,
            camera_on: false,
            paused: false,
            ipc_ok: false,
            cpu: 0.0,
            ram_mb: 0.0,
            vision_pid: 0,
            error: String::new(),
            calibration_title: "Калибровка не выполнялась".into(),
            calibration_hint:
                "Мастер измеряет зону руки и порог щипка. Модель распознавания не обучается.".into(),
            calibration_progress: 0.0,
            calibration_active: false,
            coach_line: String::new(),
            tracking_health: "трекинг в норме".into(),
            dwell_click: false,
            session_recording: false,
            stats_gestures: 0,
            stats_commands: 0,
            stats_clicks: 0,
            stats_scrolls: 0,
            stats_rejected: 0,
            stats_reconnects: 0,
            stats_uptime: 0,
            log_text: String::new(),
            show_camera: true,
            mirror: true,
            auto_context: true,
            air_keyboard: false,
            play_mode: 0,
            play_instrument: 0,
            play_element: 0,
            play_brush: 0,
            play_color: 3,
            play_preset: 0,
            play_guitar_chord: 0,
            play_guitar_mode: 0,
            play_guitar_capo: 0,
            play_muted: false,
            play_volume: 0.8,
            play_sensitivity: 1.0,
            play_thickness: 5.5,
            play_fade: 0.045,
            play_label: "—".into(),
            play_hint: "Музыка, эффекты или рисунок. Курсор останется у обычной мыши.".into(),
            play_revision: 1,
            speed: default_speed(),
            smoothness: default_smooth(),
            acceleration: default_accel(),
            deadzone: default_deadzone(),
            confidence_threshold: default_confidence(),
            quality_index: 1,
            hand_index: 0,
            sensitivity_index: 1,
            cameras: Vec::new(),
            gestures: Vec::new(),
            profiles: Vec::new(),
            preview: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl ScreenRect {
    pub fn fallback() -> Self {
        Self {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        }
    }
}
