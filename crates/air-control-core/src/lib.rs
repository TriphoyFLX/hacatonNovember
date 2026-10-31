//! AIR CONTROL core.
//!
//! The vision process is untrusted: it may only describe hands. Commands are
//! chosen here and executed by a single input path with an allowlist.
//! Windows and macOS share the engine; OS-specific bits live behind `cfg`.

mod coach;
mod commands;
mod cursor;
mod engine;
mod error;
mod filter;
mod gestures;
mod input;
mod midi;
mod model;
mod preview;
mod protocol;
mod session;
mod stage;
mod store;
mod uia;
mod vision;
mod voice;

pub use commands::{
    command_from_id, command_id_from_label, command_label, voice_phrase_catalog, CommandSpec,
    COMMAND_CHOICES,
};
pub use engine::{start_engine, CommandSender, EngineCommand, EngineHandle};
pub use error::CoreError;
pub use input::enable_dpi_awareness as enable_per_monitor_dpi;
pub use model::{CameraInfo, GestureInfo, PreviewImage, ProfileInfo, Snapshot};
pub use protocol::{describe_vision_line, parse_voice_line};
pub use store::{app_dir, log_dir};
pub use vision::{locate_python, locate_vision};
pub use voice::{describe_intent, interpret_line, spawn_voice, voice_process_planned};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL_VERSION: u32 = 1;
