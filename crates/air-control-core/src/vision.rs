//! Local vision process supervisor.
//!
//! Python is allowed to send landmarks and camera status only. A tracking
//! message cannot ask the core to run a system command.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, watch};

use crate::error::CoreError;
use crate::protocol::{parse_vision_line, Incoming, ParseOutcome, TrackingMessage};

#[derive(Debug, Clone)]
pub struct VisionConfig {
    pub camera_index: i32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub preview: bool,
    pub preview_every: u32,
    pub mirror: bool,
}

#[derive(Debug)]
pub enum VisionCommand {
    Prepare,
    Start(VisionConfig),
    Update(VisionConfig),
    Release,
    ListCameras,
    Stop,
    Shutdown,
}

#[derive(Debug)]
pub enum VisionEvent {
    Hello {
        pid: u32,
        mediapipe: String,
    },
    Cameras(Vec<crate::protocol::CameraDevice>),
    Status {
        state: String,
        message: String,
        width: u32,
        height: u32,
        fps: u32,
    },
    Error {
        code: String,
        message: String,
    },
    Log(String),
    Exited {
        code: Option<i32>,
    },
    Restarting {
        reason: String,
    },
    Failed {
        message: String,
    },
}

pub struct VisionSystem {
    pub commands: mpsc::Sender<VisionCommand>,
    pub tracking: watch::Receiver<Option<TrackingMessage>>,
    pub events: mpsc::Receiver<VisionEvent>,
}

pub fn spawn_vision() -> VisionSystem {
    let (commands, command_rx) = mpsc::channel(32);
    let (tracking_tx, tracking) = watch::channel(None);
    let (event_tx, events) = mpsc::channel(64);
    tokio::spawn(async move {
        if let Err(err) = supervise(command_rx, tracking_tx, event_tx.clone()).await {
            let _ = event_tx
                .send(VisionEvent::Failed {
                    message: err.to_string(),
                })
                .await;
        }
    });
    VisionSystem {
        commands,
        tracking,
        events,
    }
}

struct Host {
    generation: u64,
    child: Child,
    stdin: ChildStdin,
}

async fn supervise(
    mut commands: mpsc::Receiver<VisionCommand>,
    tracking_tx: watch::Sender<Option<TrackingMessage>>,
    events: mpsc::Sender<VisionEvent>,
) -> Result<(), CoreError> {
    let mut desired: Option<VisionConfig> = None;
    let mut keep_alive = false;
    let mut host: Option<Host> = None;
    let mut generation = 0u64;
    let mut failures = 0u32;
    let mut last_rx = Instant::now();
    let mut restart_at: Option<Instant> = None;
    let (line_tx, mut lines) = mpsc::channel::<(u64, String)>(32);
    let (exit_tx, mut exits) = mpsc::channel::<(u64, Option<i32>)>(8);
    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(VisionCommand::Prepare) => {
                        keep_alive = true;
                        if host.is_none() {
                            generation += 1;
                            match launch(generation, &line_tx, &exit_tx, None).await {
                                Ok(started) => {
                                    host = Some(started);
                                    last_rx = Instant::now();
                                }
                                Err(err) => {
                                    let _ = events.send(VisionEvent::Failed { message: err.to_string() }).await;
                                }
                            }
                        } else if let Some(current) = host.as_mut() {
                            let _ = write_command(
                                &mut current.stdin,
                                &serde_json::json!({"version": 1, "type": "list_cameras"}),
                            )
                            .await;
                        }
                    }
                    Some(VisionCommand::Start(config)) | Some(VisionCommand::Update(config)) => {
                        desired = Some(config.clone());
                        keep_alive = true;
                        failures = 0;
                        if host.is_none() {
                            generation += 1;
                            match launch(generation, &line_tx, &exit_tx, Some(&config)).await {
                                Ok(started) => {
                                    host = Some(started);
                                    last_rx = Instant::now();
                                }
                                Err(err) => {
                                    failures += 1;
                                    let _ = events.send(VisionEvent::Failed { message: err.to_string() }).await;
                                    desired = None;
                                }
                            }
                        } else if let Some(current) = host.as_mut() {
                            let _ = write_command(&mut current.stdin, &configure_json(&config)).await;
                        }
                    }
                    Some(VisionCommand::Release) => {
                        desired = None;
                        if let Some(current) = host.as_mut() {
                            let _ = write_command(
                                &mut current.stdin,
                                &serde_json::json!({"version": 1, "type": "release"}),
                            )
                            .await;
                        }
                    }
                    Some(VisionCommand::ListCameras) => {
                        if let Some(current) = host.as_mut() {
                            let _ = write_command(
                                &mut current.stdin,
                                &serde_json::json!({"version": 1, "type": "list_cameras"}),
                            )
                            .await;
                        }
                    }
                    Some(VisionCommand::Stop) => {
                        desired = None;
                        stop_host(host.take()).await;
                    }
                    None => {
                        stop_host(host.take()).await;
                        break;
                    }
                    Some(VisionCommand::Shutdown) => {
                        stop_host(host.take()).await;
                        break;
                    }
                }
            }
            line = lines.recv() => {
                if let Some((id, line)) = line {
                    if host.as_ref().is_some_and(|item| item.generation == id) {
                        last_rx = Instant::now();
                        handle_line(&line, &tracking_tx, &events).await;
                    }
                }
            }
            exit = exits.recv() => {
                if let Some((id, code)) = exit {
                    if host.as_ref().is_some_and(|item| item.generation == id) {
                        host = None;
                        let _ = events.send(VisionEvent::Exited { code }).await;
                        if desired.is_some() || keep_alive {
                            failures += 1;
                            if failures >= 6 {
                                desired = None;
                                keep_alive = false;
                                let _ = events.send(VisionEvent::Failed { message: "процесс зрения остановлен после повторных сбоев".into() }).await;
                            } else {
                                let delay = 500u64.saturating_mul(1 << failures.min(4));
                                restart_at = Some(Instant::now() + Duration::from_millis(delay.min(5_000)));
                                let _ = events.send(VisionEvent::Restarting { reason: format!("процесс завершился с кодом {code:?}") }).await;
                            }
                        }
                    }
                }
            }
            _ = ticker.tick() => {
                if (desired.is_some() || keep_alive) && host.is_some() && last_rx.elapsed() > Duration::from_secs(45) {
                    let _ = events.send(VisionEvent::Restarting { reason: "нет сообщений процесса зрения 45 секунд".into() }).await;
                    let old = host.take();
                    stop_host(old).await;
                    failures += 1;
                    if failures >= 6 {
                        desired = None;
                        keep_alive = false;
                        let _ = events.send(VisionEvent::Failed { message: "процесс зрения замолчал и не поднялся снова".into() }).await;
                    } else {
                        restart_at = Some(Instant::now());
                    }
                }
                if let Some(when) = restart_at {
                    if when <= Instant::now() {
                        restart_at = None;
                        if desired.is_some() || keep_alive {
                            generation += 1;
                            let open = desired.clone();
                            match launch(generation, &line_tx, &exit_tx, open.as_ref()).await {
                                Ok(started) => {
                                    host = Some(started);
                                    last_rx = Instant::now();
                                }
                                Err(err) => {
                                    failures += 1;
                                    restart_at = Some(Instant::now() + Duration::from_secs(2));
                                    let _ = events.send(VisionEvent::Failed { message: err.to_string() }).await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

async fn handle_line(
    line: &str,
    tracking_tx: &watch::Sender<Option<TrackingMessage>>,
    events: &mpsc::Sender<VisionEvent>,
) {
    if line.starts_with("{") {
        match parse_vision_line(line) {
            Ok(ParseOutcome::Message(Incoming::Tracking(message))) => {
                let _ = tracking_tx.send(Some(message));
            }
            Ok(ParseOutcome::Message(other)) => {
                let _ = events.try_send(event_from(other));
            }
            Ok(ParseOutcome::Ignored { reason }) => {
                let _ = events.try_send(VisionEvent::Log(reason));
            }
            Err(err) => {
                let _ = events.try_send(VisionEvent::Error {
                    code: "bad_ipc".into(),
                    message: err.to_string(),
                });
            }
        }
    } else if !line.trim().is_empty() {
        let _ = events.try_send(VisionEvent::Log(line.chars().take(240).collect()));
    }
}

fn event_from(message: Incoming) -> VisionEvent {
    match message {
        Incoming::Hello { pid, mediapipe } => VisionEvent::Hello { pid, mediapipe },
        Incoming::Cameras { devices } => VisionEvent::Cameras(devices),
        Incoming::CameraStatus {
            state,
            message,
            width,
            height,
            fps,
            ..
        } => VisionEvent::Status {
            state,
            message,
            width,
            height,
            fps,
        },
        Incoming::Heartbeat { .. } => VisionEvent::Log("heartbeat".into()),
        Incoming::Error { code, message } => VisionEvent::Error { code, message },
        Incoming::Goodbye => VisionEvent::Log("goodbye".into()),
        Incoming::Tracking(_) => VisionEvent::Log("tracking".into()),
    }
}

async fn launch(
    generation: u64,
    lines: &mpsc::Sender<(u64, String)>,
    exits: &mpsc::Sender<(u64, Option<i32>)>,
    open: Option<&VisionConfig>,
) -> Result<Host, CoreError> {
    let vision_dir =
        locate_vision().ok_or_else(|| CoreError::Vision("папка vision не найдена".into()))?;
    let python = locate_python(&vision_dir).ok_or_else(|| {
        CoreError::Vision(
            "Python 3.11/3.12 не найден. Запустите scripts/setup.ps1 или scripts/setup.sh".into(),
        )
    })?;
    let mut command = Command::new(python);
    command
        .arg("-m")
        .arg("air_control_vision")
        .current_dir(&vision_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|err| CoreError::Vision(err.to_string()))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| CoreError::Vision("stdin процесса зрения недоступен".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::Vision("stdout процесса зрения недоступен".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CoreError::Vision("stderr процесса зрения недоступен".into()))?;
    let line_tx = lines.clone();
    let exit_tx = exits.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if line_tx.send((generation, line)).await.is_err() {
                break;
            }
        }
        let _ = exit_tx.send((generation, None)).await;
    });
    let log_tx = lines.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if log_tx.send((generation, line)).await.is_err() {
                break;
            }
        }
    });
    let mut host = Host {
        generation,
        child,
        stdin,
    };
    let _ = write_command(
        &mut host.stdin,
        &serde_json::json!({"version": 1, "type": "list_cameras"}),
    )
    .await;
    if let Some(config) = open {
        let _ = write_command(&mut host.stdin, &configure_json(config)).await;
    }
    Ok(host)
}

async fn stop_host(host: Option<Host>) {
    let Some(mut host) = host else { return };
    let _ = write_command(
        &mut host.stdin,
        &serde_json::json!({"version": 1, "type": "shutdown"}),
    )
    .await;
    drop(host.stdin);
    let finished = tokio::time::timeout(Duration::from_millis(700), host.child.wait()).await;
    if finished.is_err() {
        let _ = host.child.kill().await;
        let _ = host.child.wait().await;
    }
}

async fn write_command(stdin: &mut ChildStdin, value: &serde_json::Value) -> Result<(), CoreError> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    stdin
        .write_all(&bytes)
        .await
        .map_err(|err| CoreError::Vision(err.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|err| CoreError::Vision(err.to_string()))?;
    Ok(())
}

fn configure_json(config: &VisionConfig) -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "type": "configure",
        "camera_index": config.camera_index,
        "width": config.width,
        "height": config.height,
        "fps": config.fps,
        "preview": config.preview,
        "preview_every": config.preview_every,
        "mirror": config.mirror,
    })
}

pub fn locate_vision() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("AIR_CONTROL_ROOT") {
        let vision = PathBuf::from(root).join("vision");
        if vision_marker(&vision) {
            return Some(vision);
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        let mut dir = executable;
        while dir.pop() {
            let vision = dir.join("vision");
            if vision_marker(&vision) {
                return Some(vision);
            }
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vision");
    vision_marker(&bundled).then_some(bundled)
}

fn vision_marker(vision: &Path) -> bool {
    vision
        .join("air_control_vision")
        .join("__main__.py")
        .exists()
}

pub fn locate_python(vision_dir: &Path) -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("AIR_CONTROL_PYTHON") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Some(path);
        }
    }
    // Windows venv
    let win = vision_dir.join(".venv").join("Scripts").join("python.exe");
    if win.exists() {
        return Some(win);
    }
    // macOS / Linux venv
    for name in ["python3", "python"] {
        let unix = vision_dir.join(".venv").join("bin").join(name);
        if unix.exists() {
            return Some(unix);
        }
    }
    #[cfg(windows)]
    for version in ["-3.11", "-3.12"] {
        if let Some(path) = python_from_launcher(version) {
            return Some(path);
        }
    }
    for command in ["python3", "python"] {
        if let Some(path) = python_executable(command) {
            return Some(path);
        }
    }
    None
}

fn python_executable(command: &str) -> Option<PathBuf> {
    let output = std::process::Command::new(command)
        .args([
            "-c",
            "import sys; assert (3,11) <= sys.version_info[:2] < (3,13); print(sys.executable)",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(text.trim());
    path.exists().then_some(path)
}

#[cfg(windows)]
fn python_from_launcher(version: &str) -> Option<PathBuf> {
    let output = std::process::Command::new("py")
        .args([version, "-c", "import sys; print(sys.executable)"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(text.trim());
    path.exists().then_some(path)
}
