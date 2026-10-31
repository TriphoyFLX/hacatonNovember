//! Orchestrates tracking, gesture decisions, safety, and the input thread.
//!
//! Control stays off until the user enables it. Pause releases every held
//! button and key. The vision process cannot submit a system command.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;

use crate::coach::{
    quality_gate, scene_hint, scene_hint_text, tracking_health, MistakeLog, SceneHint, Stillness,
    TremorMonitor,
};
use crate::commands::{command_label, gesture_how, gesture_title, review, CommandSpec};
use crate::cursor::{PointerController, PointerEvent, PointerInput};
use crate::error::CoreError;
use crate::filter::{EuroTuner, OneEuroFilter};
use crate::gestures::{
    aim_point, aim_point_depth, classify_hand_gap, classify_hand_gated, cursor_distance_gain,
    distance, edge_penalty, finger_shape, hand_collapsed, index_press_active, match_custom,
    normalize_landmarks, pinch_anchor, resample, select_hands, steady_hands,
    CustomMatchStats, GateResult, GateSet, HandLinker, HandMemory, MotionTracker, PoseHold,
    RememberedPoses, BUILTIN_GESTURES,
};
use crate::input::{self, spawn_hotkey, spawn_input, HotkeyHandle, InputMsg};
use crate::model::NormRect;
use crate::model::{
    CalibrationProfile, CameraInfo, ContextRule, ContextRuleInfo, CustomGesture, GestureInfo,
    HandSide, PoseGates, PreviewImage, PrimaryHand, Profile, ProfileInfo, QualityPreset, ScreenRect,
    SensitivityProfile, Snapshot, TrackedHand, UsageStatistics, UserSettings,
};
use crate::preview::{
    decode_jpeg, downscale_rgb, render_preview, render_preview_rgb, HandStyle,
};
use crate::protocol::TrackingMessage;
use crate::session::{SessionFrame, SessionHand, SessionRecorder};
use crate::stage::Stage;
use crate::store::{
    self, factory_profile, profile_coach_line, resolve_profile_id, valid_name, validate_custom,
};
use crate::uia::{self, UiTarget};
use crate::vision::{spawn_vision, VisionCommand, VisionConfig, VisionEvent, VisionSystem};
use crate::voice;

pub type CommandSender = mpsc::Sender<EngineCommand>;

pub struct EngineHandle {
    commands: mpsc::Sender<EngineCommand>,
    snapshot: Arc<RwLock<Snapshot>>,
    done: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    runtime: Option<tokio::runtime::Runtime>,
    _hotkey: HotkeyHandle,
}

#[derive(Debug)]
pub enum EngineCommand {
    StartCamera,
    StopCamera,
    RefreshCameras,
    OpenCameraSettings,
    SetControl(bool),
    TogglePause,
    EmergencyStop,
    SetVoiceListen(bool),
    SelectCamera(i32),
    SetQuality(i32),
    SetMirror(bool),
    SetShowCamera(bool),
    SetPrimaryHand(i32),
    SetSensitivityProfile(i32),
    SetSpeed(f32),
    SetSmoothness(f32),
    SetAcceleration(f32),
    SetDeadzone(f32),
    SetConfidence(f32),
    SetProfile(String),
    SetAutoContext(bool),
    SetAirKeyboard(bool),
    SetDwellClick(bool),
    SetSnapUi(bool),
    SetSnapRadius(f32),
    SetDepthCursor(bool),
    SetDepthGain(f32),
    ToggleSessionRecord,
    AddContextRule { pattern: String, profile_id: String },
    ToggleContextRule { pattern: String },
    RemoveContextRule { pattern: String },
    ResetProfile(String),
    SetProfileTune {
        profile_id: String,
        override_tune: bool,
        speed: f32,
        smoothness: f32,
        deadzone: f32,
        confidence: f32,
    },
    SuggestThresholds,
    ApplySuggestedConfidence(f32),
    StartExam,
    CalibrationStart,
    CalibrationNext,
    CalibrationCancel,
    CalibrationDrill,
    RecordGesture { name: String, threshold: f32 },
    DeleteGesture(String),
    AssignCommand { gesture: String, label: String },
    ExportGestures(std::path::PathBuf),
    ImportGestures(std::path::PathBuf),
    SetPlayMode(i32),
    SetPlayInstrument(i32),
    SetPlayElement(i32),
    SetPlayBrush(i32),
    SetPlayColor(i32),
    SetPlayPreset(i32),
    SetGuitarChord(i32),
    SetGuitarMode(i32),
    SetGuitarCapo(i32),
    PluckGuitarString(i32),
    SetPlayMuted(bool),
    SetPlayVolume(f32),
    SetPlaySensitivity(f32),
    SetPlayThickness(f32),
    SetPlayFade(f32),
    ClearArt,
    /// Active Slint page — preview JPEG/paint only for the page that shows the camera.
    SetUiPage(i32),
    Note(String),
    Shutdown,
}

pub fn start_engine() -> Result<EngineHandle, CoreError> {
    let snapshot = Arc::new(RwLock::new(Snapshot::default()));
    let (tx, rx) = mpsc::channel(128);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let hotkey_tx = tx.clone();
    let hotkey_err = tx.clone();
    let hotkey = spawn_hotkey(
        move || {
            let _ = hotkey_tx.blocking_send(EngineCommand::TogglePause);
        },
        move |message| {
            let _ = hotkey_err.blocking_send(EngineCommand::Note(message));
        },
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| CoreError::Vision(err.to_string()))?;
    let shared = snapshot.clone();
    runtime.spawn(async move {
        if let Err(err) = run(rx, shared).await {
            tracing::error!(error = %err, "engine stopped");
        }
        let _ = done_tx.send(());
    });
    Ok(EngineHandle {
        commands: tx,
        snapshot,
        done: Mutex::new(Some(done_rx)),
        runtime: Some(runtime),
        _hotkey: hotkey,
    })
}

impl EngineHandle {
    pub fn commands(&self) -> CommandSender {
        self.commands.clone()
    }

    pub fn snapshot(&self) -> Arc<RwLock<Snapshot>> {
        self.snapshot.clone()
    }

    pub fn shutdown(&mut self) {
        let _ = self.commands.blocking_send(EngineCommand::Shutdown);
        if let Some(done) = self.done.lock().ok().and_then(|mut slot| slot.take()) {
            let _ = done.recv_timeout(Duration::from_secs(4));
        }
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.shutdown();
        self.runtime.take();
    }
}

#[derive(PartialEq, Eq)]
enum Flow {
    Continue,
    Stop,
}

struct Session {
    settings: UserSettings,
    calibration: CalibrationProfile,
    profiles: Vec<Profile>,
    custom: Vec<CustomGesture>,
    stats: UsageStatistics,
    started: Instant,
    uptime_mark: u64,
    gates: GateSet,
    pointer: PointerController,
    pinch_held: bool,
    /// Keep a held pinch alive through a few noisy frames while dragging.
    pinch_sticky_until: u64,
    last_good_aim: Option<(f32, f32)>,
    last_good_anchor: Option<(f32, f32)>,
    stillness: Stillness,
    tremor: TremorMonitor,
    euro_tuner: EuroTuner,
    custom_stats: CustomMatchStats,
    dwell: DwellClick,
    session_dump: Option<SessionRecorder>,
    mistakes: MistakeLog,
    coach_line: String,
    tracking_health: String,
    scene_hint: SceneHint,
    sharpness: f32,
    brightness: f32,
    flicker: f32,
    wanted_pose: String,
    motion: MotionTracker,
    memory: HandMemory,
    primary_pose: PoseHold,
    secondary_pose: PoseHold,
    last_wrists: Vec<(f32, f32)>,
    last_hand_ms: u64,
    history: VecDeque<Vec<f32>>,
    frame_times: VecDeque<u64>,
    input: Option<crossbeam_channel::Sender<InputMsg>>,
    input_ok: bool,
    snapshot: Arc<RwLock<Snapshot>>,
    logs: VecDeque<String>,
    control: bool,
    paused: bool,
    camera_on: bool,
    released: bool,
    vision_pid: u32,
    ipc_ok: bool,
    camera_status: String,
    error: String,
    cameras: Vec<CameraInfo>,
    calibration_session: Option<CalSession>,
    recorder: Option<Recorder>,
    latency_avg: f32,
    capture_ms: f32,
    inference_ms: f32,
    ipc_ms: f32,
    gesture_ms: f32,
    dispatch_ms: f32,
    fps: f32,
    cpu: f32,
    ram_mb: f32,
    system: sysinfo::System,
    screen: ScreenRect,
    screen_at: Instant,
    context_checked: Instant,
    context_app: String,
    context_profile: String,
    our_pid: u32,
    gesture: String,
    confidence: f32,
    command: String,
    command_status: String,
    rejection: String,
    hand_status: String,
    settings_revision: u64,
    gestures_revision: u64,
    cameras_revision: u64,
    profiles_revision: u64,
    frame_revision: u64,
    preview: Option<Arc<PreviewImage>>,
    play_frame_revision: u64,
    play_preview: Option<Arc<PreviewImage>>,
    /// Last decoded camera still for instrument HUD (avoids black flicker, reused without re-decode).
    stage_cam: Option<image::RgbImage>,
    play_paint_tick: u32,
    /// Current UI page from Slint (0 start, 1 hand, 3 calibrate, 7 camera, 10 scene, 12 instruments…).
    ui_page: i32,
    last_stats_save: Instant,
    stage: Stage,
    stage_guard: bool,
    scene_cursor: bool,
    glide: HandGlide,
    linker: HandLinker,
    play_revision: u64,
    context_rules: Vec<ContextRule>,
    ui_targets: Vec<UiTarget>,
    ui_targets_at: Instant,
    snap_target: String,
    threshold_suggest: String,
    suggested_confidence: f32,
    session_frames: u64,
    exam: Option<ExamSession>,
    /// Local speech worker. The microphone stays closed until the user asks.
    voice: voice::VoiceSystem,
    voice_listening: bool,
    voice_ready: bool,
    voice_line: String,
    last_voice: Instant,
}

struct ExamSession {
    poses: Vec<&'static str>,
    index: usize,
    held: u32,
}

struct GlideSlot {
    track_id: u64,
    palm: (f32, f32),
    axes: Vec<OneEuroFilter>,
    world: Vec<OneEuroFilter>,
}

struct HandGlide {
    slots: Vec<GlideSlot>,
}

impl Default for HandGlide {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

#[derive(Debug, Default, Clone)]
struct DwellClick {
    anchor: Option<(f32, f32)>,
    since_ms: u64,
    fired: bool,
}

impl DwellClick {
    fn clear(&mut self) {
        self.anchor = None;
        self.since_ms = 0;
        self.fired = false;
    }

    /// Returns true once when the tip stayed inside R for ~600 ms.
    fn tick(&mut self, now_ms: u64, tip: (f32, f32)) -> bool {
        const RADIUS: f32 = 0.018;
        const HOLD_MS: u64 = 600;
        match self.anchor {
            Some(anchor) if (tip.0 - anchor.0).hypot(tip.1 - anchor.1) <= RADIUS => {
                if !self.fired && now_ms.saturating_sub(self.since_ms) >= HOLD_MS {
                    self.fired = true;
                    return true;
                }
            }
            _ => {
                self.anchor = Some(tip);
                self.since_ms = now_ms;
                self.fired = false;
            }
        }
        false
    }
}

impl HandGlide {
    fn apply_params(&mut self, min_cutoff: f32, beta: f32) {
        for slot in &mut self.slots {
            for filter in &mut slot.axes {
                filter.set_params(min_cutoff, beta);
            }
            for filter in &mut slot.world {
                filter.set_params((min_cutoff * 1.3).max(1.5), (beta * 1.2).max(0.6));
            }
        }
    }

    fn follow(&mut self, now_ms: u64, hands: &[TrackedHand], latency_ms: f32) -> Vec<TrackedHand> {
        let time = now_ms as f32 * 0.001;
        let delay = (latency_ms * 0.001).clamp(0.0, 0.12);
        let mut used = vec![false; self.slots.len()];
        let mut out = Vec::with_capacity(hands.len());
        for hand in hands {
            let palm = (
                (hand.landmarks[0][0] + hand.landmarks[5][0] + hand.landmarks[17][0]) / 3.0,
                (hand.landmarks[0][1] + hand.landmarks[5][1] + hand.landmarks[17][1]) / 3.0,
            );
            let by_id = self.slots.iter().enumerate().find(|(index, slot)| {
                !used[*index] && slot.track_id != 0 && slot.track_id == hand.track_id
            });
            let index = by_id.map(|(index, _)| index).or_else(|| {
                self.slots
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !used[*index])
                    .map(|(index, slot)| {
                        let dist = (slot.palm.0 - palm.0).hypot(slot.palm.1 - palm.1);
                        (index, dist)
                    })
                    .filter(|(_, dist)| *dist < 0.28)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(index, _)| index)
            });
            let index = match index {
                Some(index) => index,
                None => {
                    let (cut, beta) = self
                        .slots
                        .first()
                        .and_then(|slot| slot.axes.first().map(|f| f.params()))
                        .unwrap_or((1.7, 0.9));
                    self.slots.push(GlideSlot {
                        track_id: hand.track_id,
                        palm,
                        axes: (0..63).map(|_| OneEuroFilter::new(cut, beta)).collect(),
                        world: (0..63)
                            .map(|_| {
                                OneEuroFilter::new((cut * 1.3).max(1.5), (beta * 1.2).max(0.6))
                            })
                            .collect(),
                    });
                    used.push(false);
                    self.slots.len() - 1
                }
            };
            used[index] = true;
            self.slots[index].track_id = hand.track_id;
            self.slots[index].palm = palm;
            let axes = &mut self.slots[index].axes;
            let mut landmarks = hand.landmarks;
            for (point_index, point) in landmarks.iter_mut().enumerate() {
                for axis in 0..3 {
                    let (x_hat, dx_hat) =
                        axes[point_index * 3 + axis].filter_velocity(time, point[axis]);
                    // Constant-velocity dead reckoning across measured pipeline delay.
                    let gain = (0.55 + dx_hat.abs() * 0.12).clamp(0.45, 0.8);
                    point[axis] = x_hat + dx_hat * delay * gain;
                }
            }
            let world = hand.world.map(|raw| {
                let filters = &mut self.slots[index].world;
                let mut filtered = raw;
                for (point_index, point) in filtered.iter_mut().enumerate() {
                    for axis in 0..3 {
                        let (x_hat, dx_hat) =
                            filters[point_index * 3 + axis].filter_velocity(time, point[axis]);
                        let gain = (0.5 + dx_hat.abs() * 0.2).clamp(0.4, 0.75);
                        point[axis] = x_hat + dx_hat * delay * gain;
                    }
                }
                filtered
            });
            out.push(TrackedHand {
                track_id: hand.track_id,
                side: hand.side,
                score: hand.score,
                landmarks,
                world,
            });
        }
        let mut cursor = 0;
        self.slots.retain(|_| {
            let keep = used.get(cursor).copied().unwrap_or(false);
            cursor += 1;
            keep
        });
        out
    }

    /// Image-space tip velocity (landmark 8, x/y) after the last `follow` pass.
    fn tip_velocity(&self, track_id: u64) -> (f32, f32) {
        let Some(slot) = self
            .slots
            .iter()
            .find(|slot| slot.track_id != 0 && slot.track_id == track_id)
        else {
            return (0.0, 0.0);
        };
        let ix = 8 * 3;
        if slot.axes.len() <= ix + 1 {
            return (0.0, 0.0);
        }
        (slot.axes[ix].velocity(), slot.axes[ix + 1].velocity())
    }
}

struct Recorder {
    name: String,
    threshold: f32,
    frames: Vec<Vec<f32>>,
    started_ms: u64,
}

enum CalStep {
    Palm {
        open_sum: f32,
        samples: u32,
    },
    Point {
        held: u32,
    },
    Pinch {
        left: u32,
        ratios: Vec<f32>,
        gaps: Vec<f32>,
        down: bool,
        held_frames: u32,
    },
    Thumb {
        held: u32,
        gaps: Vec<f32>,
    },
    Zone {
        until_ms: u64,
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
        travel: f32,
        samples: u32,
        last: Option<(f32, f32)>,
    },
    Verify {
        stage: u8,
        held: u32,
        v_sum: f32,
        v_n: u32,
    },
    Drill {
        left: u32,
        gaps: Vec<f32>,
        ratios: Vec<f32>,
        down: bool,
        held_frames: u32,
        until_ms: u64,
    },
}

struct CalSession {
    step: CalStep,
    note: String,
    scale_sum: f32,
    span_sum: f32,
    open_ratio: f32,
    pinch_ratio: f32,
    pinch_gap: f32,
    thumb_gap: f32,
    v_spread: f32,
    palm_sum: [f32; 4],
    palm_n: u32,
    point_sum: [f32; 4],
    point_n: u32,
    thumb_sum: [f32; 4],
    thumb_n: u32,
}

async fn run(
    mut commands: mpsc::Receiver<EngineCommand>,
    snapshot: Arc<RwLock<Snapshot>>,
) -> Result<(), CoreError> {
    let (settings, calibration, mut profiles, custom, stats) = store::load_all();
    if !profiles.iter().any(|profile| profile.id == "explorer") {
        if let Some(explorer) = factory_profile("explorer") {
            profiles.push(explorer);
            let _ = store::save_profiles(&profiles);
        }
    }
    let context_rules = store::load_context_rules();
    let voice = voice::spawn_voice();
    let _ = voice.commands.try_send(voice::VoiceCommand::Prepare);
    let (input, ready) = spawn_input();
    let input_ok = match ready.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => true,
        Ok(Err(message)) => {
            tracing::error!(error = %message, "system input is unavailable");
            false
        }
        Err(_) => false,
    };
    let vision = spawn_vision();
    let mut session = Session {
        settings,
        calibration,
        profiles,
        custom,
        stats,
        started: Instant::now(),
        uptime_mark: 0,
        gates: GateSet::default(),
        pointer: PointerController::default(),
        pinch_held: false,
        pinch_sticky_until: 0,
        last_good_aim: None,
        last_good_anchor: None,
        stillness: Stillness::default(),
        tremor: TremorMonitor::default(),
        euro_tuner: EuroTuner::new(1.7, 0.9),
        custom_stats: CustomMatchStats::default(),
        dwell: DwellClick::default(),
        session_dump: None,
        mistakes: MistakeLog::default(),
        coach_line: String::new(),
        tracking_health: "трекинг в норме".into(),
        scene_hint: SceneHint::Ok,
        sharpness: 120.0,
        brightness: 128.0,
        flicker: 0.0,
        wanted_pose: String::new(),
        motion: MotionTracker::default(),
        memory: HandMemory::new(280),
        primary_pose: PoseHold::default(),
        secondary_pose: PoseHold::default(),
        last_wrists: Vec::new(),
        last_hand_ms: 0,
        history: VecDeque::new(),
        frame_times: VecDeque::new(),
        input: input_ok.then_some(input),
        input_ok,
        snapshot,
        logs: VecDeque::new(),
        control: false,
        paused: false,
        camera_on: false,
        released: true,
        vision_pid: 0,
        ipc_ok: false,
        camera_status: "Камера выключена".into(),
        error: if input_ok {
            String::new()
        } else {
            "Системный ввод недоступен. Камера и распознавание работают, управление курсором — нет."
                .into()
        },
        cameras: Vec::new(),
        calibration_session: None,
        recorder: None,
        latency_avg: 0.0,
        capture_ms: 0.0,
        inference_ms: 0.0,
        ipc_ms: 0.0,
        gesture_ms: 0.0,
        dispatch_ms: 0.0,
        fps: 0.0,
        cpu: 0.0,
        ram_mb: 0.0,
        system: sysinfo::System::new(),
        screen: input::virtual_screen(),
        screen_at: Instant::now(),
        context_checked: Instant::now() - Duration::from_secs(10),
        context_app: String::new(),
        context_profile: "desktop".into(),
        our_pid: std::process::id(),
        gesture: "—".into(),
        confidence: 0.0,
        command: "—".into(),
        command_status: "ожидание".into(),
        rejection: String::new(),
        hand_status: "Рука не видна".into(),
        settings_revision: 1,
        gestures_revision: 1,
        cameras_revision: 1,
        profiles_revision: 1,
        frame_revision: 0,
        preview: None,
        play_frame_revision: 0,
        play_preview: None,
        stage_cam: None,
        play_paint_tick: 0,
        ui_page: 0,
        last_stats_save: Instant::now(),
        stage: Stage::default(),
        stage_guard: false,
        scene_cursor: false,
        glide: HandGlide::default(),
        linker: HandLinker::default(),
        play_revision: 1,
        context_rules,
        ui_targets: Vec::new(),
        ui_targets_at: Instant::now() - Duration::from_secs(1),
        snap_target: String::new(),
        threshold_suggest: String::new(),
        suggested_confidence: 0.0,
        session_frames: 0,
        exam: None,
        voice,
        voice_listening: false,
        voice_ready: voice::voice_process_planned(),
        voice_line: String::new(),
        last_voice: Instant::now() - Duration::from_secs(2),
    };
    session.widen_frame();
    session.camera_status = "Ищем камеру…".into();
    session.log("AIR CONTROL готов. Камера запускается только по вашей команде.");
    session.publish();
    let mut ticker = tokio::time::interval(Duration::from_millis(250));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let VisionSystem {
        commands: vision_tx,
        mut tracking,
        mut events,
    } = vision;
    let _ = vision_tx.try_send(VisionCommand::Prepare);

    loop {
        tokio::select! {
            biased;
            command = commands.recv() => {
                match command {
                    Some(command) => {
                        if session.handle(command, &vision_tx).await == Flow::Stop {
                            break;
                        }
                    }
                    None => break,
                }
            }
            changed = tracking.changed() => {
                if changed.is_ok() {
                    let message = tracking.borrow().clone();
                    if let Some(message) = message {
                        session.on_tracking(message);
                    }
                }
            }
            event = events.recv() => {
                if let Some(event) = event {
                    session.on_vision(event);
                }
            }
            _ = ticker.tick() => session.on_tick(),
        }
    }
    session.control = false;
    session.send_input(InputMsg::ReleaseAll);
    session.send_input(InputMsg::Shutdown);
    session.flush_uptime();
    let _ = store::save_stats(&session.stats);
    let _ = store::save_settings(&session.settings);
    let _ = session.voice.commands.send(voice::VoiceCommand::Shutdown).await;
    let _ = vision_tx.send(VisionCommand::Shutdown).await;
    Ok(())
}

impl Session {
    async fn handle(
        &mut self,
        command: EngineCommand,
        vision: &mpsc::Sender<VisionCommand>,
    ) -> Flow {
        match command {
            EngineCommand::StartCamera => {
                self.camera_on = true;
                self.ipc_ok = false;
                self.camera_status = "Запуск камеры".into();
                self.error.clear();
                self.log("Камера включается с вашего разрешения.");
                self.arm_scene_cursor();
                let _ = vision
                    .send(VisionCommand::Start(self.vision_config()))
                    .await;
            }
            EngineCommand::StopCamera => {
                self.camera_on = false;
                self.control = false;
                self.stage.release_sound();
                self.send_input(InputMsg::ReleaseAll);
                self.camera_status = if self.cameras.is_empty() {
                    "Камера выключена".into()
                } else {
                    format!("Найдена: {}", self.camera_label())
                };
                let _ = vision.send(VisionCommand::Release).await;
                self.log("Камера остановлена, удержание кнопок снято.");
            }
            EngineCommand::RefreshCameras => {
                self.camera_status = "Ищем камеру…".into();
                self.error.clear();
                let _ = vision.send(VisionCommand::ListCameras).await;
                let _ = vision.send(VisionCommand::Prepare).await;
            }
            EngineCommand::OpenCameraSettings => {
                open_camera_privacy();
                self.log("Открыты параметры камеры.");
            }
            EngineCommand::SetControl(enabled) => {
                if enabled && self.stage.is_live() {
                    self.error = "Сначала вернитесь к курсору на странице «Сцена».".into();
                } else if enabled && !self.camera_on {
                    self.error = "Сначала включите камеру.".into();
                } else if enabled && !self.input_ok {
                    self.error = "Системный ввод недоступен.".into();
                } else {
                    self.control = enabled;
                    self.paused = false;
                    if !enabled {
                        self.send_input(InputMsg::ReleaseAll);
                    }
                    self.error.clear();
                    self.log(if enabled {
                        "Управление курсором включено."
                    } else {
                        "Управление курсором выключено."
                    });
                }
            }
            EngineCommand::TogglePause => {
                self.paused = !self.paused;
                if self.paused {
                    self.stop_voice();
                    self.send_input(InputMsg::ReleaseAll);
                    self.released = true;
                    self.log("Пауза. Мышь и клавиатура снова ваши.");
                } else {
                    self.log("Пауза снята.");
                }
            }
            EngineCommand::EmergencyStop => {
                self.control = false;
                self.paused = true;
                self.stop_voice();
                self.stage.release_sound();
                self.send_input(InputMsg::ReleaseAll);
                self.released = true;
                self.command_status = "остановлено".to_string();
                self.log("Аварийная остановка. Команды заблокированы, кнопки отпущены.");
            }
            EngineCommand::SetVoiceListen(enabled) => {
                if enabled && self.paused {
                    self.error = "Сначала снимите паузу.".into();
                } else if enabled && !self.input_ok {
                    self.error = "Системный ввод недоступен.".into();
                } else {
                    self.voice_listening = enabled;
                    self.error.clear();
                    let command = if enabled {
                        voice::VoiceCommand::Start
                    } else {
                        voice::VoiceCommand::Stop
                    };
                    let _ = self.voice.commands.try_send(command);
                    self.log(if enabled {
                        "Микрофон включается."
                    } else {
                        "Микрофон выключен."
                    });
                }
            }
            EngineCommand::SelectCamera(index) => {
                let index = index.max(0);
                self.settings.camera_index = index;
                for camera in &mut self.cameras {
                    camera.selected = camera.index == index;
                }
                self.cameras_revision = self.cameras_revision.wrapping_add(1);
                self.persist_settings();
                if !self.camera_on && !self.cameras.is_empty() {
                    self.camera_status = format!("Найдена: {}", self.camera_label());
                }
                if self.camera_on {
                    let _ = vision
                        .send(VisionCommand::Update(self.vision_config()))
                        .await;
                }
            }
            EngineCommand::SetQuality(index) => {
                self.settings.quality = QualityPreset::from_index(index);
                self.persist_settings();
                if self.camera_on {
                    let _ = vision
                        .send(VisionCommand::Update(self.vision_config()))
                        .await;
                }
            }
            EngineCommand::SetMirror(value) => {
                self.settings.mirror = value;
                self.persist_settings();
                if self.camera_on {
                    let _ = vision
                        .send(VisionCommand::Update(self.vision_config()))
                        .await;
                }
            }
            EngineCommand::SetShowCamera(value) => {
                self.settings.show_camera = value;
                self.persist_settings();
                if self.camera_on {
                    let _ = vision
                        .send(VisionCommand::Update(self.vision_config()))
                        .await;
                }
            }
            EngineCommand::SetPrimaryHand(index) => {
                self.settings.primary_hand = PrimaryHand::from_index(index);
                self.persist_settings();
            }
            EngineCommand::SetSensitivityProfile(index) => {
                self.settings.sensitivity = SensitivityProfile::from_index(index);
                self.persist_settings();
            }
            EngineCommand::SetSpeed(value) => {
                self.settings.speed = value.clamp(0.6, 2.2);
                self.persist_settings();
            }
            EngineCommand::SetSmoothness(value) => {
                self.settings.smoothness = value.clamp(0.0, 1.0);
                self.persist_settings();
            }
            EngineCommand::SetAcceleration(value) => {
                self.settings.acceleration = value.clamp(0.0, 1.0);
                self.persist_settings();
            }
            EngineCommand::SetDeadzone(value) => {
                self.settings.deadzone = value.clamp(0.0, 12.0);
                self.persist_settings();
            }
            EngineCommand::SetConfidence(value) => {
                self.settings.confidence = value.clamp(0.4, 0.95);
                self.persist_settings();
            }
            EngineCommand::SetProfile(id) => {
                if self.profiles.iter().any(|profile| profile.id == id) {
                    self.coach_line = profile_coach_line(&id).into();
                    self.settings.profile = id;
                    self.settings.auto_context = false;
                    self.persist_settings();
                    self.profiles_revision = self.profiles_revision.wrapping_add(1);
                    self.gestures_revision = self.gestures_revision.wrapping_add(1);
                }
            }
            EngineCommand::SetAutoContext(value) => {
                self.settings.auto_context = value;
                self.persist_settings();
                self.profiles_revision = self.profiles_revision.wrapping_add(1);
                self.gestures_revision = self.gestures_revision.wrapping_add(1);
            }
            EngineCommand::SetAirKeyboard(value) => {
                self.settings.air_keyboard = value;
                self.persist_settings();
                self.gestures_revision = self.gestures_revision.wrapping_add(1);
                self.log(if value {
                    "Воздушная клавиатура включена."
                } else {
                    "Воздушная клавиатура выключена."
                });
            }
            EngineCommand::SetDwellClick(value) => {
                self.settings.dwell_click = value;
                self.dwell.clear();
                self.persist_settings();
                self.log(if value {
                    "Клик удержанием включён: держите указательный ~0.6 с."
                } else {
                    "Клик удержанием выключен."
                });
            }
            EngineCommand::SetSnapUi(value) => {
                self.settings.snap_ui = value;
                self.persist_settings();
                self.log(if value {
                    "Прилипание к кнопкам включено."
                } else {
                    "Прилипание к кнопкам выключено."
                });
            }
            EngineCommand::SetSnapRadius(value) => {
                self.settings.snap_radius_px = value.clamp(12.0, 80.0);
                self.persist_settings();
            }
            EngineCommand::SetDepthCursor(value) => {
                self.settings.depth_cursor = value;
                self.persist_settings();
                self.log(if value {
                    "Учёт глубины курсора включён."
                } else {
                    "Учёт глубины курсора выключен."
                });
            }
            EngineCommand::SetDepthGain(value) => {
                self.settings.depth_gain = value.clamp(0.3, 1.5);
                self.persist_settings();
            }
            EngineCommand::AddContextRule {
                pattern,
                profile_id,
            } => {
                let pattern = pattern.trim().to_string();
                if pattern.is_empty() || pattern.len() > 64 {
                    self.error = "Шаблон имени программы слишком короткий или длинный.".into();
                } else if !self.profiles.iter().any(|profile| profile.id == profile_id) {
                    self.error = "Такого профиля нет.".into();
                } else {
                    self.context_rules.retain(|rule| {
                        rule.pattern.to_ascii_lowercase() != pattern.to_ascii_lowercase()
                    });
                    self.context_rules.push(ContextRule {
                        pattern,
                        profile_id,
                        enabled: true,
                    });
                    let _ = store::save_context_rules(&self.context_rules);
                    self.profiles_revision = self.profiles_revision.wrapping_add(1);
                    self.error.clear();
                    self.log("Правило контекста сохранено.");
                }
            }
            EngineCommand::ToggleContextRule { pattern } => {
                if let Some(rule) = self
                    .context_rules
                    .iter_mut()
                    .find(|rule| rule.pattern.eq_ignore_ascii_case(&pattern))
                {
                    rule.enabled = !rule.enabled;
                    let _ = store::save_context_rules(&self.context_rules);
                    self.profiles_revision = self.profiles_revision.wrapping_add(1);
                }
            }
            EngineCommand::RemoveContextRule { pattern } => {
                self.context_rules
                    .retain(|rule| !rule.pattern.eq_ignore_ascii_case(&pattern));
                let _ = store::save_context_rules(&self.context_rules);
                self.profiles_revision = self.profiles_revision.wrapping_add(1);
                self.log("Правило удалено.");
            }
            EngineCommand::ResetProfile(id) => {
                if let Some(factory) = factory_profile(&id) {
                    if let Some(slot) = self.profiles.iter_mut().find(|profile| profile.id == id) {
                        *slot = factory;
                    } else {
                        self.profiles.push(factory);
                    }
                    let _ = store::save_profiles(&self.profiles);
                    self.profiles_revision = self.profiles_revision.wrapping_add(1);
                    self.gestures_revision = self.gestures_revision.wrapping_add(1);
                    self.log(&format!("Профиль «{id}» сброшен к заводскому."));
                }
            }
            EngineCommand::SetProfileTune {
                profile_id,
                override_tune,
                speed,
                smoothness,
                deadzone,
                confidence,
            } => {
                if let Some(profile) = self
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == profile_id)
                {
                    profile.tune.override_tune = override_tune;
                    profile.tune.speed = speed.clamp(0.4, 2.5);
                    profile.tune.smoothness = smoothness.clamp(0.0, 1.0);
                    profile.tune.deadzone = deadzone.clamp(0.0, 12.0);
                    profile.tune.confidence = confidence.clamp(0.4, 0.95);
                    let _ = store::save_profiles(&self.profiles);
                    self.profiles_revision = self.profiles_revision.wrapping_add(1);
                    self.settings_revision = self.settings_revision.wrapping_add(1);
                }
            }
            EngineCommand::SuggestThresholds => self.suggest_from_session(),
            EngineCommand::ApplySuggestedConfidence(value) => {
                self.settings.confidence = value.clamp(0.4, 0.95);
                self.persist_settings();
                self.log(&format!(
                    "Уверенность обновлена до {:.2}.",
                    self.settings.confidence
                ));
            }
            EngineCommand::StartExam => self.begin_exam(),
            EngineCommand::ToggleSessionRecord => {
                if let Some(recorder) = self.session_dump.take() {
                    match recorder.finish() {
                        Ok(path) => {
                            self.log(&format!("Запись сессии сохранена: {}", path.display()))
                        }
                        Err(err) => self.error = err.to_string(),
                    }
                } else {
                    match SessionRecorder::start() {
                        Ok(recorder) => {
                            self.log(&format!(
                                "Запись сессии начата → {}",
                                recorder.path().display()
                            ));
                            self.session_dump = Some(recorder);
                        }
                        Err(err) => self.error = err.to_string(),
                    }
                }
            }
            EngineCommand::CalibrationStart => {
                self.calibration_session = Some(CalSession {
                    step: CalStep::Palm {
                        open_sum: 0.0,
                        samples: 0,
                    },
                    note: "Тест 1. Раскройте ладонь и держите её спокойно. Большой палец не поднимайте.".into(),
                    scale_sum: 0.0,
                    span_sum: 0.0,
                    open_ratio: 0.0,
                    pinch_ratio: 0.0,
                    pinch_gap: 0.0,
                    thumb_gap: 0.0,
                    v_spread: 0.0,
                    palm_sum: [0.0; 4],
                    palm_n: 0,
                    point_sum: [0.0; 4],
                    point_n: 0,
                    thumb_sum: [0.0; 4],
                    thumb_n: 0,
                });
                self.log("Подстройка под вашу руку: ладонь, указательный, щипок, большой, зона, проверка и тренировка.");
            }
            EngineCommand::CalibrationNext => self.advance_calibration(),
            EngineCommand::CalibrationCancel => {
                self.calibration_session = None;
                self.wanted_pose.clear();
                self.error.clear();
                self.log("Калибровка отменена. Сохранённый профиль не изменён.");
            }
            EngineCommand::CalibrationDrill => {
                if self.calibration_session.is_some() {
                    self.error = "Сначала закончите текущую подстройку.".into();
                } else {
                    self.start_pinch_drill(true);
                }
            }
            EngineCommand::RecordGesture { name, threshold } => {
                if !valid_name(&name) || BUILTIN_GESTURES.contains(&name.as_str()) {
                    self.error =
                        "Имя жеста должно быть своим, до 32 символов, без служебных названий."
                            .into();
                } else {
                    self.recorder = Some(Recorder {
                        name,
                        threshold: threshold.clamp(0.15, 1.2),
                        frames: Vec::new(),
                        started_ms: self.now_ms(),
                    });
                    self.error.clear();
                    self.log("Запись жеста начата. Повторите движение около секунды.");
                }
            }
            EngineCommand::DeleteGesture(name) => {
                let before = self.custom.len();
                self.custom.retain(|gesture| gesture.name != name);
                if self.custom.len() != before {
                    for profile in &mut self.profiles {
                        profile.mappings.retain(|(id, _)| id != &name);
                    }
                    let _ = store::save_gestures(&self.custom);
                    let _ = store::save_profiles(&self.profiles);
                    self.gestures_revision = self.gestures_revision.wrapping_add(1);
                    self.log(&format!("Жест «{name}» удалён."));
                }
            }
            EngineCommand::AssignCommand { gesture, label } => {
                self.assign_command(&gesture, &label)
            }
            EngineCommand::ExportGestures(path) => {
                match store::export_gestures(&path, &self.custom, &self.profiles) {
                    Ok(()) => self.log(&format!("Жесты экспортированы в {}", path.display())),
                    Err(err) => self.error = err.to_string(),
                }
            }
            EngineCommand::ImportGestures(path) => {
                match store::import_gestures(&path, &mut self.custom, &mut self.profiles) {
                    Ok(count) => {
                        let _ = store::save_gestures(&self.custom);
                        let _ = store::save_profiles(&self.profiles);
                        self.gestures_revision = self.gestures_revision.wrapping_add(1);
                        self.log(&format!(
                            "Импортировано жестов: {count}. Существующие другие жесты сохранены."
                        ));
                    }
                    Err(err) => self.error = err.to_string(),
                }
            }
            EngineCommand::SetPlayMode(mode) => {
                let was_live = self.stage.is_live();
                self.stage.set_mode(mode);
                self.play_revision = self.play_revision.wrapping_add(1);
                if self.stage.is_live() {
                    self.scene_cursor = false;
                    if self.control || !was_live {
                        self.send_input(InputMsg::ReleaseAll);
                        self.released = true;
                        self.control = false;
                        self.stage_guard = true;
                        self.log("Сцена включена. Курсор слушает обычную мышь.");
                    }
                } else {
                    self.scene_cursor = true;
                    self.stage_guard = false;
                    if self.camera_on {
                        self.arm_scene_cursor();
                    } else {
                        self.log("Сначала включите камеру — тогда курсор поедет за пальцем.");
                    }
                }
                if self.camera_on {
                    let _ = vision
                        .send(VisionCommand::Update(self.vision_config()))
                        .await;
                }
            }
            EngineCommand::SetPlayInstrument(index) => {
                self.stage.set_instrument(index);
                self.play_revision = self.play_revision.wrapping_add(1);
                if self.camera_on {
                    let _ = vision
                        .send(VisionCommand::Update(self.vision_config()))
                        .await;
                }
            }
            EngineCommand::SetPlayElement(index) => {
                self.stage.set_element(index);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetPlayBrush(index) => {
                self.stage.set_brush(index);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetPlayColor(index) => {
                self.stage.set_color(index);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetPlayPreset(index) => {
                self.stage.set_preset(index);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetGuitarChord(index) => {
                self.stage.set_guitar_chord(index);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetGuitarMode(mode) => {
                self.stage.set_guitar_mode(mode);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetGuitarCapo(capo) => {
                self.stage.set_guitar_capo(capo);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::PluckGuitarString(string) => {
                self.stage.pluck_guitar_string(string, self.now_ms());
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetPlayMuted(muted) => {
                self.stage.set_muted(muted);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetPlayVolume(value) => {
                self.stage.set_volume(value);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetPlaySensitivity(value) => {
                self.stage.set_sensitivity(value);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetPlayThickness(value) => {
                self.stage.set_thickness(value);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::SetPlayFade(value) => {
                self.stage.set_fade(value);
                self.play_revision = self.play_revision.wrapping_add(1);
            }
            EngineCommand::ClearArt => {
                self.stage.clear_art();
                self.play_revision = self.play_revision.wrapping_add(1);
                self.log("Рисунок стёрт.");
            }
            EngineCommand::SetUiPage(page) => {
                let page = page.max(0);
                if self.ui_page != page {
                    let prev = self.ui_page;
                    self.ui_page = page;
                    if prev == 12 && page != 12 {
                        if self.play_preview.is_some() || self.stage_cam.is_some() {
                            self.play_preview = None;
                            self.stage_cam = None;
                            self.play_frame_revision = self.play_frame_revision.wrapping_add(1);
                        }
                    }
                    if !self.preview_page_visible() && self.preview.is_some() {
                        self.preview = None;
                        self.frame_revision = self.frame_revision.wrapping_add(1);
                    }
                    if self.camera_on {
                        let _ = vision
                            .send(VisionCommand::Update(self.vision_config()))
                            .await;
                    }
                    self.publish();
                }
            }
            EngineCommand::Note(message) => self.log(&message),
            EngineCommand::Shutdown => {
                self.stage.release_sound();
                return Flow::Stop;
            }
        }
        self.publish();
        Flow::Continue
    }

    fn on_vision(&mut self, event: VisionEvent) {
        match event {
            VisionEvent::Hello { pid, mediapipe } => {
                self.vision_pid = pid;
                self.ipc_ok = true;
                if mediapipe == "starting" && !self.camera_on {
                    self.camera_status = "Загружается распознавание руки…".into();
                }
                self.log(&format!("Процесс зрения {pid}, MediaPipe {mediapipe}."));
            }
            VisionEvent::Cameras(devices) => {
                if !devices.is_empty() {
                    let ranked: Vec<(i32, String)> = devices
                        .iter()
                        .map(|device| (device.index, device.name.clone()))
                        .collect();
                    let chosen = preferred_camera_index(&ranked, self.settings.camera_index);
                    if chosen != self.settings.camera_index {
                        self.settings.camera_index = chosen;
                        self.persist_settings();
                    }
                }
                self.cameras = devices
                    .into_iter()
                    .map(|device| CameraInfo {
                        index: device.index,
                        label: device.name,
                        selected: device.index == self.settings.camera_index,
                    })
                    .collect();
                self.cameras_revision = self.cameras_revision.wrapping_add(1);
                if !self.camera_on {
                    if self.cameras.is_empty() {
                        self.camera_status = "Камера не найдена".into();
                        self.error = if cfg!(target_os = "macos") {
                            "Камера не ответила. Разрешите доступ в Системные настройки → Конфиденциальность → Камера, закройте другие приложения с камерой и нажмите «Проверить ещё раз».".into()
                        } else {
                            "Windows не видит съёмный модуль. Снимите камеру Honor с верхней рамки и поставьте снова до щелчка. Магнит может держать корпус, а контакты — нет.".into()
                        };
                    } else {
                        self.camera_status = format!("Найдена: {}", self.camera_label());
                        self.error.clear();
                    }
                }
            }
            VisionEvent::Status {
                state,
                message,
                width,
                height,
                fps,
            } => {
                self.ipc_ok = true;
                self.camera_status = match state.as_str() {
                    "streaming" => "Картинка идёт".into(),
                    "reconnecting" => "Камера не открылась. Пробуем ещё раз.".into(),
                    "opening" | "warming_up" => "Включаем камеру…".into(),
                    "waiting" if !self.camera_on => self.camera_status.clone(),
                    "waiting" => "Камера выключена".into(),
                    other => format!("{other} {message}"),
                };
                if state == "streaming" {
                    self.error.clear();
                    // Ensure the picker has at least the live device.
                    if self.cameras.is_empty() {
                        self.cameras = vec![CameraInfo {
                            index: self.settings.camera_index,
                            label: self.camera_label(),
                            selected: true,
                        }];
                        self.cameras_revision = self.cameras_revision.wrapping_add(1);
                    }
                }
                let _ = (width, height, fps);
                if !message.is_empty() && state != "streaming" {
                    self.log(&format!("{state}: {message}"));
                }
            }
            VisionEvent::Error { code, message } => {
                self.error = message.clone();
                self.log(&format!("{code}: {message}"));
            }
            VisionEvent::Restarting { reason } => {
                self.stats.camera_reconnects = self.stats.camera_reconnects.saturating_add(1);
                self.camera_status = "Перезапуск зрения".into();
                self.log(&reason);
            }
            VisionEvent::Failed { message } => {
                self.error = message.clone();
                self.camera_on = false;
                self.control = false;
                self.send_input(InputMsg::ReleaseAll);
                self.log(&message);
            }
            VisionEvent::Exited { code } => {
                self.ipc_ok = false;
                self.vision_pid = 0;
                self.log(&format!("Процесс зрения завершился: {code:?}"));
            }
            VisionEvent::Log(line) if line != "heartbeat" => {
                if !line.is_empty() {
                    self.log(&line);
                }
            }
            VisionEvent::Log(_) => self.ipc_ok = true,
        }
        self.publish();
    }

    fn on_tick(&mut self) {
        if self.screen_at.elapsed() > Duration::from_secs(2) {
            self.screen = input::virtual_screen();
            self.screen_at = Instant::now();
        }
        if self.context_checked.elapsed() > Duration::from_millis(400) {
            let (pid, name) = input::foreground_process();
            self.context_app = if name.is_empty() {
                "не определено".into()
            } else {
                name.clone()
            };
            if self.settings.auto_context {
                let next = if pid == self.our_pid || name.is_empty() {
                    self.context_profile.clone()
                } else {
                    resolve_profile_id(&name, &self.context_rules)
                };
                if next != self.context_profile {
                    self.context_profile = next.clone();
                    self.profiles_revision = self.profiles_revision.wrapping_add(1);
                    self.gestures_revision = self.gestures_revision.wrapping_add(1);
                    self.coach_line = profile_coach_line(&next).into();
                }
            }
            self.context_checked = Instant::now();
        }
        self.system.refresh_memory();
        self.system.refresh_cpu_usage();
        self.cpu = self.system.global_cpu_usage();
        self.ram_mb = self.system.used_memory() as f32 / (1024.0 * 1024.0);
        if self.paused && !self.released {
            self.send_input(InputMsg::ReleaseAll);
            self.released = true;
        }
        if self.last_stats_save.elapsed() > Duration::from_secs(30) {
            self.flush_uptime();
            let _ = store::save_stats(&self.stats);
            self.last_stats_save = Instant::now();
        }
        self.advance_range_timer();
        while let Ok(event) = self.voice.events.try_recv() {
            match event {
                voice::VoiceEvent::Log(message) => {
                    if !message.is_empty() {
                        self.log(&message);
                    }
                }
                voice::VoiceEvent::Status {
                    ready,
                    listening,
                    message,
                } => {
                    if let Some(ready) = ready {
                        self.voice_ready = ready;
                    }
                    if let Some(listening) = listening {
                        self.voice_listening = listening;
                    }
                    if !message.is_empty() {
                        self.log(&format!("Голос: {message}"));
                    }
                }
                voice::VoiceEvent::Intent(intent) => self.accept_voice(intent),
                voice::VoiceEvent::Failed { message, ready } => {
                    let asked = self.voice_listening;
                    self.voice_listening = false;
                    if let Some(ready) = ready {
                        self.voice_ready = ready;
                    }
                    self.voice_line = message.clone();
                    if asked {
                        self.error = message;
                    } else {
                        self.log(&message);
                    }
                }
            }
        }
        self.publish();
    }

    fn on_tracking(&mut self, message: TrackingMessage) {
        let started = Instant::now();
        let now = self.now_ms();
        self.ipc_ok = true;
        self.capture_ms = message.capture_ms;
        self.inference_ms = message.inference_ms;
        self.sharpness = message.sharpness;
        self.brightness = message.brightness;
        self.flicker = message.flicker;
        let wall = wall_ms();
        self.ipc_ms = wall.saturating_sub(message.timestamp_ms).min(5_000) as f32;
        // Prefer capture-clock stamps so dropped frames keep honest dt / fps.
        let frame_clock = if message.frame_ms > 0 {
            message.frame_ms
        } else {
            now
        };
        self.frame_times.push_back(frame_clock);
        while self.frame_times.len() > 30 {
            self.frame_times.pop_front();
        }
        if let (Some(first), Some(last)) = (self.frame_times.front(), self.frame_times.back()) {
            let span = last.saturating_sub(*first);
            if span > 0 && self.frame_times.len() > 1 {
                self.fps = (self.frame_times.len() - 1) as f32 * 1000.0 / span as f32;
            }
        }
        let (quality_mult, quality_hint) =
            quality_gate(message.sharpness, message.brightness, message.flicker);
        self.scene_hint = quality_hint;
        self.tracking_health = tracking_health(
            message.sharpness,
            message.brightness,
            message.flicker,
            self.fps,
        );
        if let Some(recorder) = self.session_dump.as_mut() {
            let frame = SessionFrame {
                ts: message.timestamp_ms,
                frame_ms: message.frame_ms,
                sharpness: message.sharpness,
                brightness: message.brightness,
                flicker: message.flicker,
                hands: message.hands.iter().map(SessionHand::from).collect(),
            };
            let _ = recorder.push(&frame);
            self.session_frames = recorder.frames();
        }

        let incoming = message
            .hands
            .iter()
            .map(|hand| TrackedHand {
                track_id: 0,
                side: if hand.score < 0.72 {
                    HandSide::Unknown
                } else {
                    HandSide::parse(&hand.handedness)
                },
                score: hand.score,
                landmarks: hand.landmarks,
                world: hand.world,
            })
            .collect();
        let incoming = steady_hands(
            &self.last_wrists,
            incoming,
            now.saturating_sub(self.last_hand_ms),
        );
        let incoming = self.linker.link(incoming);
        let visible = self.memory.update(now, incoming);
        let anchor = self.last_wrists.first().copied();
        let hands = select_hands(&visible, self.settings.primary_hand, anchor);
        if hands.is_empty() {
            self.last_wrists.clear();
        } else {
            self.last_wrists = hands
                .iter()
                .map(|hand| (hand.landmarks[0][0], hand.landmarks[0][1]))
                .collect();
            self.last_hand_ms = now;
        }
        self.observe_calibration(&hands, now);
        self.observe_recording(&hands, now);
        self.observe_exam(&hands, now);
        if self.settings.snap_ui && self.ui_targets_at.elapsed() > Duration::from_millis(140) {
            self.ui_targets = uia::poll_targets();
            self.ui_targets_at = Instant::now();
        }

        let profile_id = self.active_profile_id();
        let live_scale = hands
            .first()
            .map(|hand| distance(hand.landmarks[0], hand.landmarks[9]))
            .unwrap_or(0.0);
        let gates = PoseGates {
            extend: self.calibration.extend_gate(),
            curl: self.calibration.curl_gate(),
            v_spread: self.calibration.v_gate(),
            distance: self.calibration.distance_boost(live_scale),
        };
        let instant = hands.first().and_then(|hand| {
            classify_hand_gated(
                hand,
                self.calibration.pinch_limit(),
                self.calibration.gap_limit(),
                self.remembered(),
                gates,
            )
        });
        let secondary_instant = hands.get(1).and_then(|hand| {
            classify_hand_gated(
                hand,
                self.calibration.pinch_limit(),
                self.calibration.gap_limit(),
                self.remembered(),
                gates,
            )
        });
        let pose = self.primary_pose.settle(instant.clone());
        let secondary = self.secondary_pose.settle(secondary_instant.clone());
        let mut motion = self.motion.push(now, &hands);
        let primary_points = instant
            .as_ref()
            .is_some_and(|item| item.id == "index_point" || item.id == "pinch");
        if primary_points {
            motion.clear();
        }
        if let Some(hand) = hands.first() {
            let wrist = (hand.landmarks[0][0], hand.landmarks[0][1]);
            self.stillness.push(wrist);
            self.tremor.push(now, wrist);
            self.history.push_back(normalize_landmarks(&hand.landmarks));
            while self.history.len() > 12 {
                self.history.pop_front();
            }
            if self.stillness.calm() {
                self.euro_tuner.observe(self.stillness.residual_std());
                let (cutoff, beta) = self.euro_tuner.params();
                self.glide.apply_params(cutoff, beta);
            }
        } else {
            self.stillness.clear();
            self.tremor.clear();
            self.history.clear();
            self.dwell.clear();
        }
        let hand_calm = self.stillness.calm();
        if let Some(line) = self.tremor.check(now) {
            self.coach_line = line.into();
            if self.control && !self.paused {
                self.paused = true;
                self.send_input(InputMsg::ReleaseAll);
                self.released = true;
                self.log(line);
            }
        }

        let hand_score = hands.first().map(|hand| hand.score).unwrap_or(0.0);
        let penalty = hands
            .first()
            .map(|hand| edge_penalty(&hand.landmarks))
            .unwrap_or(0.0);
        let span_penalty = self.span_penalty(hands.first());
        let prefer = match self.settings.primary_hand {
            PrimaryHand::Left => "Левая",
            PrimaryHand::Right => "Правая",
            PrimaryHand::Any => "Ведущая",
        };
        self.hand_status = match hands.as_slice() {
            [] => "Рука не видна".into(),
            [one] => match one.side {
                HandSide::Unknown => format!("{prefer} рука ведёт курсор"),
                side => format!("{} рука ведёт курсор", side.label()),
            },
            [primary, secondary] => format!(
                "{} — курсор · {} — громкость и свайп",
                primary.side.label(),
                secondary.side.label()
            ),
            _ => "Две руки".into(),
        };

        let mut label = pose
            .as_ref()
            .map(|item| item.id.to_string())
            .unwrap_or_else(|| "—".into());
        let mut confidence = pose
            .as_ref()
            .map(|item| item.confidence * (0.55 + 0.45 * hand_score) - penalty - span_penalty)
            .unwrap_or(0.0)
            .clamp(0.0, 0.99);
        confidence *= quality_mult;
        if penalty < 0.1 && self.primary_pose.confirmed() && quality_mult > 0.7 {
            confidence = confidence.max(0.86 * quality_mult);
        }
        self.rejection.clear();
        if quality_mult < 0.55 {
            self.rejection = "tracking".into();
        }
        if penalty >= 0.25 && pose.is_some() {
            self.rejection = "edge".into();
        }
        let pose_needs_still = pose.as_ref().is_some_and(|item| {
            matches!(
                item.id,
                "v_sign" | "thumb_up" | "thumb_down" | "open_palm" | "closed_fist"
            )
        });
        if pose_needs_still && !hand_calm {
            self.rejection = "shake".into();
            confidence *= 0.55;
        }
        if !self.wanted_pose.is_empty() {
            let seen = pose.as_ref().map(|item| item.id).unwrap_or("—");
            self.mistakes.note(&self.wanted_pose, seen);
        }
        if self.coach_line.is_empty() || !self.tremor.resting(now) {
            self.coach_line = self.mistakes.summary();
        }
        if self.coach_line.is_empty() {
            let hint = scene_hint_text(self.scene_hint);
            if !hint.is_empty() {
                self.coach_line = hint.into();
            }
        }

        let shown = self.glide.follow(frame_clock, &hands, self.latency_avg);
        let stage_live = self.stage.is_live();
        // Run instrument/magic/art only on the page that displays them — saves CPU elsewhere.
        let stage_active = stage_live && self.stage_page_visible();
        if stage_live {
            if !self.stage_guard {
                self.send_input(InputMsg::ReleaseAll);
                self.released = true;
                self.control = false;
                self.stage_guard = true;
            }
            if stage_active {
                self.stage.step(now, &shown, self.calibration.pinch_limit());
                label = self.stage.label();
                self.command = self.stage.detail();
                self.command_status = "сцена".into();
                self.rejection.clear();
            }
        }

        let collapsed = hands
            .first()
            .is_some_and(|hand| hand_collapsed(&hand.landmarks, self.calibration.finger_span));
        let was_held = self.pinch_held || now < self.pinch_sticky_until;
        let geometric_tap = !collapsed
            && hands
                .first()
                .is_some_and(|hand| index_press_active(hand, was_held));
        if geometric_tap {
            self.pinch_held = true;
            self.pinch_sticky_until = now.saturating_add(140);
        } else if now < self.pinch_sticky_until && !hands.is_empty() && !collapsed {
            self.pinch_held = true;
        } else if collapsed && was_held && now < self.pinch_sticky_until {
            self.pinch_held = true;
        } else {
            self.pinch_held = false;
            self.pinch_sticky_until = 0;
        }
        let pinch_is_click = self.pinch_held;
        if pinch_is_click {
            motion.clear();
            label = "index_tap".into();
            confidence = confidence.max(0.84);
        }
        let (point, anchor) = if collapsed {
            (self.last_good_aim, self.last_good_anchor.or(self.last_good_aim))
        } else {
            let point = hands.first().map(|hand| {
                if self.settings.depth_cursor && hand.score >= 0.72 {
                    aim_point_depth(
                        &hand.landmarks,
                        hand.world.as_ref(),
                        self.settings.depth_gain,
                    )
                } else {
                    aim_point(&hand.landmarks)
                }
            });
            let anchor = hands.first().map(|hand| pinch_anchor(&hand.landmarks));
            if point.is_some() {
                self.last_good_aim = point;
                self.last_good_anchor = anchor;
            }
            (point, anchor)
        };
        if hands.is_empty() {
            self.last_good_aim = None;
            self.last_good_anchor = None;
        }
        let tip_vel = hands
            .first()
            .map(|hand| self.glide.tip_velocity(hand.track_id))
            .unwrap_or((0.0, 0.0));
        let depth_gain = if self.settings.depth_cursor {
            let world_z = hands
                .first()
                .and_then(|hand| hand.world.as_ref())
                .map(|world| world[0][2]);
            cursor_distance_gain(
                live_scale,
                self.calibration.hand_scale,
                world_z,
                self.settings.depth_gain,
            )
        } else {
            1.0
        };
        let tracking = !stage_live
            && self.control
            && !self.paused
            && self.input_ok
            && point.is_some();
        let (speed, smoothness, deadzone, conf_floor) = self.active_pointer_tune();
        let click_allowed =
            self.input_ok && (pinch_is_click || confidence >= conf_floor * 0.45);
        let dispatch_started = Instant::now();
        let update = if tracking {
            self.pointer.update(&PointerInput {
                now_ms: now,
                tracking: true,
                point,
                pinch: pinch_is_click,
                click_allowed,
                anchor,
                screen: self.screen,
                range: self.calibration.range(),
                speed,
                acceleration: self.settings.acceleration,
                deadzone,
                smoothness,
                profile: self.settings.sensitivity,
                depth_gain,
                tip_vel,
                latency_ms: self.latency_avg,
            })
        } else {
            self.pointer.update(&PointerInput {
                now_ms: now,
                tracking: false,
                point: None,
                pinch: false,
                click_allowed: false,
                anchor: None,
                screen: self.screen,
                range: self.calibration.range(),
                speed,
                acceleration: self.settings.acceleration,
                deadzone,
                smoothness,
                profile: self.settings.sensitivity,
                depth_gain: 1.0,
                tip_vel: (0.0, 0.0),
                latency_ms: 0.0,
            })
        };
        if update.double_pinch {
            label = "double_pinch".into();
            if self.control && !self.paused {
                if let Some((x, y)) = update.repeat_at {
                    self.send_input(InputMsg::Move(x, y));
                }
                self.command = "Двойной клик".into();
                self.command_status = "выполнено".into();
                if !update.quick_pair {
                    self.send_input(InputMsg::DoubleClick);
                    self.stats.clicks = self.stats.clicks.saturating_add(1);
                    self.stats.commands_dispatched =
                        self.stats.commands_dispatched.saturating_add(1);
                }
            }
        }
        for event in &update.events {
            if self.control && !self.paused {
                if update.double_pinch && matches!(event, PointerEvent::Move(_, _)) {
                    continue;
                }
                if update.double_pinch
                    && !update.quick_pair
                    && matches!(event, PointerEvent::ClickLeft | PointerEvent::PressLeft)
                {
                    continue;
                }
                match event {
                    PointerEvent::ClickLeft => {
                        if let Some((x, y)) = update.repeat_at {
                            self.click_with_snap(x, y);
                        } else {
                            self.send_input(InputMsg::ClickLeft);
                        }
                    }
                    PointerEvent::PressLeft => {
                        if let Some((x, y)) = update.repeat_at {
                            let (sx, sy, name) = self.snap_coords(x, y);
                            self.send_input(InputMsg::Move(sx, sy));
                            if !name.is_empty() && self.coach_line.is_empty() {
                                self.coach_line = format!("прилипло: {name}");
                            }
                        }
                        self.command = "Зажато".into();
                        self.command_status = "удержание".into();
                        self.send_input(InputMsg::PressLeft);
                    }
                    other => self.apply_pointer(other),
                }
            }
        }
        if update.tap && self.control && !self.paused {
            self.stats.clicks = self.stats.clicks.saturating_add(1);
            self.stats.commands_dispatched = self.stats.commands_dispatched.saturating_add(1);
            self.command = "Левый клик".into();
            self.command_status = "выполнено".into();
        }

        if self.settings.dwell_click && tracking && !pinch_is_click && !self.tremor.resting(now) {
            if let Some(tip) = point {
                if self.dwell.tick(now, tip) {
                    if let Some((x, y)) = update.repeat_at {
                        self.click_with_snap(x, y);
                    } else {
                        self.send_input(InputMsg::ClickLeft);
                    }
                    self.stats.clicks = self.stats.clicks.saturating_add(1);
                    self.stats.commands_dispatched =
                        self.stats.commands_dispatched.saturating_add(1);
                    label = "dwell_click".into();
                    self.command = "Клик удержанием".into();
                    self.command_status = "выполнено".into();
                    confidence = confidence.max(0.9);
                }
            }
        } else if !self.settings.dwell_click {
            self.dwell.clear();
        }

        if !stage_live && self.control && !self.paused && self.input_ok {
            let mut active = Vec::new();
            if primary_points {
                if let Some(item) = secondary.clone() {
                    if item.id == "thumb_up" || item.id == "thumb_down" {
                        let command = if item.id == "thumb_up" {
                            CommandSpec::VolumeUp
                        } else {
                            CommandSpec::VolumeDown
                        };
                        active.push(item.id.to_string());
                        self.fire(item.id, command, confidence.max(0.8), 160, 400, now);
                    }
                }
            }
            if let Some(item) = pose.clone() {
                let shaky = self.rejection == "shake"
                    && matches!(
                        item.id,
                        "v_sign" | "thumb_up" | "thumb_down" | "open_palm" | "closed_fist"
                    );
                if item.id != "index_point" && item.id != "pinch" && !pinch_is_click && !shaky {
                    active.push(item.id.to_string());
                    self.fire_mapped(&profile_id, item.id, confidence, now);
                }
            }
            for event in motion {
                active.push(event.id().to_string());
                self.fire_mapped(&profile_id, event.id(), confidence.max(0.8), now);
            }
            if self.recorder.is_none() && !pinch_is_click && !primary_points {
                if let Some((name, score)) = match_custom(
                    &self.history.iter().cloned().collect::<Vec<_>>(),
                    &self.custom,
                    Some(&mut self.custom_stats),
                ) {
                    if score > confidence {
                        label = name.clone();
                        confidence = score;
                    }
                    active.push(name.clone());
                    self.fire_mapped(&profile_id, &name, score, now);
                }
            }
            self.gates.deactivate_unlisted(&active);
            self.released = false;
        } else if !stage_live && pose.is_some() && confidence >= 0.55 {
            self.rejection = if self.paused {
                "paused".into()
            } else if !self.control {
                "control_off".into()
            } else {
                "input_unavailable".into()
            };
            if !self.released {
                self.send_input(InputMsg::ReleaseAll);
                self.released = true;
            }
        }

        if label != "—" && label != self.gesture {
            self.stats.gestures_recognized = self.stats.gestures_recognized.saturating_add(1);
        }
        self.gesture = if label == "—" {
            "—".into()
        } else {
            gesture_title(&label)
        };
        self.confidence = confidence.max(0.0);
        self.gesture_ms = started.elapsed().as_secs_f32() * 1000.0;
        self.dispatch_ms = dispatch_started.elapsed().as_secs_f32() * 1000.0;
        let e2e =
            self.capture_ms + self.inference_ms + self.ipc_ms + self.gesture_ms + self.dispatch_ms;
        self.latency_avg = if self.latency_avg == 0.0 {
            e2e
        } else {
            self.latency_avg * 0.9 + e2e * 0.1
        };

        let show_camera = self.settings.show_camera;
        let want_play = self.play_page_visible();
        let want_preview = self.preview_page_visible() && !want_play;
        let zone = if stage_live {
            NormRect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            }
        } else {
            self.calibration.range()
        };
        let draw_cursor = if stage_live {
            None
        } else {
            point.filter(|_| show_camera || message.frame_id % 2 == 0)
        };
        // Decode JPEG only when the open page actually shows a camera picture.
        let jpeg = message
            .preview_jpeg
            .as_deref()
            .filter(|_| show_camera && (want_play || want_preview));
        let music_live = self.stage.is_music();
        let melody_live = self.stage.is_melody();
        let max_preview_w = if melody_live {
            420
        } else if music_live {
            360
        } else {
            480
        };
        let frame_still = jpeg
            .and_then(decode_jpeg)
            .map(|rgb| downscale_rgb(&rgb, max_preview_w));
        if let Some(rgb) = frame_still.as_ref() {
            if message.frame_id % 45 == 0 && matches!(self.scene_hint, SceneHint::Ok) {
                let fallback = scene_hint(rgb, hands.first());
                if fallback != SceneHint::Ok {
                    self.scene_hint = fallback;
                }
                if self.coach_line.is_empty() {
                    let tip = scene_hint_text(self.scene_hint);
                    if !tip.is_empty() {
                        self.coach_line = tip.into();
                    }
                }
            }
        }

        if want_play {
            // Instruments page only — skip the shared Full mesh preview.
            if let Some(rgb) = frame_still {
                self.stage_cam = Some(rgb);
            }
            self.play_paint_tick = self.play_paint_tick.wrapping_add(1);
            let paint_every = if melody_live { 1 } else { 2 };
            if self.play_paint_tick % paint_every == 0 {
                if let Some(cam) = self.stage_cam.clone() {
                    let hand_style = if melody_live {
                        HandStyle::Aim
                    } else {
                        HandStyle::Tips
                    };
                    let image = render_preview_rgb(
                        cam,
                        &shown,
                        zone,
                        None,
                        self.scene_hint,
                        hand_style,
                        |canvas, pass| {
                            if pass == 0 {
                                self.stage.paint_under(canvas);
                            } else {
                                self.stage.paint_over(canvas);
                            }
                        },
                    );
                    self.play_preview = Some(Arc::new(image));
                    self.play_frame_revision = self.play_frame_revision.wrapping_add(1);
                }
            }
            if self.preview.is_some() {
                self.preview = None;
                self.frame_revision = self.frame_revision.wrapping_add(1);
            }
        } else if want_preview {
            if self.play_preview.is_some() || self.stage_cam.is_some() {
                self.play_preview = None;
                self.stage_cam = None;
                self.play_paint_tick = 0;
                self.play_frame_revision = self.play_frame_revision.wrapping_add(1);
            }
            let paint_preview =
                frame_still.is_some() || (!show_camera && message.frame_id % 2 == 0);
            if paint_preview {
                let stage_fx = self.stage.is_live() && self.stage_page_visible();
                let image = if let Some(rgb) = frame_still {
                    render_preview_rgb(
                        rgb,
                        &shown,
                        zone,
                        draw_cursor,
                        self.scene_hint,
                        if stage_fx {
                            HandStyle::Tips
                        } else {
                            HandStyle::Full
                        },
                        |canvas, pass| {
                            if !stage_fx {
                                return;
                            }
                            if pass == 0 {
                                self.stage.paint_under(canvas);
                            } else {
                                self.stage.paint_over(canvas);
                            }
                        },
                    )
                } else {
                    render_preview(
                        None,
                        &shown,
                        zone,
                        draw_cursor,
                        false,
                        self.scene_hint,
                        |_canvas, _pass| {},
                    )
                    .expect("blank preview")
                };
                self.preview = Some(Arc::new(image));
                self.frame_revision = self.frame_revision.wrapping_add(1);
            }
        } else {
            // Other pages: landmarks/cursor still run, but no camera picture work.
            if self.play_preview.is_some() || self.stage_cam.is_some() {
                self.play_preview = None;
                self.stage_cam = None;
                self.play_paint_tick = 0;
                self.play_frame_revision = self.play_frame_revision.wrapping_add(1);
            }
            if self.preview.is_some() {
                self.preview = None;
                self.frame_revision = self.frame_revision.wrapping_add(1);
            }
        }
        self.publish();
    }

    fn fire_mapped(&mut self, profile_id: &str, gesture: &str, confidence: f32, now: u64) {
        let Some(binding) = self.binding(profile_id, gesture).cloned() else {
            return;
        };
        if !binding.enabled || matches!(binding.command, CommandSpec::None | CommandSpec::Cursor) {
            return;
        }
        if (gesture == "double_pinch" || gesture == "pinch")
            && matches!(
                binding.command,
                CommandSpec::DoubleClick | CommandSpec::LeftClick | CommandSpec::Drag
            )
        {
            return;
        }
        self.fire(
            gesture,
            binding.command,
            confidence,
            binding.min_duration_ms,
            binding.cooldown_ms,
            now,
        );
    }

    fn fire(
        &mut self,
        gesture: &str,
        command: CommandSpec,
        confidence: f32,
        min_duration_ms: u64,
        cooldown_ms: u64,
        now: u64,
    ) {
        let gate = self
            .gates
            .observe(gesture, true, min_duration_ms, cooldown_ms, now);
        if gate != GateResult::Fired {
            if let GateResult::Rejected(reason) = gate {
                if confidence >= 0.45 {
                    self.rejection = reason.into();
                    self.stats.rejected = self.stats.rejected.saturating_add(1);
                    self.command_status = "отклонено".into();
                }
            }
            return;
        }
        let (_, _, _, conf_floor) = self.active_pointer_tune();
        let minimum = self
            .binding(&self.active_profile_id(), gesture)
            .map(|item| item.min_confidence)
            .unwrap_or(conf_floor);
        let minimum = minimum.max(conf_floor);
        match review(self.paused, confidence, minimum, true) {
            Ok(()) => {
                if self.blocked_window_command(&command) {
                    self.rejection = "conflict".into();
                    self.command_status = "отклонено".into();
                    return;
                }
                self.command = command_label(&command);
                self.command_status = "выполнено".into();
                self.stats.commands_dispatched = self.stats.commands_dispatched.saturating_add(1);
                if matches!(command, CommandSpec::Scroll { .. }) {
                    self.stats.scrolls = self.stats.scrolls.saturating_add(1);
                }
                self.send_input(InputMsg::Command(command));
                self.log(&format!("{} → {}", gesture_title(gesture), self.command));
            }
            Err(reason) => {
                self.gates.rearm(gesture);
                self.rejection = reason.into();
                self.stats.rejected = self.stats.rejected.saturating_add(1);
                self.command_status = "отклонено".into();
            }
        }
    }

    fn apply_pointer(&mut self, event: &PointerEvent) {
        match event {
            PointerEvent::Move(x, y) => self.send_input(InputMsg::Move(*x, *y)),
            PointerEvent::PressLeft => {
                self.command = "Зажато".into();
                self.command_status = "удержание".into();
                self.send_input(InputMsg::PressLeft);
            }
            PointerEvent::ReleaseLeft => {
                self.command = "Отпущено".into();
                self.command_status = "готово".into();
                self.send_input(InputMsg::ReleaseLeft);
            }
            PointerEvent::ClickLeft => {
                // Prefer last pointer position via a no-op move snap when available.
                self.send_input(InputMsg::ClickLeft);
            }
        }
    }

    fn snap_coords(&mut self, x: i32, y: i32) -> (i32, i32, String) {
        if !self.settings.snap_ui {
            self.snap_target.clear();
            return (x, y, String::new());
        }
        match uia::snap_point(x, y, self.settings.snap_radius_px, &self.ui_targets) {
            Some((sx, sy, name)) => {
                self.snap_target = name.clone();
                (sx, sy, name)
            }
            None => {
                self.snap_target.clear();
                (x, y, String::new())
            }
        }
    }

    fn click_with_snap(&mut self, x: i32, y: i32) {
        let (sx, sy, name) = self.snap_coords(x, y);
        if !name.is_empty() && self.coach_line.is_empty() {
            self.coach_line = format!("прилипло: {name}");
        }
        self.send_input(InputMsg::Move(sx, sy));
        self.send_input(InputMsg::ClickLeft);
    }

    fn active_pointer_tune(&self) -> (f32, f32, f32, f32) {
        let profile_id = self.active_profile_id();
        if let Some(profile) = self.profiles.iter().find(|profile| profile.id == profile_id) {
            if profile.tune.override_tune {
                return (
                    profile.tune.speed,
                    profile.tune.smoothness,
                    profile.tune.deadzone,
                    profile.tune.confidence,
                );
            }
        }
        (
            self.settings.speed,
            self.settings.smoothness,
            self.settings.deadzone,
            self.settings.confidence,
        )
    }

    fn begin_exam(&mut self) {
        self.exam = Some(ExamSession {
            poses: vec!["open_palm", "pinch", "v_sign", "index_point"],
            index: 0,
            held: 0,
        });
        self.wanted_pose = "open_palm".into();
        self.coach_line = "Экзамен: раскрытая ладонь.".into();
        self.log("Экзамен: покажите раскрытую ладонь.");
    }

    fn suggest_from_session(&mut self) {
        let Some(path) = crate::session::latest_session_path() else {
            self.threshold_suggest =
                "Нет записанных сессий. Включите запись на странице диагностики.".into();
            self.suggested_confidence = 0.0;
            return;
        };
        match crate::session::read_session(&path) {
            Ok(frames) => {
                self.session_frames = frames.len() as u64;
                let (text, conf) =
                    crate::session::suggest_thresholds(&frames, self.settings.confidence);
                self.threshold_suggest = text.clone();
                self.suggested_confidence = conf.unwrap_or(0.0);
                self.log(&text);
            }
            Err(err) => self.error = err.to_string(),
        }
    }

    fn observe_exam(&mut self, hands: &[TrackedHand], _now: u64) {
        let Some(exam) = &self.exam else {
            return;
        };
        let Some(wanted) = exam.poses.get(exam.index).copied() else {
            self.exam = None;
            self.wanted_pose.clear();
            self.coach_line = "Экзамен пройден. Можно работать.".into();
            return;
        };
        self.wanted_pose = wanted.into();
        let pinch = self.calibration.pinch_limit();
        let gap = self.calibration.gap_limit();
        let remembered = self.remembered();
        let gates = PoseGates {
            extend: self.calibration.extend_gate(),
            curl: self.calibration.curl_gate(),
            v_spread: self.calibration.v_gate(),
            distance: hands
                .first()
                .map(|hand| {
                    self.calibration
                        .distance_boost(distance(hand.landmarks[0], hand.landmarks[9]))
                })
                .unwrap_or(1.0),
        };
        let seen = hands
            .first()
            .and_then(|hand| classify_hand_gated(hand, pinch, gap, remembered, gates).map(|item| item.id));
        let Some(exam) = self.exam.as_mut() else {
            return;
        };
        if seen == Some(wanted) {
            exam.held = exam.held.saturating_add(1);
        } else {
            exam.held = 0;
        }
        if exam.held >= 18 {
            exam.index = exam.index.saturating_add(1);
            exam.held = 0;
            if let Some(next) = exam.poses.get(exam.index).copied() {
                self.log(&format!("Экзамен: дальше «{}».", gesture_title(next)));
            } else {
                self.exam = None;
                self.wanted_pose.clear();
                self.coach_line = "Экзамен пройден. Можно работать.".into();
                self.log("Экзамен пройден.");
            }
        }
    }

    fn blocked_window_command(&self, command: &CommandSpec) -> bool {
        if !matches!(
            command,
            CommandSpec::MinimizeWindow | CommandSpec::MaximizeWindow | CommandSpec::RestoreWindow
        ) {
            return false;
        }
        input::foreground_process().0 == self.our_pid
    }

    fn binding(&self, profile_id: &str, gesture: &str) -> Option<&crate::model::GestureBinding> {
        self.profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .and_then(|profile| profile.binding(gesture))
    }

    fn active_profile_id(&self) -> String {
        if self.settings.air_keyboard {
            "air_keyboard".into()
        } else if self.settings.auto_context {
            self.context_profile.clone()
        } else {
            self.settings.profile.clone()
        }
    }

    fn stop_voice(&mut self) {
        if self.voice_listening {
            self.voice_listening = false;
            let _ = self.voice.commands.try_send(voice::VoiceCommand::Stop);
        }
    }

    fn accept_voice(&mut self, intent: voice::VoiceIntent) {
        if !self.voice_listening {
            return;
        }
        let Some(command) = intent.command.clone() else {
            let line = voice::describe_intent(&intent);
            self.voice_line = line.clone();
            self.log(&line);
            return;
        };
        let cooldown = self.last_voice.elapsed() >= Duration::from_millis(800);
        match review(self.paused, intent.confidence, 0.55, cooldown) {
            Ok(()) => {
                if self.blocked_window_command(&command) {
                    self.rejection = "conflict".into();
                    let line = format!("услышал «{}» — это окно не сворачиваем", intent.text);
                    self.voice_line = line.clone();
                    self.log(&line);
                    return;
                }
                self.last_voice = Instant::now();
                self.command = command_label(&command);
                self.command_status = "выполнено".into();
                self.stats.commands_dispatched = self.stats.commands_dispatched.saturating_add(1);
                if matches!(command, CommandSpec::Scroll { .. }) {
                    self.stats.scrolls = self.stats.scrolls.saturating_add(1);
                }
                self.voice_line = format!("услышал «{}» → {}", intent.text, self.command);
                let line = self.voice_line.clone();
                self.log(&line);
                self.send_input(InputMsg::Command(command));
            }
            Err(reason) => {
                self.rejection = reason.into();
                self.command_status = "отклонено".into();
                self.stats.rejected = self.stats.rejected.saturating_add(1);
                self.voice_line = format!("услышал «{}» — не выполнено", intent.text);
                let line = self.voice_line.clone();
                self.log(&line);
            }
        }
    }

    fn send_input(&self, message: InputMsg) {
        let Some(tx) = &self.input else {
            return;
        };
        match message {
            InputMsg::Move(_, _) => {
                let _ = tx.try_send(message);
            }
            _ => {
                let _ = tx.send_timeout(message, Duration::from_millis(80));
            }
        }
    }

    /// Pages that show the shared camera `frame` image.
    fn preview_page_visible(&self) -> bool {
        matches!(self.ui_page, 0 | 1 | 3 | 7 | 10)
    }

    /// Instruments page showing `play-frame`.
    fn play_page_visible(&self) -> bool {
        self.ui_page == 12 && self.stage.is_music()
    }

    /// Stage effects only while their page is open.
    fn stage_page_visible(&self) -> bool {
        if self.stage.is_music() {
            self.ui_page == 12
        } else if self.stage.is_live() {
            self.ui_page == 10
        } else {
            false
        }
    }

    fn vision_config(&self) -> VisionConfig {
        let (width, height, mut fps, mut every) = self.settings.quality.params();
        let want_play = self.play_page_visible();
        let want_preview = self.preview_page_visible();
        // Chase ≥30 FPS. Do NOT shrink width/height when entering instruments —
        // that used to reopen VideoCapture on every page switch (FPS died to ~20).
        fps = fps.max(30);
        if want_play {
            every = every.min(2);
        }
        // No JPEG encode when the open page does not display the camera picture.
        // Landmarks still stream for cursor control (soft configure, no reopen).
        let send_preview = self.settings.show_camera && (want_play || want_preview);
        VisionConfig {
            camera_index: self.settings.camera_index,
            width,
            height,
            fps,
            preview: send_preview,
            preview_every: if send_preview { every } else { 30 },
            mirror: self.settings.mirror,
        }
    }

    fn camera_label(&self) -> String {
        self.cameras
            .iter()
            .find(|camera| camera.index == self.settings.camera_index)
            .or_else(|| self.cameras.iter().find(|camera| camera.selected))
            .or_else(|| self.cameras.first())
            .map(|camera| camera.label.clone())
            .unwrap_or_else(|| "камера".into())
    }

    fn persist_settings(&mut self) {
        self.settings_revision = self.settings_revision.wrapping_add(1);
        if let Err(err) = store::save_settings(&self.settings) {
            self.error = err.to_string();
        }
    }

    fn assign_command(&mut self, gesture: &str, label: &str) {
        let Some(id) = crate::commands::command_id_from_label(label) else {
            self.error = "Такой команды нет в разрешённом списке.".into();
            return;
        };
        let Ok(command) = crate::commands::command_from_id(id) else {
            self.error = "Команда отклонена политикой безопасности.".into();
            return;
        };
        let profile_id = self.active_profile_id();
        let Some(profile) = self
            .profiles
            .iter_mut()
            .find(|profile| profile.id == profile_id)
        else {
            return;
        };
        if let Some(binding) = profile.binding_mut(gesture) {
            binding.command = command;
        } else if self.custom.iter().any(|item| item.name == gesture)
            || BUILTIN_GESTURES.contains(&gesture)
        {
            profile.mappings.push((
                gesture.to_string(),
                crate::model::GestureBinding::new(command, self.settings.confidence, 0, 450),
            ));
        } else {
            self.error = "Жест не найден.".into();
            return;
        }
        let _ = store::save_profiles(&self.profiles);
        self.gestures_revision = self.gestures_revision.wrapping_add(1);
        self.error.clear();
    }

    fn observe_calibration(&mut self, hands: &[TrackedHand], _now: u64) {
        if let Some(session) = self.calibration_session.as_ref() {
            self.wanted_pose = match &session.step {
                CalStep::Palm { .. } => "open_palm".into(),
                CalStep::Point { .. } => "index_point".into(),
                CalStep::Pinch { .. } | CalStep::Drill { .. } => "pinch".into(),
                CalStep::Thumb { .. } => "thumb_up".into(),
                CalStep::Zone { .. } => String::new(),
                CalStep::Verify { stage, .. } => match stage {
                    0 => "pinch".into(),
                    1 => "v_sign".into(),
                    _ => "thumb_up".into(),
                },
            };
        }
        let Some(hand) = hands.first() else {
            if let Some(session) = self.calibration_session.as_mut() {
                if matches!(
                    session.step,
                    CalStep::Point { .. }
                        | CalStep::Thumb { .. }
                        | CalStep::Pinch { .. }
                        | CalStep::Verify { .. }
                        | CalStep::Drill { .. }
                ) {
                    session.note =
                        "Рука вышла из кадра. Верните её и повторите то, что написано сверху."
                            .into();
                }
            }
            return;
        };
        let pose = classify_hand_gap(hand, 0.42, 0.0).map(|item| item.id);
        let shape = finger_shape(hand);
        let scale = crate::gestures::distance(hand.landmarks[0], hand.landmarks[9]);
        let span = crate::gestures::distance(hand.landmarks[8], hand.landmarks[20]);
        let ratio = crate::gestures::pinch_ratio(&hand.landmarks);
        let gap = crate::gestures::distance(hand.landmarks[4], hand.landmarks[8])
            / crate::gestures::distance(hand.landmarks[5], hand.landmarks[8]).max(0.02);
        let now_ms = self.now_ms();
        let advance = {
            let Some(session) = self.calibration_session.as_mut() else {
                return;
            };
            match &mut session.step {
                CalStep::Palm { open_sum, samples } => {
                    let palm = crate::gestures::thumb_way(&hand.landmarks) == 0
                        && crate::gestures::finger_open(&hand.landmarks, 5, 6, 8) > 0.34
                        && crate::gestures::finger_open(&hand.landmarks, 9, 10, 12) > 0.34
                        && crate::gestures::finger_open(&hand.landmarks, 13, 14, 16) > 0.34;
                    if palm {
                        session.scale_sum += scale;
                        session.span_sum += span;
                        add_shape(&mut session.palm_sum, &mut session.palm_n, shape);
                        *open_sum += ratio;
                        *samples += 1;
                        session.note = format!(
                            "Ладонь вижу. Это ваша рука без жеста. Держите ещё {}.",
                            14u32.saturating_sub(*samples)
                        );
                    } else if crate::gestures::thumb_way(&hand.landmarks) != 0 {
                        session.note =
                            "Это большой палец. Для первого теста раскройте все пальцы.".into();
                    } else {
                        session.note =
                            "Раскройте ладонь: указательный, средний и безымянный прямые.".into();
                    }
                    if *samples >= 14 {
                        1
                    } else {
                        0
                    }
                }
                CalStep::Point { held } => {
                    if pose == Some("index_point") {
                        add_shape(&mut session.point_sum, &mut session.point_n, shape);
                        *held = held.saturating_add(1);
                        session.note = format!(
                            "Указательный вижу. Так вы будете водить курсор. Держите ещё {}.",
                            10u32.saturating_sub(*held)
                        );
                    } else {
                        *held = 0;
                        session.note = format!(
                            "Сейчас это {}. Выпрямите только указательный, остальные подогните.",
                            pose_name(pose)
                        );
                    }
                    if *held >= 10 {
                        2
                    } else {
                        0
                    }
                }
                CalStep::Pinch {
                    left,
                    ratios,
                    gaps,
                    down,
                    held_frames,
                } => {
                    let apart = session.open_ratio.max(0.7);
                    let touching = crate::gestures::pinch_active(&hand.landmarks, false, 0.42);
                    let released = gap > 0.55
                        || ratio > apart * 0.78
                        || crate::gestures::thumb_way(&hand.landmarks) != 0;
                    if touching {
                        *held_frames = held_frames.saturating_add(1);
                        if *held_frames >= 3 {
                            ratios.push(ratio);
                            gaps.push(gap);
                            *down = true;
                        }
                        session.note = "Касание вижу. Разожмите пальцы.".into();
                    } else if *down && released {
                        *down = false;
                        *held_frames = 0;
                        *left = left.saturating_sub(1);
                        session.note =
                            format!("Это ваше касание. Осталось {left}. Разожмите и повторите.");
                    } else {
                        *held_frames = 0;
                        session.note = if crate::gestures::thumb_way(&hand.landmarks) != 0 {
                            "Это большой палец вверх. Опустите его на кончик указательного.".into()
                        } else {
                            format!(
                                "Коснитесь большим именно кончика указательного. Осталось {left}."
                            )
                        };
                    }
                    if *left == 0 {
                        3
                    } else {
                        0
                    }
                }
                CalStep::Thumb { held, gaps } => {
                    let lifted = pose == Some("thumb_up")
                        && !crate::gestures::pinch_active(&hand.landmarks, false, 0.42);
                    if lifted {
                        add_shape(&mut session.thumb_sum, &mut session.thumb_n, shape);
                        *held = held.saturating_add(1);
                        gaps.push(gap);
                        session.note = format!(
                            "Большой палец вверх вижу. Указательного не касайтесь. Держите ещё {}.",
                            10u32.saturating_sub(*held)
                        );
                    } else {
                        *held = 0;
                        gaps.clear();
                        session.note = if pose == Some("pinch")
                            || crate::gestures::pinch_active(&hand.landmarks, false, 0.42)
                        {
                            "Пальцы слишком близко: это щипок. Поднимите большой выше кулака."
                                .into()
                        } else {
                            format!(
                                "Сейчас это {}. Сожмите кулак и поднимите большой выше остальных.",
                                pose_name(pose)
                            )
                        };
                    }
                    if *held >= 10 {
                        4
                    } else {
                        0
                    }
                }
                CalStep::Zone {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                    travel,
                    samples,
                    last,
                    ..
                } => {
                    let point = (hand.landmarks[8][0], hand.landmarks[8][1]);
                    *min_x = min_x.min(point.0);
                    *min_y = min_y.min(point.1);
                    *max_x = max_x.max(point.0);
                    *max_y = max_y.max(point.1);
                    if let Some(previous) = last {
                        let dx = point.0 - previous.0;
                        let dy = point.1 - previous.1;
                        *travel += (dx * dx + dy * dy).sqrt();
                    }
                    *last = Some(point);
                    *samples += 1;
                    session.note =
                        "Вся картинка — ваш экран. Поводите рукой от края до края.".into();
                    0
                }
                CalStep::Verify {
                    stage,
                    held,
                    v_sum,
                    v_n,
                } => {
                    let need = match *stage {
                        0 => "pinch",
                        1 => "v_sign",
                        _ => "thumb_up",
                    };
                    self.wanted_pose = need.into();
                    let ok = pose == Some(need)
                        || (*stage == 0
                            && crate::gestures::pinch_active(&hand.landmarks, false, 0.42));
                    if ok {
                        *held = held.saturating_add(1);
                        if *stage == 1 {
                            let scale = distance(hand.landmarks[0], hand.landmarks[9]).max(0.02);
                            *v_sum += distance(hand.landmarks[8], hand.landmarks[12]) / scale;
                            *v_n += 1;
                        }
                        session.note = format!(
                            "Проверка: {} вижу. Держите ещё {}.",
                            pose_name(Some(need)),
                            8u32.saturating_sub(*held)
                        );
                    } else {
                        *held = 0;
                        session.note = format!(
                            "Проверка {}. Сейчас это {}.",
                            pose_name(Some(need)),
                            pose_name(pose)
                        );
                    }
                    if *held >= 8 {
                        5
                    } else {
                        0
                    }
                }
                CalStep::Drill {
                    left,
                    gaps,
                    ratios,
                    down,
                    held_frames,
                    until_ms,
                } => {
                    self.wanted_pose = "pinch".into();
                    if now_ms >= *until_ms && *left > 0 {
                        session.note = "Время вышло. Нажмите «Дальше», чтобы сохранить то, что уже есть, или начните тренировку снова.".into();
                        0
                    } else {
                        let touching = crate::gestures::pinch_active(&hand.landmarks, false, 0.42);
                        let released = gap > 0.55 || ratio > session.open_ratio.max(0.7) * 0.78;
                        if touching {
                            *held_frames = held_frames.saturating_add(1);
                            if *held_frames >= 2 {
                                gaps.push(gap);
                                ratios.push(ratio);
                                *down = true;
                            }
                            session.note =
                                format!("Тренировка щипка: касание {}. Разожмите.", 6 - *left);
                        } else if *down && released {
                            *down = false;
                            *held_frames = 0;
                            *left = left.saturating_sub(1);
                            session.note = format!("Ещё {} касания за оставшееся время.", *left);
                        } else {
                            *held_frames = 0;
                            session.note = format!("Тренировка: 5 щипков. Осталось {}.", *left);
                        }
                        if *left == 0 {
                            6
                        } else {
                            0
                        }
                    }
                }
            }
        };
        match advance {
            1 => self.finish_palm(),
            2 => self.finish_point(),
            3 => self.finish_pinch(),
            4 => self.finish_thumb(),
            5 => self.advance_verify(),
            6 => self.finish_drill(),
            _ => {}
        }
    }

    fn finish_palm(&mut self) {
        let (open_sum, samples, scale_sum, span_sum) = {
            let Some(session) = self.calibration_session.as_ref() else {
                return;
            };
            let CalStep::Palm { open_sum, samples } = session.step else {
                return;
            };
            (open_sum, samples, session.scale_sum, session.span_sum)
        };
        if samples < 12 {
            self.error = "Нужно чуть дольше подержать раскрытую ладонь.".into();
            return;
        }
        let open_ratio = open_sum / samples as f32;
        if open_ratio < 0.40 {
            self.error = "Ладонь была сжата. Раскройте пальцы и подержите ещё.".into();
            return;
        }
        let Some(session) = self.calibration_session.as_mut() else {
            return;
        };
        session.open_ratio = open_ratio;
        session.scale_sum = scale_sum / samples as f32;
        session.span_sum = span_sum / samples as f32;
        session.step = CalStep::Point { held: 0 };
        session.note =
            "Тест 2 из 7. Выпрямите один указательный. Остальные пальцы подогните.".into();
        self.error.clear();
        self.log("Дальше ваш указательный палец: так программа узнает, как вы водите курсор.");
    }

    fn finish_point(&mut self) {
        let ready = self
            .calibration_session
            .as_ref()
            .is_some_and(|session| matches!(session.step, CalStep::Point { held } if held >= 8));
        if !ready {
            self.error = "Подержите один указательный палец, пока тест не засчитается.".into();
            return;
        }
        let Some(session) = self.calibration_session.as_mut() else {
            return;
        };
        session.step = CalStep::Pinch {
            left: 3,
            ratios: Vec::new(),
            gaps: Vec::new(),
            down: false,
            held_frames: 0,
        };
        session.note =
            "Тест 3 из 7. Три раза коснитесь большим кончика указательного и разожмите.".into();
        self.error.clear();
        self.log("Дальше три ваших касания. Так программа узнает, где для вас начинается щипок.");
    }

    fn finish_pinch(&mut self) {
        let (ratios, gaps, open_ratio) = {
            let Some(session) = self.calibration_session.as_ref() else {
                return;
            };
            let CalStep::Pinch { ratios, gaps, .. } = &session.step else {
                return;
            };
            (ratios.clone(), gaps.clone(), session.open_ratio)
        };
        let mut ordered = ratios.clone();
        ordered.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let contact = ordered.get(ordered.len() / 5).copied().unwrap_or(0.28);
        let gap_mean = if gaps.is_empty() {
            0.22
        } else {
            gaps.iter().sum::<f32>() / gaps.len() as f32
        };
        if ordered.len() < 6 || open_ratio - contact < 0.12 {
            self.error = "Касание почти не отличалось от открытой ладони. Коснитесь кончика и разожмите три раза.".into();
            if let Some(session) = self.calibration_session.as_mut() {
                session.step = CalStep::Pinch {
                    left: 3,
                    ratios: Vec::new(),
                    gaps: Vec::new(),
                    down: false,
                    held_frames: 0,
                };
                session.note = "Большой палец на кончик указательного, не вверх.".into();
            }
            return;
        }
        let slack = (open_ratio - contact).clamp(0.0, 0.18) * 0.25;
        let Some(session) = self.calibration_session.as_mut() else {
            return;
        };
        session.pinch_ratio = (contact + slack).clamp(0.18, 0.40);
        session.pinch_gap = gap_mean;
        session.step = CalStep::Thumb {
            held: 0,
            gaps: Vec::new(),
        };
        session.note =
            "Тест 4 из 7. Сожмите кулак и поднимите большой выше остальных. Кончика не касайтесь."
                .into();
        self.error.clear();
        self.log("Дальше большой палец вверх. Так программа отличит его от вашего щипка.");
    }

    fn finish_thumb(&mut self) {
        let (gaps, pinch_gap, ready) = {
            let Some(session) = self.calibration_session.as_ref() else {
                return;
            };
            let CalStep::Thumb { held, gaps } = &session.step else {
                return;
            };
            (gaps.clone(), session.pinch_gap, *held >= 8)
        };
        if !ready {
            self.error = "Подержите большой палец вверх, не касаясь указательного.".into();
            return;
        }
        let gap_mean = if gaps.is_empty() {
            0.0
        } else {
            gaps.iter().sum::<f32>() / gaps.len() as f32
        };
        if gap_mean < pinch_gap + 0.12 {
            self.error = "Большой палец слишком близко к указательному. Поднимите его выше и не касайтесь кончика.".into();
            if let Some(session) = self.calibration_session.as_mut() {
                session.step = CalStep::Thumb {
                    held: 0,
                    gaps: Vec::new(),
                };
                session.note = "Кулак, большой палец выше остальных. Это не щипок.".into();
            }
            return;
        }
        let now = self.now_ms();
        let Some(session) = self.calibration_session.as_mut() else {
            return;
        };
        session.thumb_gap = gap_mean;
        session.step = CalStep::Zone {
            until_ms: now + 4_000,
            min_x: 1.0,
            min_y: 1.0,
            max_x: 0.0,
            max_y: 0.0,
            travel: 0.0,
            samples: 0,
            last: None,
        };
        session.note = "Тест 5 из 7. Вся картинка — ваш экран. Поводите рукой по кадру.".into();
        self.error.clear();
        self.log("Последний тест: зона на весь кадр. Скорость курсора не меняется.");
    }

    fn advance_calibration(&mut self) {
        let kind = self
            .calibration_session
            .as_ref()
            .map(|session| match &session.step {
                CalStep::Palm { .. } => 1,
                CalStep::Point { .. } => 2,
                CalStep::Pinch { left, .. } => 3 + *left as i32,
                CalStep::Thumb { .. } => 7,
                CalStep::Zone { .. } => 8,
                CalStep::Verify { .. } => 9,
                CalStep::Drill { left, .. } => 10 + *left as i32,
            });
        match kind {
            Some(1) => self.finish_palm(),
            Some(2) => self.finish_point(),
            Some(left) if (3..7).contains(&left) => {
                let left = left - 3;
                self.error = format!("Осталось {left} касания. Коснитесь кончика и разожмите.");
            }
            Some(7) => self.finish_thumb(),
            Some(8) => self.commit_zone(true),
            Some(9) => self.advance_verify(),
            Some(left) if (10..16).contains(&left) => self.finish_drill(),
            _ => {}
        }
    }

    fn advance_range_timer(&mut self) {
        let now = self.now_ms();
        let ready = self.calibration_session.as_ref().is_some_and(|session| {
            matches!(&session.step, CalStep::Zone { until_ms, min_x, min_y, max_x, max_y, samples, .. }
                if now >= *until_ms
                    && *samples >= 8
                    && *max_x - *min_x >= 0.25
                    && *max_y - *min_y >= 0.25)
        });
        if ready {
            self.commit_zone(false);
        }
    }

    fn commit_zone(&mut self, allow_small: bool) {
        let bounds = self
            .calibration_session
            .as_ref()
            .and_then(|session| match session.step {
                CalStep::Zone {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                    ..
                } => Some((min_x, min_y, max_x, max_y)),
                _ => None,
            });
        let Some((min_x, min_y, max_x, max_y)) = bounds else {
            return;
        };
        let wide = max_x - min_x >= 0.25 && max_y - min_y >= 0.25;
        if !wide && !allow_small {
            return;
        }
        let Some(mut session) = self.calibration_session.take() else {
            return;
        };
        if session.pinch_ratio <= 0.0 {
            self.calibration_session = Some(session);
            self.error = "Щипок ещё не запомнен. Пройдите касания ещё раз.".into();
            return;
        }
        self.apply_session_shapes(&session);
        self.calibration.min_x = 0.0;
        self.calibration.min_y = 0.0;
        self.calibration.max_x = 1.0;
        self.calibration.max_y = 1.0;
        self.calibration.valid = true;
        let _ = store::save_calibration(&self.calibration);
        session.step = CalStep::Verify {
            stage: 0,
            held: 0,
            v_sum: 0.0,
            v_n: 0,
        };
        session.note =
            "Проверка 1 из 3. Покажите щипок ещё раз — программа сверит с вашим касанием.".into();
        self.calibration_session = Some(session);
        self.wanted_pose = "pinch".into();
        self.error.clear();
        self.log("Зона сохранена. Дальше короткая проверка жестов.");
    }

    fn apply_session_shapes(&mut self, session: &CalSession) {
        self.calibration.open_ratio = session.open_ratio;
        self.calibration.pinch_ratio = session.pinch_ratio;
        self.calibration.pinch_gap = session.pinch_gap;
        self.calibration.thumb_gap = session.thumb_gap;
        self.calibration.hand_scale = session.scale_sum;
        self.calibration.finger_span = session.span_sum;
        self.calibration.palm_open = mean_shape(session.palm_sum, session.palm_n);
        self.calibration.point_open = mean_shape(session.point_sum, session.point_n);
        self.calibration.thumb_open = mean_shape(session.thumb_sum, session.thumb_n);
        let palm = self.calibration.palm_open;
        let point = self.calibration.point_open;
        let thumb = self.calibration.thumb_open;
        let extend = ((palm.iter().sum::<f32>() / 4.0) * 0.55 + point[0] * 0.45).clamp(0.30, 0.40);
        let curl = ((thumb.iter().sum::<f32>() / 4.0) + 0.08).clamp(0.28, 0.42);
        self.calibration.extend_min = extend;
        self.calibration.curl_max = curl;
        if session.v_spread > 0.05 {
            self.calibration.v_spread = session.v_spread;
        }
    }

    fn advance_verify(&mut self) {
        let (stage, held, v_mean) = {
            let Some(session) = self.calibration_session.as_ref() else {
                return;
            };
            let CalStep::Verify {
                stage,
                held,
                v_sum,
                v_n,
            } = session.step
            else {
                return;
            };
            let mean = if v_n == 0 { 0.0 } else { v_sum / v_n as f32 };
            (stage, held, mean)
        };
        if held < 6 {
            self.error = "Подержите жест чуть дольше.".into();
            return;
        }
        if stage == 1 && v_mean > 0.05 {
            if let Some(session) = self.calibration_session.as_mut() {
                session.v_spread = v_mean;
            }
            self.calibration.v_spread = v_mean;
        }
        if stage >= 2 {
            let now = self.now_ms();
            let Some(session) = self.calibration_session.as_mut() else {
                return;
            };
            session.step = CalStep::Drill {
                left: 5,
                gaps: Vec::new(),
                ratios: Vec::new(),
                down: false,
                held_frames: 0,
                until_ms: now + 30_000,
            };
            session.note = "Тренировка 30 секунд: пять щипков. Так порог станет ещё точнее.".into();
            self.wanted_pose = "pinch".into();
            self.error.clear();
            self.log("Проверка пройдена. Дальше пять щипков.");
            return;
        }
        let next = stage + 1;
        let Some(session) = self.calibration_session.as_mut() else {
            return;
        };
        session.step = CalStep::Verify {
            stage: next,
            held: 0,
            v_sum: 0.0,
            v_n: 0,
        };
        session.note = match next {
            1 => "Проверка 2 из 3. Покажите букву V.".into(),
            _ => "Проверка 3 из 3. Большой палец вверх, без касания.".into(),
        };
        self.wanted_pose = if next == 1 {
            "v_sign".into()
        } else {
            "thumb_up".into()
        };
        self.error.clear();
    }

    fn start_pinch_drill(&mut self, standalone: bool) {
        let now = self.now_ms();
        self.calibration_session = Some(CalSession {
            step: CalStep::Drill {
                left: 5,
                gaps: Vec::new(),
                ratios: Vec::new(),
                down: false,
                held_frames: 0,
                until_ms: now + 30_000,
            },
            note: "Тренировка щипка: 5 касаний за 30 секунд.".into(),
            scale_sum: self.calibration.hand_scale,
            span_sum: self.calibration.finger_span,
            open_ratio: self.calibration.open_ratio.max(0.7),
            pinch_ratio: self.calibration.pinch_ratio,
            pinch_gap: self.calibration.pinch_gap,
            thumb_gap: self.calibration.thumb_gap,
            v_spread: self.calibration.v_spread,
            palm_sum: self.calibration.palm_open,
            palm_n: if self.calibration.has_shapes() { 1 } else { 0 },
            point_sum: self.calibration.point_open,
            point_n: if self.calibration.has_shapes() { 1 } else { 0 },
            thumb_sum: self.calibration.thumb_open,
            thumb_n: if self.calibration.has_shapes() { 1 } else { 0 },
        });
        self.wanted_pose = "pinch".into();
        self.error.clear();
        self.log(if standalone {
            "Отдельная тренировка щипка."
        } else {
            "Тренировка щипка после проверки."
        });
    }

    fn finish_drill(&mut self) {
        let (gaps, ratios, open_ratio) = {
            let Some(session) = self.calibration_session.as_ref() else {
                return;
            };
            let CalStep::Drill { gaps, ratios, .. } = &session.step else {
                return;
            };
            (gaps.clone(), ratios.clone(), session.open_ratio)
        };
        if ratios.len() < 4 {
            self.error = "Слишком мало касаний. Повторите тренировку.".into();
            return;
        }
        let mut ordered = ratios.clone();
        ordered.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let contact = ordered.get(ordered.len() / 5).copied().unwrap_or(0.28);
        let gap_mean = if gaps.is_empty() {
            self.calibration.pinch_gap
        } else {
            gaps.iter().sum::<f32>() / gaps.len() as f32
        };
        let slack = (open_ratio - contact).clamp(0.0, 0.18) * 0.25;
        self.calibration.pinch_ratio = (contact + slack).clamp(0.18, 0.40);
        if gap_mean > 0.05 {
            self.calibration.pinch_gap = gap_mean;
        }
        self.calibration.valid = true;
        let _ = store::save_calibration(&self.calibration);
        self.calibration_session = None;
        self.wanted_pose.clear();
        self.mistakes = MistakeLog::default();
        self.error.clear();
        self.log("Тренировка сохранена. Щипок подстроен ещё точнее и уже действует.");
        self.begin_exam();
    }

    fn observe_recording(&mut self, hands: &[TrackedHand], now: u64) {
        let Some(recorder) = self.recorder.as_mut() else {
            return;
        };
        if let Some(hand) = hands.first() {
            recorder.frames.push(normalize_landmarks(&hand.landmarks));
        }
        let done = recorder.frames.len() >= 36 || now.saturating_sub(recorder.started_ms) >= 1_500;
        if !done {
            return;
        }
        let recorder = self.recorder.take().unwrap();
        if recorder.frames.len() < 8 {
            self.error = "Запись прервана: рука потеряна.".into();
            return;
        }
        let frames = resample(&recorder.frames, 12);
        let mut gesture = CustomGesture {
            name: recorder.name,
            threshold: recorder.threshold,
            frames: frames.clone(),
            templates: Vec::new(),
        };
        if let Err(err) = validate_custom(&gesture) {
            self.error = err.to_string();
            return;
        }
        if let Some(existing) = self
            .custom
            .iter_mut()
            .find(|item| item.name == gesture.name)
        {
            // Keep up to 3 takes: primary frames + templates.
            if existing.templates.len() < 2 {
                existing.templates.push(frames);
            } else {
                existing.templates.remove(0);
                existing.templates.push(frames);
            }
            existing.threshold = gesture.threshold;
            gesture = existing.clone();
        } else {
            self.custom.push(gesture.clone());
        }
        let profile_id = self.active_profile_id();
        if let Some(profile) = self
            .profiles
            .iter_mut()
            .find(|profile| profile.id == profile_id)
        {
            if profile.binding(&gesture.name).is_none() {
                profile.mappings.push((
                    gesture.name.clone(),
                    crate::model::GestureBinding::new(CommandSpec::None, 0.7, 0, 700),
                ));
            }
        }
        let _ = store::save_gestures(&self.custom);
        let _ = store::save_profiles(&self.profiles);
        self.gestures_revision = self.gestures_revision.wrapping_add(1);
        self.error.clear();
        self.log(&format!("Шаблон «{}» сохранён. Назначьте ему команду. Это сопоставление образца, не обучение модели.", gesture.name));
    }

    fn span_penalty(&self, hand: Option<&TrackedHand>) -> f32 {
        let Some(hand) = hand else { return 0.0 };
        if self.calibration.finger_span < 0.04 {
            return 0.0;
        }
        let span = crate::gestures::distance(hand.landmarks[8], hand.landmarks[20]);
        if span < self.calibration.finger_span * 0.35 {
            0.15
        } else {
            0.0
        }
    }

    fn flush_uptime(&mut self) {
        let seconds = self.started.elapsed().as_secs();
        self.stats.uptime_seconds = self
            .stats
            .uptime_seconds
            .saturating_add(seconds.saturating_sub(self.uptime_mark));
        self.uptime_mark = seconds;
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn arm_scene_cursor(&mut self) {
        if !self.scene_cursor
            || self.stage.is_live()
            || !self.camera_on
            || !self.input_ok
            || self.paused
        {
            return;
        }
        self.stage_guard = false;
        if self.control {
            return;
        }
        self.control = true;
        self.error.clear();
        self.log("Курсор отдан пальцу.");
    }

    fn log(&mut self, message: &str) {
        tracing::info!(target: "air_control", "{message}");
        self.logs.push_back(message.to_string());
        while self.logs.len() > 40 {
            self.logs.pop_front();
        }
    }

    fn publish(&mut self) {
        self.flush_uptime();
        let profile = self
            .profiles
            .iter()
            .find(|profile| profile.id == self.active_profile_id());
        let profile_title = profile
            .map(|profile| profile.title.clone())
            .unwrap_or_else(|| "Профиль".into());
        let gestures = self.gesture_rows();
        let profiles = self
            .profiles
            .iter()
            .map(|profile| ProfileInfo {
                id: profile.id.clone(),
                title: profile.title.clone(),
                active: profile.id == self.active_profile_id(),
                override_tune: profile.tune.override_tune,
                speed: if profile.tune.override_tune {
                    profile.tune.speed
                } else {
                    self.settings.speed
                },
                smoothness: if profile.tune.override_tune {
                    profile.tune.smoothness
                } else {
                    self.settings.smoothness
                },
                deadzone: if profile.tune.override_tune {
                    profile.tune.deadzone
                } else {
                    self.settings.deadzone
                },
                confidence: if profile.tune.override_tune {
                    profile.tune.confidence
                } else {
                    self.settings.confidence
                },
            })
            .collect();
        let (calibration_title, calibration_hint, calibration_progress, calibration_active) =
            self.calibration_view();
        let system_status = if self.paused {
            "Пауза"
        } else if self.control {
            "Управление активно"
        } else if self.camera_on {
            "Камера активна"
        } else {
            "Ожидание"
        };
        let context_line = if self.settings.air_keyboard {
            format!("Воздушная клавиатура · окно {}", self.context_app)
        } else if self.settings.auto_context {
            format!(
                "Активное приложение {} → профиль по имени процесса",
                self.context_app
            )
        } else {
            format!(
                "Профиль выбран вручную · сейчас открыто {}",
                self.context_app
            )
        };
        let mut snap = self.snapshot.write().unwrap_or_else(|err| err.into_inner());
        snap.revision = snap.revision.wrapping_add(1);
        snap.settings_revision = self.settings_revision;
        snap.gestures_revision = self.gestures_revision;
        snap.cameras_revision = self.cameras_revision;
        snap.profiles_revision = self.profiles_revision;
        snap.frame_revision = self.frame_revision;
        snap.system_status = system_status.into();
        snap.camera_status = self.camera_status.clone();
        snap.hand_status = self.hand_status.clone();
        snap.gesture = self.gesture.clone();
        snap.confidence = self.confidence;
        snap.command = self.command.clone();
        snap.command_status = self.command_status.clone();
        snap.rejection = self.rejection.clone();
        snap.profile = profile_title;
        snap.context_line = context_line;
        snap.fps = self.fps;
        snap.latency_ms = self.latency_avg;
        snap.capture_ms = self.capture_ms;
        snap.inference_ms = self.inference_ms;
        snap.ipc_ms = self.ipc_ms;
        snap.gesture_ms = self.gesture_ms;
        snap.dispatch_ms = self.dispatch_ms;
        snap.control_on = self.control;
        snap.camera_on = self.camera_on;
        snap.paused = self.paused;
        snap.ipc_ok = self.ipc_ok;
        snap.cpu = self.cpu;
        snap.ram_mb = self.ram_mb;
        snap.vision_pid = self.vision_pid;
        snap.error = self.error.clone();
        snap.calibration_title = calibration_title;
        snap.calibration_hint = calibration_hint;
        snap.calibration_progress = calibration_progress;
        snap.calibration_active = calibration_active;
        snap.coach_line = self.coach_line.clone();
        snap.tracking_health = self.tracking_health.clone();
        snap.dwell_click = self.settings.dwell_click;
        snap.depth_cursor = self.settings.depth_cursor;
        snap.depth_gain = self.settings.depth_gain;
        snap.snap_ui = self.settings.snap_ui;
        snap.snap_radius_px = self.settings.snap_radius_px;
        snap.session_recording = self.session_dump.is_some();
        snap.session_frames = self.session_frames;
        snap.threshold_suggest = self.threshold_suggest.clone();
        snap.suggested_confidence = self.suggested_confidence;
        snap.exam_active = self.exam.is_some();
        snap.exam_hint = self
            .exam
            .as_ref()
            .and_then(|exam| exam.poses.get(exam.index).copied())
            .map(gesture_title)
            .unwrap_or_default();
        snap.exam_progress = self
            .exam
            .as_ref()
            .map(|exam| exam.index as f32 / exam.poses.len().max(1) as f32)
            .unwrap_or(0.0);
        snap.snap_target = self.snap_target.clone();
        snap.voice_ready = self.voice_ready;
        snap.voice_listening = self.voice_listening;
        snap.voice_line = self.voice_line.clone();
        snap.context_rules = self
            .context_rules
            .iter()
            .map(|rule| ContextRuleInfo {
                pattern: rule.pattern.clone(),
                profile_id: rule.profile_id.clone(),
                profile_title: self
                    .profiles
                    .iter()
                    .find(|profile| profile.id == rule.profile_id)
                    .map(|profile| profile.title.clone())
                    .unwrap_or_else(|| rule.profile_id.clone()),
                enabled: rule.enabled,
            })
            .collect();
        snap.stats_gestures = self.stats.gestures_recognized;
        snap.stats_commands = self.stats.commands_dispatched;
        snap.stats_clicks = self.stats.clicks;
        snap.stats_scrolls = self.stats.scrolls;
        snap.stats_rejected = self.stats.rejected;
        snap.stats_reconnects = self.stats.camera_reconnects;
        snap.stats_uptime = self.stats.uptime_seconds;
        snap.log_text = self.logs.iter().cloned().collect::<Vec<_>>().join("\n");
        snap.show_camera = self.settings.show_camera;
        snap.mirror = self.settings.mirror;
        snap.auto_context = self.settings.auto_context;
        snap.air_keyboard = self.settings.air_keyboard;
        let play = self.stage.view();
        snap.play_mode = play.mode;
        snap.play_instrument = play.instrument;
        snap.play_element = play.element;
        snap.play_brush = play.brush;
        snap.play_color = play.color;
        snap.play_preset = play.preset;
        snap.play_guitar_chord = play.guitar_chord;
        snap.play_guitar_mode = play.guitar_mode;
        snap.play_guitar_capo = play.guitar_capo;
        snap.play_guitar_strings = play.guitar_string_energy;
        snap.play_muted = play.muted;
        snap.play_volume = play.volume;
        snap.play_sensitivity = play.sensitivity;
        snap.play_thickness = play.thickness;
        snap.play_fade = play.fade;
        snap.play_label = play.label;
        snap.play_hint = play.hint;
        snap.play_revision = self.play_revision;
        snap.play_frame_revision = self.play_frame_revision;
        snap.play_frame_ready = self.play_preview.is_some();
        snap.play_preview = self.play_preview.clone();
        snap.speed = self.settings.speed;
        snap.smoothness = self.settings.smoothness;
        snap.acceleration = self.settings.acceleration;
        snap.deadzone = self.settings.deadzone;
        snap.confidence_threshold = self.settings.confidence;
        snap.quality_index = self.settings.quality.index();
        snap.hand_index = self.settings.primary_hand.index();
        snap.sensitivity_index = self.settings.sensitivity.index();
        snap.cameras = self.cameras.clone();
        snap.gestures = gestures;
        snap.profiles = profiles;
        snap.preview = self.preview.clone();
    }

    fn gesture_rows(&self) -> Vec<GestureInfo> {
        let profile_id = self.active_profile_id();
        let mut rows = Vec::new();
        if let Some(profile) = self
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
        {
            for (id, binding) in &profile.mappings {
                rows.push(GestureInfo {
                    id: id.clone(),
                    title: gesture_title(id),
                    command: command_label(&binding.command),
                    detail: format!(
                        "уверенность {:.0}% · удержание {} мс · пауза {} мс",
                        binding.min_confidence * 100.0,
                        binding.min_duration_ms,
                        binding.cooldown_ms
                    ),
                    how: gesture_how(id).into(),
                    custom: false,
                });
            }
        }
        for gesture in &self.custom {
            if rows.iter().any(|row| row.id == gesture.name) {
                if let Some(row) = rows.iter_mut().find(|row| row.id == gesture.name) {
                    row.custom = true;
                    row.detail = format!("шаблон · порог {:.2}", gesture.threshold);
                }
            } else {
                rows.push(GestureInfo {
                    id: gesture.name.clone(),
                    title: gesture.name.clone(),
                    command: "Нет".into(),
                    detail: format!("шаблон · порог {:.2}", gesture.threshold),
                    how: gesture_how(&gesture.name).into(),
                    custom: true,
                });
            }
        }
        rows
    }

    fn widen_frame(&mut self) {
        let width = self.calibration.max_x - self.calibration.min_x;
        let height = self.calibration.max_y - self.calibration.min_y;
        if width >= 0.98 && height >= 0.98 {
            return;
        }
        self.calibration.min_x = 0.0;
        self.calibration.min_y = 0.0;
        self.calibration.max_x = 1.0;
        self.calibration.max_y = 1.0;
        if self.calibration.valid {
            let _ = store::save_calibration(&self.calibration);
            self.log("Зона курсора расширена на всю картинку.");
        }
    }

    fn remembered(&self) -> Option<RememberedPoses> {
        if !self.calibration.has_shapes() {
            return None;
        }
        Some(RememberedPoses {
            palm: self.calibration.palm_open,
            point: self.calibration.point_open,
            thumb: self.calibration.thumb_open,
        })
    }

    fn calibration_view(&self) -> (String, String, f32, bool) {
        match &self.calibration_session {
            Some(CalSession { note, step: CalStep::Palm { samples, .. }, .. }) => (
                note.clone(),
                "Тест 1 из 7 · ваша ладонь".into(),
                (*samples as f32 / 14.0).clamp(0.0, 1.0) * 0.12,
                true,
            ),
            Some(CalSession { note, step: CalStep::Point { held }, .. }) => (
                note.clone(),
                "Тест 2 из 7 · ваш указательный".into(),
                0.12 + (*held as f32 / 10.0).clamp(0.0, 1.0) * 0.12,
                true,
            ),
            Some(CalSession { note, step: CalStep::Pinch { left, .. }, .. }) => (
                note.clone(),
                "Тест 3 из 7 · ваш щипок".into(),
                0.24 + ((3 - *left) as f32 / 3.0).clamp(0.0, 1.0) * 0.12,
                true,
            ),
            Some(CalSession { note, step: CalStep::Thumb { held, .. }, .. }) => (
                note.clone(),
                "Тест 4 из 7 · ваш большой палец".into(),
                0.36 + (*held as f32 / 10.0).clamp(0.0, 1.0) * 0.12,
                true,
            ),
            Some(CalSession { note, step: CalStep::Zone { min_x, max_x, min_y, max_y, .. }, .. }) => {
                let span = (((*max_x - *min_x).max(0.0) + (*max_y - *min_y).max(0.0)) / 2.0).min(1.0);
                (note.clone(), "Тест 5 из 7 · ваша зона".into(), 0.48 + span * 0.12, true)
            }
            Some(CalSession { note, step: CalStep::Verify { stage, held, .. }, .. }) => (
                note.clone(),
                format!("Проверка {} из 3 · сверка жестов", stage + 1),
                0.60 + (*stage as f32 / 3.0) * 0.12 + (*held as f32 / 8.0).clamp(0.0, 1.0) * 0.04,
                true,
            ),
            Some(CalSession { note, step: CalStep::Drill { left, until_ms, .. }, .. }) => {
                let left_secs = until_ms.saturating_sub(self.now_ms()) as f32 / 1000.0;
                (
                    note.clone(),
                    format!("Тренировка щипка · ещё {left}, ~{left_secs:.0} с"),
                    0.76 + ((5 - *left) as f32 / 5.0).clamp(0.0, 1.0) * 0.24,
                    true,
                )
            }
            None if self.calibration.has_shapes() => (
                "Уже действует: ладонь, указательный, щипок и большой сравниваются с вашей рукой. Можно отдельно потренировать щипок.".into(),
                "Рука подстроена под вас".into(),
                1.0,
                false,
            ),
            None if self.calibration.valid => (
                "Щипок уже подстроен. Пройдите тесты ещё раз или запустите тренировку щипка, чтобы порог стал точнее.".into(),
                "Рука подстроена под вас".into(),
                1.0,
                false,
            ),
            None => (
                "Семь шагов: ладонь, указательный, щипок, большой, зона, проверка и тренировка. Зона всегда на весь кадр.".into(),
                "Рука ещё не подстроена".into(),
                0.0,
                false,
            ),
        }
    }
}

fn add_shape(sum: &mut [f32; 4], count: &mut u32, shape: [f32; 4]) {
    for (slot, value) in sum.iter_mut().zip(shape) {
        *slot += value;
    }
    *count += 1;
}

fn mean_shape(sum: [f32; 4], count: u32) -> [f32; 4] {
    if count == 0 {
        return [0.0; 4];
    }
    let count = count as f32;
    [
        sum[0] / count,
        sum[1] / count,
        sum[2] / count,
        sum[3] / count,
    ]
}

fn pose_name(pose: Option<&str>) -> &'static str {
    match pose {
        Some("pinch") => "щипок",
        Some("thumb_up") => "большой палец вверх",
        Some("thumb_down") => "большой палец вниз",
        Some("open_palm") => "раскрытая ладонь",
        Some("closed_fist") => "кулак",
        Some("v_sign") => "буква V",
        Some("index_point") | Some("index_tap") => "указательный",
        Some(_) => "другое движение",
        None => "рука без ясного жеста",
    }
}

fn camera_score(name: &str) -> i32 {
    let lowered = name.to_ascii_lowercase();
    if [
        "infrared",
        "windows hello",
        "ir camera",
        "rgb-ir",
        "rgb ir",
    ]
    .iter()
    .any(|mark| lowered.contains(mark))
        || lowered.contains(" ir ")
    {
        return -80;
    }
    if [
        "obs virtual",
        "obs camera",
        "manycam",
        "snap camera",
        "droidcam",
        "virtual camera",
        "ndi",
    ]
    .iter()
    .any(|mark| lowered.contains(mark))
    {
        return -40;
    }
    let mut score = 1;
    for (mark, points) in [
        ("honor", 50),
        ("magicbook", 40),
        ("magic book", 40),
        ("fhd camera", 50),
        ("usb camera", 35),
        ("usb2.0", 25),
        ("usb 2.0", 25),
        ("uvc", 20),
        ("webcam", 18),
        ("hd camera", 15),
        ("hd usb", 15),
    ] {
        if lowered.contains(mark) {
            score += points;
        }
    }
    if lowered.contains("camera") {
        score += 4;
    }
    score
}

fn preferred_camera_index(devices: &[(i32, String)], saved: i32) -> i32 {
    let usable: Vec<&(i32, String)> = devices
        .iter()
        .filter(|(_, name)| camera_score(name) >= 0)
        .collect();
    if let Some((index, name)) = devices.iter().find(|(index, _)| *index == saved) {
        if camera_score(name) >= 0 || usable.is_empty() {
            return *index;
        }
    }
    usable
        .into_iter()
        .max_by_key(|(_, name)| camera_score(name))
        .map(|(index, _)| *index)
        .or_else(|| devices.first().map(|(index, _)| *index))
        .unwrap_or(0)
}

fn open_camera_privacy() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:privacy-webcam"])
            .creation_flags(0x0800_0000)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Camera")
            .spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        tracing::info!("open camera privacy settings is not wired on this OS");
    }
}

fn wall_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis() as u64)
        .unwrap_or(0)
}

impl Default for crate::model::ScreenRect {
    fn default() -> Self {
        Self::fallback()
    }
}

#[cfg(test)]
mod camera_pick_tests {
    use super::{camera_score, preferred_camera_index};

    #[test]
    fn prefers_usb_color_over_ir() {
        let devices = vec![
            (0, "IR Camera".into()),
            (1, "USB Camera".into()),
        ];
        assert!(camera_score("USB Camera") > camera_score("IR Camera"));
        assert_eq!(preferred_camera_index(&devices, 0), 1);
    }

    #[test]
    fn keeps_saved_color_camera() {
        let devices = vec![(0, "USB Camera".into()), (1, "OBS Virtual Camera".into())];
        assert_eq!(preferred_camera_index(&devices, 0), 0);
    }

    #[test]
    fn honors_named_module() {
        let devices = vec![
            (0, "Windows Hello IR".into()),
            (1, "Honor USB Camera".into()),
        ];
        assert_eq!(preferred_camera_index(&devices, 99), 1);
    }
}
