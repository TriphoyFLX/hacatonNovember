//! Local voice worker. The child process sends text only.
//! The core maps that text through the command allowlist.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

use crate::commands::{command_label, intent_from_phrase, CommandSpec};
use crate::error::CoreError;
use crate::protocol::{parse_voice_line, Incoming, ParseOutcome};

const MODEL_NAME: &str = "vosk-model-small-ru-0.22";

#[derive(Debug, Clone)]
pub struct VoiceIntent {
    pub text: String,
    pub confidence: f32,
    pub command: Option<CommandSpec>,
}

#[derive(Debug)]
pub enum VoiceCommand {
    /// Load the local model. Does not open the microphone.
    Prepare,
    /// Open the microphone.
    Start,
    /// Close the microphone. The process stays up.
    Stop,
    Shutdown,
}

#[derive(Debug)]
pub enum VoiceEvent {
    Log(String),
    Status {
        ready: Option<bool>,
        listening: Option<bool>,
        message: String,
    },
    Intent(VoiceIntent),
    Failed {
        message: String,
        ready: Option<bool>,
    },
}

pub struct VoiceSystem {
    pub commands: mpsc::Sender<VoiceCommand>,
    pub events: mpsc::Receiver<VoiceEvent>,
}

pub fn interpret_line(text: &str, confidence: f32) -> VoiceIntent {
    let command = if confidence >= 0.55 {
        intent_from_phrase(text)
    } else {
        None
    };
    VoiceIntent {
        text: text.chars().take(120).collect(),
        confidence: confidence.clamp(0.0, 1.0),
        command,
    }
}

pub fn describe_intent(intent: &VoiceIntent) -> String {
    match &intent.command {
        Some(command) => format!("услышал «{}» → {}", intent.text, command_label(command)),
        None => format!("услышал «{}» — нет в списке", intent.text),
    }
}

/// True when the Russian Vosk model is already on disk.
pub fn voice_process_planned() -> bool {
    locate_voice()
        .map(|dir| model_ready(&dir))
        .unwrap_or(false)
}

pub fn locate_voice() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("AIR_CONTROL_ROOT") {
        let voice = PathBuf::from(root).join("voice");
        if voice_marker(&voice) {
            return Some(voice);
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        let mut dir = executable;
        while dir.pop() {
            let voice = dir.join("voice");
            if voice_marker(&voice) {
                return Some(voice);
            }
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../voice");
    voice_marker(&bundled).then_some(bundled)
}

fn voice_marker(voice: &Path) -> bool {
    voice
        .join("air_control_voice")
        .join("__main__.py")
        .exists()
}

fn model_ready(voice_dir: &Path) -> bool {
    voice_dir
        .join("models")
        .join(MODEL_NAME)
        .join("conf")
        .join("model.conf")
        .exists()
}

pub fn spawn_voice() -> VoiceSystem {
    let (commands, command_rx) = mpsc::channel(8);
    let (event_tx, events) = mpsc::channel(16);
    tokio::spawn(async move {
        supervise(command_rx, event_tx).await;
    });
    VoiceSystem { commands, events }
}

struct Host {
    generation: u64,
    child: Child,
    stdin: ChildStdin,
}

async fn supervise(mut commands: mpsc::Receiver<VoiceCommand>, events: mpsc::Sender<VoiceEvent>) {
    let (line_tx, mut lines) = mpsc::channel::<(u64, String)>(32);
    let (exit_tx, mut exits) = mpsc::channel::<u64>(4);
    let mut host: Option<Host> = None;
    let mut generation = 0u64;
    let mut want_listen = false;
    let mut failures = 0u32;

    loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(VoiceCommand::Prepare) => {
                        if let Err(message) = ensure_host(&mut host, &mut generation, &line_tx, &exit_tx).await {
                            let _ = events.send(VoiceEvent::Failed { message, ready: Some(false) }).await;
                        } else if let Some(current) = host.as_mut() {
                            let _ = write_json(&mut current.stdin, "prepare").await;
                        }
                    }
                    Some(VoiceCommand::Start) => {
                        want_listen = true;
                        failures = 0;
                        if let Err(message) = ensure_host(&mut host, &mut generation, &line_tx, &exit_tx).await {
                            want_listen = false;
                            let _ = events.send(VoiceEvent::Failed { message, ready: None }).await;
                        } else if let Some(current) = host.as_mut() {
                            let _ = write_json(&mut current.stdin, "start").await;
                        }
                    }
                    Some(VoiceCommand::Stop) => {
                        want_listen = false;
                        if let Some(current) = host.as_mut() {
                            let _ = write_json(&mut current.stdin, "stop").await;
                        }
                    }
                    Some(VoiceCommand::Shutdown) | None => {
                        stop_host(host.take()).await;
                        break;
                    }
                }
            }
            line = lines.recv() => {
                let Some((gen, line)) = line else { break };
                if host.as_ref().map(|item| item.generation) != Some(gen) {
                    continue;
                }
                if let Some(event) = event_from_line(&line) {
                    if events.send(event).await.is_err() {
                        break;
                    }
                }
            }
            exit_gen = exits.recv() => {
                let Some(gen) = exit_gen else { break };
                if host.as_ref().map(|item| item.generation) != Some(gen) {
                    continue;
                }
                host = None;
                if want_listen && failures < 2 {
                    failures += 1;
                    if ensure_host(&mut host, &mut generation, &line_tx, &exit_tx).await.is_ok() {
                        if let Some(current) = host.as_mut() {
                            let _ = write_json(&mut current.stdin, "start").await;
                        }
                    } else {
                        want_listen = false;
                        let _ = events.send(VoiceEvent::Failed {
                            message: "Голос остановился.".into(),
                            ready: None,
                        }).await;
                    }
                } else if want_listen {
                    want_listen = false;
                    let _ = events.send(VoiceEvent::Failed {
                        message: "Голос остановился.".into(),
                        ready: None,
                    }).await;
                }
            }
        }
    }
}

fn event_from_line(line: &str) -> Option<VoiceEvent> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if !line.starts_with('{') {
        return Some(VoiceEvent::Log(line.chars().take(240).collect()));
    }
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let kind = value.get("type").and_then(|item| item.as_str()).unwrap_or("");
    let message = value
        .get("message")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .chars()
        .take(240)
        .collect::<String>();
    let ready = value.get("ready").and_then(|item| item.as_bool());
    match kind {
        "voice" => match parse_voice_line(line).ok()? {
            ParseOutcome::Message(Incoming::Voice { text, confidence }) => {
                Some(VoiceEvent::Intent(interpret_line(&text, confidence)))
            }
            _ => None,
        },
        "hello" | "status" => {
            let listening = value.get("listening").and_then(|item| item.as_bool());
            Some(VoiceEvent::Status {
                ready,
                listening,
                message,
            })
        }
        "error" => Some(VoiceEvent::Failed { message, ready }),
        "log" => Some(VoiceEvent::Log(message)),
        "goodbye" => Some(VoiceEvent::Log("голос остановлен".into())),
        _ => None,
    }
}

async fn ensure_host(
    host: &mut Option<Host>,
    generation: &mut u64,
    lines: &mpsc::Sender<(u64, String)>,
    exits: &mpsc::Sender<u64>,
) -> Result<(), String> {
    if host.is_some() {
        return Ok(());
    }
    *generation += 1;
    match launch(*generation, lines, exits).await {
        Ok(started) => {
            *host = Some(started);
            Ok(())
        }
        Err(err) => Err(err.to_string()),
    }
}

async fn launch(
    generation: u64,
    lines: &mpsc::Sender<(u64, String)>,
    exits: &mpsc::Sender<u64>,
) -> Result<Host, CoreError> {
    let voice_dir = locate_voice().ok_or_else(|| CoreError::Vision("папка voice не найдена".into()))?;
    let vision_hint = voice_dir
        .parent()
        .map(|root| root.join("vision"))
        .unwrap_or_else(|| voice_dir.clone());
    let python = crate::vision::locate_python(&vision_hint).ok_or_else(|| {
        CoreError::Vision(
            "Python 3.11/3.12 не найден. Запустите setup.ps1 рядом с программой.".into(),
        )
    })?;
    let mut command = Command::new(python);
    command
        .arg("-m")
        .arg("air_control_voice")
        .current_dir(&voice_dir)
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
        .ok_or_else(|| CoreError::Vision("stdin голоса недоступен".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::Vision("stdout голоса недоступен".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CoreError::Vision("stderr голоса недоступен".into()))?;
    let line_tx = lines.clone();
    let exit_tx = exits.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if line_tx.send((generation, line)).await.is_err() {
                break;
            }
        }
        let _ = exit_tx.send(generation).await;
    });
    let log_tx = lines.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if line.contains("LOG (") || line.contains("WARNING (") {
                continue;
            }
            if log_tx
                .send((generation, format!("голос: {line}")))
                .await
                .is_err()
            {
                break;
            }
        }
    });
    Ok(Host {
        generation,
        child,
        stdin,
    })
}

async fn stop_host(host: Option<Host>) {
    let Some(mut host) = host else { return };
    let _ = write_json(&mut host.stdin, "shutdown").await;
    drop(host.stdin);
    let finished = tokio::time::timeout(Duration::from_millis(700), host.child.wait()).await;
    if finished.is_err() {
        let _ = host.child.kill().await;
        let _ = host.child.wait().await;
    }
}

async fn write_json(stdin: &mut ChildStdin, kind: &str) -> Result<(), CoreError> {
    let line = format!("{{\"version\":1,\"type\":\"{kind}\"}}\n");
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|err| CoreError::Vision(err.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|err| CoreError::Vision(err.to_string()))?;
    Ok(())
}

pub fn intent_from_ndjson(line: &str) -> Option<VoiceIntent> {
    match parse_voice_line(line).ok()? {
        ParseOutcome::Message(Incoming::Voice { text, confidence }) => {
            Some(interpret_line(&text, confidence))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_maps_known_phrase() {
        let intent = interpret_line("открой проводник", 0.9);
        assert!(intent.command.is_some());
        assert!(describe_intent(&intent).contains("Проводник"));
    }

    #[test]
    fn ndjson_voice_line_maps() {
        let intent = intent_from_ndjson(
            r#"{"version":1,"type":"voice","text":"скопируй","confidence":0.9}"#,
        )
        .unwrap();
        assert!(intent.command.is_some());
    }

    #[test]
    fn low_confidence_does_not_map() {
        let intent = interpret_line("громче", 0.2);
        assert!(intent.command.is_none());
    }
}
