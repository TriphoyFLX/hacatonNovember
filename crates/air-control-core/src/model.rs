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
            Self::Performance => (640, 480, 30, 3),
            Self::Balanced => (640, 480, 30, 2),
            Self::Quality => (960, 540, 30, 1),
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

    pub fn euro(self, smoothness: f32) -> (f32, f32) {
        let (cutoff, beta) = match self {
            Self::Precision => (1.15, 0.015),
            Self::Balanced => (1.7, 0.04),
            Self::Fast => (2.6, 0.09),
        };
        let smooth = smoothness.clamp(0.0, 1.0);
        let min_cutoff = (cutoff * (1.45 - smooth)).max(0.45);
        let beta = (beta * (1.25 - smooth * 0.45)).max(0.0);
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
    #[serde(default)]
    pub hand_scale: f32,
    #[serde(default)]
    pub finger_span: f32,
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
    0.42
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
            hand_scale: 0.0,
            finger_span: 0.0,
            valid: false,
        }
    }
}

impl CalibrationProfile {
    pub fn range(&self) -> NormRect {
        let mut rect = if self.valid {
            NormRect {
                min_x: self.min_x,
                min_y: self.min_y,
                max_x: self.max_x,
                max_y: self.max_y,
            }
        } else {
            NormRect::standard()
        };
        if rect.max_x - rect.min_x < 0.12 {
            rect = NormRect::standard();
        }
        if rect.max_y - rect.min_y < 0.12 {
            rect = NormRect::standard();
        }
        rect
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
    pub side: HandSide,
    pub score: f32,
    pub landmarks: [[f32; 3]; 21],
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
