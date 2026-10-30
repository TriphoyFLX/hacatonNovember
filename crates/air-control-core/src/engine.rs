//! Orchestrates tracking, gesture decisions, safety, and the input thread.
//!
//! Control stays off until the user enables it. Pause releases every held
//! button and key. The vision process cannot submit a system command.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;

use crate::commands::{command_label, gesture_title, is_pointer_command, review, CommandSpec};
use crate::cursor::{PointerController, PointerEvent, PointerInput};
use crate::error::CoreError;
use crate::gestures::{
    classify_static, edge_penalty, match_custom, normalize_landmarks, resample, select_hands,
    GateResult, GateSet, HandMemory, MotionEvent, MotionTracker, BUILTIN_GESTURES,
};
use crate::input::{self, spawn_hotkey, spawn_input, HotkeyHandle, InputMsg};
use crate::model::{
    CalibrationProfile, CameraInfo, CustomGesture, GestureInfo, HandSide, PreviewImage,
    PrimaryHand, Profile, ProfileInfo, QualityPreset, ScreenRect, SensitivityProfile, Snapshot,
    TrackedHand, UsageStatistics, UserSettings,
};
use crate::preview::render_preview;
use crate::protocol::TrackingMessage;
use crate::store::{self, profile_for_executable, valid_name, validate_custom};
use crate::vision::{spawn_vision, VisionCommand, VisionConfig, VisionEvent, VisionSystem};

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
    SetControl(bool),
    TogglePause,
    EmergencyStop,
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
    CalibrationStart,
    CalibrationNext,
    CalibrationCancel,
    RecordGesture { name: String, threshold: f32 },
    DeleteGesture(String),
    AssignCommand { gesture: String, label: String },
    ExportGestures(std::path::PathBuf),
    ImportGestures(std::path::PathBuf),
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
    motion: MotionTracker,
    memory: HandMemory,
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
    last_stats_save: Instant,
}

struct Recorder {
    name: String,
    threshold: f32,
    frames: Vec<Vec<f32>>,
    started_ms: u64,
}

enum CalStep {
    Center {
        scale: f32,
        span: f32,
        samples: u32,
    },
    Range {
        until_ms: u64,
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
        travel: f32,
        samples: u32,
        last: Option<(f32, f32)>,
        scale: f32,
        span: f32,
    },
    Pinch {
        left: u32,
        ratios: Vec<f32>,
        down: bool,
        scale: f32,
        span: f32,
    },
}

struct CalSession {
    step: CalStep,
}

async fn run(
    mut commands: mpsc::Receiver<EngineCommand>,
    snapshot: Arc<RwLock<Snapshot>>,
) -> Result<(), CoreError> {
    let (settings, calibration, profiles, custom, stats) = store::load_all();
    let (input, ready) = spawn_input();
    let input_ok = match ready.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => true,
        Ok(Err(message)) => {
            tracing::error!(error = %message, "Windows input is unavailable");
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
        motion: MotionTracker::default(),
        memory: HandMemory::new(180),
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
            "Системный ввод недоступен. Камера и распознавание работают, команды Windows — нет."
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
        last_stats_save: Instant::now(),
    };
    session.log("AIR CONTROL готов. Камера запускается только по вашей команде.");
    session.publish();
    let mut ticker = tokio::time::interval(Duration::from_millis(250));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let VisionSystem {
        commands: vision_tx,
        mut tracking,
        mut events,
    } = vision;

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
                let _ = vision
                    .send(VisionCommand::Start(self.vision_config()))
                    .await;
            }
            EngineCommand::StopCamera => {
                self.camera_on = false;
                self.control = false;
                self.send_input(InputMsg::ReleaseAll);
                self.camera_status = "Камера выключена".into();
                self.ipc_ok = false;
                self.vision_pid = 0;
                let _ = vision.send(VisionCommand::Stop).await;
                self.log("Камера остановлена, удержание кнопок снято.");
            }
            EngineCommand::SetControl(enabled) => {
                if enabled && !self.camera_on {
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
                        "Управление Windows включено."
                    } else {
                        "Управление Windows выключено."
                    });
                }
            }
            EngineCommand::TogglePause => {
                self.paused = !self.paused;
                if self.paused {
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
                self.send_input(InputMsg::ReleaseAll);
                self.released = true;
                self.command_status = "остановлено".to_string();
                self.log("Аварийная остановка. Команды заблокированы, кнопки отпущены.");
            }
            EngineCommand::SelectCamera(index) => {
                self.settings.camera_index = index.max(0);
                self.persist_settings();
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
                    self.settings.profile = id;
                    self.settings.auto_context = false;
                    self.persist_settings();
                    self.profiles_revision = self.profiles_revision.wrapping_add(1);
                }
            }
            EngineCommand::SetAutoContext(value) => {
                self.settings.auto_context = value;
                self.persist_settings();
                self.profiles_revision = self.profiles_revision.wrapping_add(1);
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
            EngineCommand::CalibrationStart => {
                self.calibration_session = Some(CalSession {
                    step: CalStep::Center {
                        scale: 0.0,
                        span: 0.0,
                        samples: 0,
                    },
                });
                self.log("Калибровка: держите открытую ладонь в удобном центре.");
            }
            EngineCommand::CalibrationNext => self.advance_calibration(),
            EngineCommand::CalibrationCancel => {
                self.calibration_session = None;
                self.log("Калибровка отменена. Сохранённый профиль не изменён.");
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
            EngineCommand::Note(message) => self.log(&message),
            EngineCommand::Shutdown => return Flow::Stop,
        }
        self.publish();
        Flow::Continue
    }

    fn on_vision(&mut self, event: VisionEvent) {
        match event {
            VisionEvent::Hello { pid, mediapipe } => {
                self.vision_pid = pid;
                self.ipc_ok = true;
                self.log(&format!("Процесс зрения {pid}, MediaPipe {mediapipe}."));
            }
            VisionEvent::Cameras(devices) => {
                self.cameras = devices
                    .into_iter()
                    .map(|device| CameraInfo {
                        index: device.index,
                        label: format!("{}: {}", device.index, device.name),
                        selected: device.index == self.settings.camera_index,
                    })
                    .collect();
                self.cameras_revision = self.cameras_revision.wrapping_add(1);
                if self.cameras.is_empty() {
                    self.camera_status = "Камеры не найдены".into();
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
                    "streaming" => format!("Поток {width}×{height} @ {fps}"),
                    "reconnecting" => "Камера потеряна, переподключение".into(),
                    "opening" | "warming_up" => "Камера открывается".into(),
                    "waiting" => "Ожидание настройки камеры".into(),
                    other => format!("{other} {message}"),
                };
                if !message.is_empty() && state != "streaming" {
                    self.log(&format!("{state}: {message}"));
                }
            }
            VisionEvent::Error { code, message } => {
                self.error = format!("{code}: {message}");
                self.log(&self.error.clone());
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
                self.context_profile = if pid == self.our_pid {
                    self.context_profile.clone()
                } else {
                    profile_for_executable(&name).to_string()
                };
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
        self.publish();
    }

    fn on_tracking(&mut self, message: TrackingMessage) {
        let started = Instant::now();
        let now = self.now_ms();
        self.ipc_ok = true;
        self.capture_ms = message.capture_ms;
        self.inference_ms = message.inference_ms;
        let wall = wall_ms();
        self.ipc_ms = wall.saturating_sub(message.timestamp_ms).min(5_000) as f32;
        self.frame_times.push_back(now);
        while self.frame_times.len() > 30 {
            self.frame_times.pop_front();
        }
        if let (Some(first), Some(last)) = (self.frame_times.front(), self.frame_times.back()) {
            let span = last.saturating_sub(*first);
            if span > 0 && self.frame_times.len() > 1 {
                self.fps = (self.frame_times.len() - 1) as f32 * 1000.0 / span as f32;
            }
        }

        let incoming = message
            .hands
            .iter()
            .map(|hand| TrackedHand {
                side: HandSide::parse(&hand.handedness),
                score: hand.score,
                landmarks: hand.landmarks,
            })
            .collect();
        let visible = self.memory.update(now, incoming);
        let hands = select_hands(&visible, self.settings.primary_hand);
        self.observe_calibration(&hands, now);
        self.observe_recording(&hands, now);

        let profile_id = self.active_profile_id();
        let pose = hands
            .first()
            .and_then(|hand| classify_static(&hand.landmarks, self.calibration.pinch_ratio));
        let secondary = hands
            .get(1)
            .and_then(|hand| classify_static(&hand.landmarks, self.calibration.pinch_ratio));
        let mut motion = self.motion.push(now, &hands);
        let primary_points = pose
            .as_ref()
            .is_some_and(|item| item.id == "index_point" || item.id == "pinch");
        if primary_points {
            motion.retain(|event| {
                !matches!(
                    event,
                    MotionEvent::ZoomIn
                        | MotionEvent::ZoomOut
                        | MotionEvent::MoveLeft
                        | MotionEvent::MoveRight
                )
            });
        }
        if let Some(hand) = hands.first() {
            self.history.push_back(normalize_landmarks(&hand.landmarks));
            while self.history.len() > 12 {
                self.history.pop_front();
            }
        } else {
            self.history.clear();
        }

        let hand_score = hands.first().map(|hand| hand.score).unwrap_or(0.0);
        let penalty = hands
            .first()
            .map(|hand| edge_penalty(&hand.landmarks))
            .unwrap_or(0.0);
        let span_penalty = self.span_penalty(hands.first());
        self.hand_status = match hands.as_slice() {
            [] => "Рука не видна".into(),
            [one] => format!(
                "{} · уверенность {:.0}%",
                one.side.label(),
                one.score * 100.0
            ),
            [primary, _] => format!("Две руки · ведёт {}", primary.side.label().to_lowercase()),
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
        self.rejection.clear();
        if penalty >= 0.25 && pose.is_some() {
            self.rejection = "edge".into();
        }

        let pointer_binding = self
            .binding(&profile_id, "pinch")
            .map(|binding| binding.command.clone())
            .unwrap_or(CommandSpec::LeftClick);
        let pinch_is_click = pose.as_ref().is_some_and(|item| item.id == "pinch")
            && is_pointer_command(&pointer_binding);
        let tracking =
            self.control && !self.paused && self.input_ok && (primary_points || pinch_is_click);
        let point = hands
            .first()
            .map(|hand| (hand.landmarks[8][0], hand.landmarks[8][1]));
        let click_allowed = confidence >= self.settings.confidence && self.input_ok;
        let dispatch_started = Instant::now();
        let update = if tracking {
            self.pointer.update(&PointerInput {
                now_ms: now,
                tracking: true,
                point,
                pinch: pinch_is_click,
                click_allowed,
                screen: self.screen,
                range: self.calibration.range(),
                speed: self.settings.speed,
                acceleration: self.settings.acceleration,
                deadzone: self.settings.deadzone,
                smoothness: self.settings.smoothness,
                profile: self.settings.sensitivity,
            })
        } else {
            self.pointer.update(&PointerInput {
                now_ms: now,
                tracking: false,
                point: None,
                pinch: false,
                click_allowed: false,
                screen: self.screen,
                range: self.calibration.range(),
                speed: self.settings.speed,
                acceleration: self.settings.acceleration,
                deadzone: self.settings.deadzone,
                smoothness: self.settings.smoothness,
                profile: self.settings.sensitivity,
            })
        };
        if update.double_pinch {
            label = "double_pinch".into();
        }
        for event in &update.events {
            if self.control && !self.paused {
                self.apply_pointer(event);
            }
        }

        if self.control && !self.paused && self.input_ok {
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
                if item.id != "index_point" && !(item.id == "pinch" && pinch_is_click) {
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
        } else if pose.is_some() && confidence >= 0.55 {
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

        let draw_cursor = point.filter(|_| self.settings.show_camera || message.frame_id % 2 == 0);
        let jpeg = message
            .preview_jpeg
            .as_deref()
            .filter(|_| self.settings.show_camera);
        if jpeg.is_some() || !self.settings.show_camera && message.frame_id % 2 == 0 {
            if let Some(image) = render_preview(
                jpeg,
                &hands,
                self.calibration.range(),
                draw_cursor,
                self.settings.show_camera,
            ) {
                self.preview = Some(Arc::new(image));
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
        if gesture == "double_pinch"
            && matches!(
                binding.command,
                CommandSpec::DoubleClick | CommandSpec::LeftClick
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
        let minimum = self
            .binding(&self.active_profile_id(), gesture)
            .map(|item| item.min_confidence)
            .unwrap_or(self.settings.confidence);
        let minimum = minimum.max(self.settings.confidence);
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
                self.rejection = reason.into();
                self.stats.rejected = self.stats.rejected.saturating_add(1);
                self.command_status = "отклонено".into();
            }
        }
    }

    fn apply_pointer(&mut self, event: &PointerEvent) {
        let message = match event {
            PointerEvent::Move(x, y) => InputMsg::Move(*x, *y),
            PointerEvent::PressLeft => InputMsg::PressLeft,
            PointerEvent::ReleaseLeft => InputMsg::ReleaseLeft,
            PointerEvent::ClickLeft => {
                self.stats.clicks = self.stats.clicks.saturating_add(1);
                self.stats.commands_dispatched = self.stats.commands_dispatched.saturating_add(1);
                self.command = "Левый клик".into();
                self.command_status = "выполнено".into();
                InputMsg::ClickLeft
            }
        };
        self.send_input(message);
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

    fn send_input(&self, message: InputMsg) {
        if let Some(tx) = &self.input {
            let _ = tx.send_timeout(message, Duration::from_millis(30));
        }
    }

    fn vision_config(&self) -> VisionConfig {
        let (width, height, fps, every) = self.settings.quality.params();
        VisionConfig {
            camera_index: self.settings.camera_index,
            width,
            height,
            fps,
            preview: self.settings.show_camera,
            preview_every: every,
            mirror: self.settings.mirror,
        }
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
        let Some(hand) = hands.first() else { return };
        let Some(session) = self.calibration_session.as_mut() else {
            return;
        };
        let scale = crate::gestures::distance(hand.landmarks[0], hand.landmarks[9]);
        let span = crate::gestures::distance(hand.landmarks[8], hand.landmarks[20]);
        match &mut session.step {
            CalStep::Center {
                scale: scale_sum,
                span: span_sum,
                samples,
            } => {
                *scale_sum += scale;
                *span_sum += span;
                *samples += 1;
            }
            CalStep::Range {
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
            }
            CalStep::Pinch {
                left, ratios, down, ..
            } => {
                if let Some(pose) = classify_static(&hand.landmarks, 0.55) {
                    let ratio = crate::gestures::distance(hand.landmarks[4], hand.landmarks[8])
                        / scale.max(0.02);
                    if pose.id == "pinch" {
                        ratios.push(ratio);
                        *down = true;
                    } else if *down {
                        *down = false;
                        *left = left.saturating_sub(1);
                    }
                }
            }
        }
        let finished = matches!(
            self.calibration_session,
            Some(CalSession {
                step: CalStep::Pinch { left: 0, .. }
            })
        );
        if finished {
            if let Some(CalSession {
                step:
                    CalStep::Pinch {
                        ratios,
                        scale,
                        span,
                        ..
                    },
                ..
            }) = self.calibration_session.take()
            {
                let mean = if ratios.is_empty() {
                    0.34
                } else {
                    ratios.iter().sum::<f32>() / ratios.len() as f32
                };
                self.calibration.pinch_ratio = (mean * 1.35).clamp(0.18, 0.62);
                self.calibration.hand_scale = scale;
                self.calibration.finger_span = span;
                self.calibration.valid = true;
                let _ = store::save_calibration(&self.calibration);
                self.log("Калибровка сохранена. Изменены зона, порог щипка и масштаб руки. Модель не обучалась.");
            }
        }
    }

    fn advance_calibration(&mut self) {
        let now = self.now_ms();
        let next = match self
            .calibration_session
            .as_ref()
            .map(|session| &session.step)
        {
            Some(CalStep::Center {
                scale,
                span,
                samples,
            }) if *samples >= 8 => Some(CalStep::Range {
                until_ms: now + 3_000,
                min_x: 1.0,
                min_y: 1.0,
                max_x: 0.0,
                max_y: 0.0,
                travel: 0.0,
                samples: 0,
                last: None,
                scale: *scale / *samples as f32,
                span: *span / *samples as f32,
            }),
            Some(CalStep::Center { .. }) => {
                self.error = "Нужно больше кадров с открытой ладонью.".into();
                None
            }
            _ => None,
        };
        if let Some(step) = next {
            self.calibration_session = Some(CalSession { step });
            self.error.clear();
            self.log("Очертите рукой удобную рабочую область.");
        }
    }

    fn advance_range_timer(&mut self) {
        let now = self.now_ms();
        let ready = matches!(&self.calibration_session, Some(CalSession { step: CalStep::Range { until_ms, samples, .. } }) if now >= *until_ms && *samples >= 5);
        if !ready {
            return;
        }
        if let Some(CalSession {
            step:
                CalStep::Range {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                    travel,
                    samples,
                    scale,
                    span,
                    ..
                },
        }) = self.calibration_session.take()
        {
            let pad_x = (max_x - min_x) * 0.08;
            let pad_y = (max_y - min_y) * 0.08;
            self.calibration.min_x = (min_x - pad_x).clamp(0.0, 0.8);
            self.calibration.min_y = (min_y - pad_y).clamp(0.0, 0.8);
            self.calibration.max_x = (max_x + pad_x).clamp(0.2, 1.0);
            self.calibration.max_y = (max_y + pad_y).clamp(0.2, 1.0);
            let average = travel / samples.max(1) as f32;
            self.settings.speed = (1.0 + average * 6.0).clamp(0.9, 1.7);
            self.settings.deadzone = if average < 0.008 { 1.0 } else { 2.2 };
            self.persist_settings();
            self.calibration_session = Some(CalSession {
                step: CalStep::Pinch {
                    left: 3,
                    ratios: Vec::new(),
                    down: false,
                    scale,
                    span,
                },
            });
            self.log("Три раза сожмите большой и указательный пальцы.");
        }
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
        let gesture = CustomGesture {
            name: recorder.name,
            threshold: recorder.threshold,
            frames: resample(&recorder.frames, 12),
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
            *existing = gesture.clone();
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
                    custom: true,
                });
            }
        }
        rows
    }

    fn calibration_view(&self) -> (String, String, f32, bool) {
        match &self.calibration_session {
            Some(CalSession { step: CalStep::Center { samples, .. } }) => ("Шаг 1 из 3 · центр".into(), format!("Держите открытую ладонь неподвижно. Кадров: {samples}. Это настройка порогов, не обучение модели."), (*samples as f32 / 20.0).min(0.33), true),
            Some(CalSession { step: CalStep::Range { .. } }) => ("Шаг 2 из 3 · рабочая область".into(), "Медленно очертите область, в которой удобно двигать рукой.".into(), 0.66, true),
            Some(CalSession { step: CalStep::Pinch { left, .. } }) => ("Шаг 3 из 3 · щипок".into(), format!("Осталось щипков: {left}. Порог будет чуть шире вашего среднего."), 0.9, true),
            None if self.calibration.valid => ("Калибровка сохранена".into(), "Зона руки и порог щипка используются курсором. Модель MediaPipe не изменялась.".into(), 1.0, false),
            None => ("Калибровка ещё не выполнена".into(), "Мастер измеряет зону руки, размах пальцев и порог щипка. Модель распознавания не обучается.".into(), 0.0, false),
        }
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
