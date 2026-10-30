//! AIR CONTROL core.
//!
//! The vision process is untrusted: it may only describe hands. Commands are
//! chosen here and executed by a single input path with an allowlist.

mod commands;
mod cursor;
mod engine;
mod error;
mod filter;
mod gestures;
mod input;
mod model;
mod preview;
mod protocol;
mod store;
mod vision;

pub use commands::{
    command_from_id, command_id_from_label, command_label, CommandSpec, COMMAND_CHOICES,
};
pub use engine::{start_engine, CommandSender, EngineCommand, EngineHandle};
pub use error::CoreError;
pub use input::enable_dpi_awareness as enable_per_monitor_dpi;
pub use model::{CameraInfo, GestureInfo, PreviewImage, ProfileInfo, Snapshot};
pub use protocol::describe_vision_line;
pub use store::{app_dir, log_dir};
pub use vision::{locate_python, locate_vision};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL_VERSION: u32 = 1;
