//! The only place that injects mouse, keyboard, volume, and window commands.
//! Gesture code never calls these functions directly.

use std::mem::size_of;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

use crate::commands::{CommandSpec, MacroStep};
use crate::error::CoreError;
use crate::model::ScreenRect;

#[derive(Debug)]
pub enum InputMsg {
    Move(i32, i32),
    PressLeft,
    ReleaseLeft,
    ClickLeft,
    ClickRight,
    DoubleClick,
    Command(CommandSpec),
    ReleaseAll,
    Shutdown,
}

pub fn spawn_input() -> (
    Sender<InputMsg>,
    std::sync::mpsc::Receiver<Result<(), String>>,
) {
    let (tx, rx) = crossbeam_channel::bounded(128);
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    thread::Builder::new()
        .name("air-input".into())
        .spawn(move || match InputExecutor::new() {
            Ok(executor) => {
                let _ = ready_tx.send(Ok(()));
                input_loop(executor, rx);
            }
            Err(err) => {
                let _ = ready_tx.send(Err(err.to_string()));
            }
        })
        .ok();
    (tx, ready_rx)
}

fn input_loop(mut executor: InputExecutor, rx: Receiver<InputMsg>) {
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(InputMsg::Shutdown) => {
                executor.release_all();
                break;
            }
            Ok(other) => {
                let mut message = other;
                if let InputMsg::Move(mut x, mut y) = message {
                    while let Ok(next) = rx.try_recv() {
                        match next {
                            InputMsg::Move(nx, ny) => {
                                x = nx;
                                y = ny;
                            }
                            InputMsg::Shutdown => {
                                let _ = executor.move_to(x, y);
                                executor.release_all();
                                return;
                            }
                            next => {
                                let _ = executor.move_to(x, y);
                                message = next;
                                break;
                            }
                        }
                    }
                    if matches!(message, InputMsg::Move(_, _)) {
                        let _ = executor.move_to(x, y);
                        executor.tick();
                        continue;
                    }
                }
                dispatch(&mut executor, message);
                executor.tick();
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => executor.tick(),
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                executor.release_all();
                break;
            }
        }
    }
}

fn dispatch(executor: &mut InputExecutor, message: InputMsg) {
    let result = match message {
        InputMsg::Move(x, y) => executor.move_to(x, y),
        InputMsg::PressLeft => executor.press_left(),
        InputMsg::ReleaseLeft => executor.release_left(),
        InputMsg::ClickLeft => executor.click(Button::Left),
        InputMsg::ClickRight => executor.click(Button::Right),
        InputMsg::DoubleClick => executor.double_click(),
        InputMsg::Command(command) => executor.command(&command),
        InputMsg::ReleaseAll | InputMsg::Shutdown => {
            executor.release_all();
            Ok(())
        }
    };
    if let Err(err) = result {
        tracing::warn!(error = %err, "input command failed");
    }
}

struct InputExecutor {
    enigo: Enigo,
    left_down: bool,
    left_since: Option<Instant>,
    held_keys: Vec<Key>,
}

impl InputExecutor {
    fn new() -> Result<Self, CoreError> {
        #[cfg(target_os = "macos")]
        prompt_accessibility_if_needed();
        let enigo =
            Enigo::new(&Settings::default()).map_err(|err| CoreError::Input(err.to_string()))?;
        Ok(Self {
            enigo,
            left_down: false,
            left_since: None,
            held_keys: Vec::new(),
        })
    }

    fn move_to(&mut self, x: i32, y: i32) -> Result<(), CoreError> {
        self.enigo
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|err| CoreError::Input(err.to_string()))
    }

    fn press_left(&mut self) -> Result<(), CoreError> {
        if !self.left_down {
            self.enigo
                .button(Button::Left, Direction::Press)
                .map_err(|err| CoreError::Input(err.to_string()))?;
            self.left_down = true;
            self.left_since = Some(Instant::now());
        }
        Ok(())
    }

    fn release_left(&mut self) -> Result<(), CoreError> {
        if self.left_down {
            self.enigo
                .button(Button::Left, Direction::Release)
                .map_err(|err| CoreError::Input(err.to_string()))?;
            self.left_down = false;
            self.left_since = None;
        }
        Ok(())
    }

    fn click(&mut self, button: Button) -> Result<(), CoreError> {
        self.release_left()?;
        self.enigo
            .button(button, Direction::Press)
            .map_err(|err| CoreError::Input(err.to_string()))?;
        thread::sleep(Duration::from_millis(35));
        self.enigo
            .button(button, Direction::Release)
            .map_err(|err| CoreError::Input(err.to_string()))
    }

    fn double_click(&mut self) -> Result<(), CoreError> {
        self.click(Button::Left)?;
        thread::sleep(Duration::from_millis(45));
        self.click(Button::Left)
    }

    fn command(&mut self, command: &CommandSpec) -> Result<(), CoreError> {
        match command {
            CommandSpec::None | CommandSpec::Cursor | CommandSpec::Drag => Ok(()),
            CommandSpec::LeftClick => self.click(Button::Left),
            CommandSpec::RightClick => self.click(Button::Right),
            CommandSpec::DoubleClick => self.double_click(),
            CommandSpec::Scroll { notches } => self
                .enigo
                .scroll(*notches, Axis::Vertical)
                .map_err(|err| CoreError::Input(err.to_string())),
            CommandSpec::VolumeUp => platform::volume_up(self),
            CommandSpec::VolumeDown => platform::volume_down(self),
            CommandSpec::Mute => platform::volume_mute(self),
            CommandSpec::MediaPlayPause => platform::media_play_pause(self),
            CommandSpec::MediaNext => platform::media_next(self),
            CommandSpec::MediaPrevious => platform::media_previous(self),
            CommandSpec::KeyChord { keys } => self.chord(keys),
            CommandSpec::TypeText { text } => self
                .enigo
                .text(text)
                .map_err(|err| CoreError::Input(err.to_string())),
            CommandSpec::ZoomIn => self.chord_scroll(&["ctrl"], 2),
            CommandSpec::ZoomOut => self.chord_scroll(&["ctrl"], -2),
            CommandSpec::MinimizeWindow => platform::minimize_foreground(self),
            CommandSpec::MaximizeWindow => platform::maximize_foreground(self),
            CommandSpec::RestoreWindow => platform::restore_foreground(self),
            CommandSpec::NextWindow => platform::next_window(self),
            CommandSpec::PreviousWindow => platform::previous_window(self),
            CommandSpec::VirtualDesktopLeft => platform::desktop_left(self),
            CommandSpec::VirtualDesktopRight => platform::desktop_right(self),
            CommandSpec::OpenApp { id } => platform::open_app(id),
            CommandSpec::CloseApp { id } => platform::close_app(id),
            CommandSpec::NewNote { text } => new_note(self, text),
            CommandSpec::StartTimer { seconds } => {
                start_timer(*seconds);
                Ok(())
            }
            CommandSpec::SetAlarm { hour, minute } => {
                set_alarm(*hour, *minute);
                Ok(())
            }
            CommandSpec::SpeakWeather => {
                speak_weather();
                Ok(())
            }
            CommandSpec::PageScroll { down, pages } => page_scroll(self, *down, *pages),
            CommandSpec::WebSearch { query } => web_search(self, query),
            CommandSpec::Retell => {
                retell();
                Ok(())
            }
            CommandSpec::Macro { steps } => self.run_macro(steps),
        }
    }

    fn run_macro(&mut self, steps: &[MacroStep]) -> Result<(), CoreError> {
        for step in steps {
            match step {
                MacroStep::Delay { ms } => {
                    thread::sleep(Duration::from_millis((*ms).min(1_500)));
                }
                MacroStep::Do { command } => {
                    if matches!(command, CommandSpec::Macro { .. }) {
                        return Err(CoreError::Rejected("nested macros are not allowed".into()));
                    }
                    self.command(command)?;
                }
            }
        }
        Ok(())
    }

    fn chord_scroll(&mut self, modifiers: &[&str], notches: i32) -> Result<(), CoreError> {
        let keys: Vec<String> = modifiers.iter().map(|key| (*key).to_string()).collect();
        let keys = mac_shortcut(&keys);
        self.press_keys(&keys)?;
        let scrolled = self
            .enigo
            .scroll(notches, Axis::Vertical)
            .map_err(|err| CoreError::Input(err.to_string()));
        self.release_keys();
        scrolled
    }

    fn chord(&mut self, keys: &[String]) -> Result<(), CoreError> {
        let keys = mac_shortcut(keys);
        self.press_keys(&keys)?;
        thread::sleep(Duration::from_millis(35));
        self.release_keys();
        Ok(())
    }

    fn press_keys(&mut self, keys: &[String]) -> Result<(), CoreError> {
        for name in keys {
            let key = map_key(name)?;
            if let Err(err) = self.enigo.key(key, Direction::Press) {
                self.release_keys();
                return Err(CoreError::Input(err.to_string()));
            }
            self.held_keys.push(key);
        }
        Ok(())
    }

    fn release_keys(&mut self) {
        while let Some(key) = self.held_keys.pop() {
            let _ = self.enigo.key(key, Direction::Release);
        }
    }

    #[cfg_attr(windows, allow(dead_code))]
    fn tap_key(&mut self, key: Key) -> Result<(), CoreError> {
        self.enigo
            .key(key, Direction::Click)
            .map_err(|err| CoreError::Input(err.to_string()))
    }

    fn release_all(&mut self) {
        let _ = self.release_left();
        self.release_keys();
    }

    fn tick(&mut self) {
        if self.left_down
            && self
                .left_since
                .is_some_and(|started| started.elapsed() > Duration::from_secs(8))
        {
            tracing::warn!("released a mouse button held longer than 8 seconds");
            let _ = self.release_left();
        }
    }
}

impl Drop for InputExecutor {
    fn drop(&mut self) {
        self.release_all();
    }
}

/// On macOS, Ctrl+letter is Command. Ctrl+arrow stays Control (desktops).
fn mac_shortcut(keys: &[String]) -> Vec<String> {
    #[cfg(windows)]
    {
        keys.to_vec()
    }
    #[cfg(not(windows))]
    {
        let arrows = keys
            .iter()
            .any(|key| matches!(key.as_str(), "left" | "right" | "up" | "down"));
        if arrows {
            return keys.to_vec();
        }
        keys.iter()
            .map(|key| {
                if key == "ctrl" {
                    "cmd".to_string()
                } else {
                    key.clone()
                }
            })
            .collect()
    }
}

fn map_key(name: &str) -> Result<Key, CoreError> {
    let key = match name {
        "ctrl" => Key::Control,
        "shift" => Key::Shift,
        "alt" => Key::Alt,
        "win" | "cmd" | "meta" => Key::Meta,
        "enter" => Key::Return,
        "escape" => Key::Escape,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "f5" => Key::F5,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "backspace" => Key::Backspace,
        other if other.len() == 1 => letter_key(other.chars().next().unwrap_or(' '))?,
        other => return Err(CoreError::Rejected(format!("key is not allowed: {other}"))),
    };
    Ok(key)
}

/// Prefer virtual letter keys on Windows so Ctrl/Alt chords work.
/// `Key::Unicode` only types text and breaks shortcuts like Ctrl+C.
fn letter_key(ch: char) -> Result<Key, CoreError> {
    let lower = ch.to_ascii_lowercase();
    if !lower.is_ascii_lowercase() {
        return Err(CoreError::Rejected(format!("key is not allowed: {ch}")));
    }
    #[cfg(windows)]
    {
        let key = match lower {
            'a' => Key::A,
            'b' => Key::B,
            'c' => Key::C,
            'd' => Key::D,
            'e' => Key::E,
            'f' => Key::F,
            'g' => Key::G,
            'h' => Key::H,
            'i' => Key::I,
            'j' => Key::J,
            'k' => Key::K,
            'l' => Key::L,
            'm' => Key::M,
            'n' => Key::N,
            'o' => Key::O,
            'p' => Key::P,
            'q' => Key::Q,
            'r' => Key::R,
            's' => Key::S,
            't' => Key::T,
            'u' => Key::U,
            'v' => Key::V,
            'w' => Key::W,
            'x' => Key::X,
            'y' => Key::Y,
            'z' => Key::Z,
            _ => unreachable!(),
        };
        return Ok(key);
    }
    #[cfg(not(windows))]
    {
        Ok(Key::Unicode(lower))
    }
}

mod platform {
    use super::*;

    #[cfg(windows)]
    pub fn volume_up(_: &mut InputExecutor) -> Result<(), CoreError> {
        tap_virtual_key(0xAF)
    }
    #[cfg(windows)]
    pub fn volume_down(_: &mut InputExecutor) -> Result<(), CoreError> {
        tap_virtual_key(0xAE)
    }
    #[cfg(windows)]
    pub fn volume_mute(_: &mut InputExecutor) -> Result<(), CoreError> {
        tap_virtual_key(0xAD)
    }
    #[cfg(windows)]
    pub fn media_play_pause(_: &mut InputExecutor) -> Result<(), CoreError> {
        tap_virtual_key(0xB3)
    }
    #[cfg(windows)]
    pub fn media_next(_: &mut InputExecutor) -> Result<(), CoreError> {
        tap_virtual_key(0xB0)
    }
    #[cfg(windows)]
    pub fn media_previous(_: &mut InputExecutor) -> Result<(), CoreError> {
        tap_virtual_key(0xB1)
    }
    #[cfg(windows)]
    pub fn minimize_foreground(_: &mut InputExecutor) -> Result<(), CoreError> {
        show_foreground(windows::Win32::UI::WindowsAndMessaging::SW_MINIMIZE.0)
    }
    #[cfg(windows)]
    pub fn maximize_foreground(_: &mut InputExecutor) -> Result<(), CoreError> {
        show_foreground(windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE.0)
    }
    #[cfg(windows)]
    pub fn restore_foreground(_: &mut InputExecutor) -> Result<(), CoreError> {
        show_foreground(windows::Win32::UI::WindowsAndMessaging::SW_RESTORE.0)
    }
    #[cfg(windows)]
    pub fn next_window(executor: &mut InputExecutor) -> Result<(), CoreError> {
        // Hold Alt a beat so Windows shows the switcher instead of a no-op.
        executor.press_keys(&["alt".into()])?;
        thread::sleep(Duration::from_millis(40));
        executor.press_keys(&["tab".into()])?;
        thread::sleep(Duration::from_millis(50));
        executor.release_keys();
        Ok(())
    }
    #[cfg(windows)]
    pub fn previous_window(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.press_keys(&["alt".into(), "shift".into()])?;
        thread::sleep(Duration::from_millis(40));
        executor.press_keys(&["tab".into()])?;
        thread::sleep(Duration::from_millis(50));
        executor.release_keys();
        Ok(())
    }
    #[cfg(windows)]
    pub fn desktop_left(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.chord(&["win".into(), "ctrl".into(), "left".into()])
    }
    #[cfg(windows)]
    pub fn desktop_right(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.chord(&["win".into(), "ctrl".into(), "right".into()])
    }
    #[cfg(windows)]
    pub fn open_app(id: &str) -> Result<(), CoreError> {
        let target = windows_app_target(id)?;
        use std::os::windows::process::CommandExt;
        let status = std::process::Command::new("cmd")
            .args(["/C", "start", "", target])
            .creation_flags(0x0800_0000)
            .status()
            .map_err(|err| CoreError::Input(err.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(CoreError::Input(format!("failed to open app: {id}")))
        }
    }
    #[cfg(windows)]
    pub fn close_app(id: &str) -> Result<(), CoreError> {
        let image = windows_app_image(id)?;
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/IM", image, "/F"])
            .creation_flags(0x0800_0000)
            .status();
        Ok(())
    }
    #[cfg(windows)]
    fn windows_app_target(id: &str) -> Result<&'static str, CoreError> {
        Ok(match id {
            "safari" | "browser" => "https://",
            "chrome" => "chrome",
            "firefox" => "firefox",
            "finder" => "explorer",
            "notes" => "notepad",
            "calculator" => "calc",
            "terminal" => "wt",
            "spotify" => "spotify:",
            "telegram" => "telegram",
            "vscode" => "code",
            "mail" => "mailto:",
            "music" => "mswindowsmusic:",
            "messages" => "ms-chat:",
            "slack" => "slack",
            "discord" => "discord",
            _ => return Err(CoreError::Input(format!("app is not allowed: {id}"))),
        })
    }
    #[cfg(windows)]
    fn windows_app_image(id: &str) -> Result<&'static str, CoreError> {
        Ok(match id {
            "chrome" | "browser" => "chrome.exe",
            "firefox" => "firefox.exe",
            "notes" => "notepad.exe",
            "calculator" => "CalculatorApp.exe",
            "terminal" => "WindowsTerminal.exe",
            "spotify" => "Spotify.exe",
            "telegram" => "Telegram.exe",
            "vscode" => "Code.exe",
            "slack" => "slack.exe",
            "discord" => "Discord.exe",
            "safari" | "finder" | "mail" | "music" | "messages" => {
                return Err(CoreError::Input(format!("close is not supported for {id} on Windows")))
            }
            _ => return Err(CoreError::Input(format!("app is not allowed: {id}"))),
        })
    }

    #[cfg(not(windows))]
    pub fn volume_up(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.tap_key(Key::VolumeUp)
    }
    #[cfg(not(windows))]
    pub fn volume_down(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.tap_key(Key::VolumeDown)
    }
    #[cfg(not(windows))]
    pub fn volume_mute(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.tap_key(Key::VolumeMute)
    }
    #[cfg(not(windows))]
    pub fn media_play_pause(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.tap_key(Key::MediaPlayPause)
    }
    #[cfg(not(windows))]
    pub fn media_next(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.tap_key(Key::MediaNextTrack)
    }
    #[cfg(not(windows))]
    pub fn media_previous(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.tap_key(Key::MediaPrevTrack)
    }
    #[cfg(not(windows))]
    pub fn minimize_foreground(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.chord(&["cmd".into(), "m".into()])
    }
    #[cfg(not(windows))]
    pub fn maximize_foreground(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.chord(&["ctrl".into(), "cmd".into(), "f".into()])
    }
    #[cfg(not(windows))]
    pub fn restore_foreground(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.chord(&["ctrl".into(), "cmd".into(), "f".into()])
    }
    #[cfg(not(windows))]
    pub fn next_window(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.chord(&["cmd".into(), "tab".into()])
    }
    #[cfg(not(windows))]
    pub fn previous_window(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.chord(&["cmd".into(), "shift".into(), "tab".into()])
    }
    #[cfg(not(windows))]
    pub fn desktop_left(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.chord(&["ctrl".into(), "left".into()])
    }
    #[cfg(not(windows))]
    pub fn desktop_right(executor: &mut InputExecutor) -> Result<(), CoreError> {
        executor.chord(&["ctrl".into(), "right".into()])
    }
    #[cfg(not(windows))]
    pub fn open_app(id: &str) -> Result<(), CoreError> {
        let name = macos_app_name(id)?;
        let status = if id == "browser" {
            std::process::Command::new("open")
                .args(["https://"])
                .status()
        } else {
            std::process::Command::new("open")
                .args(["-a", name])
                .status()
        }
        .map_err(|err| CoreError::Input(err.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(CoreError::Input(format!("failed to open {name}")))
        }
    }
    #[cfg(not(windows))]
    pub fn close_app(id: &str) -> Result<(), CoreError> {
        let name = macos_app_name(id)?;
        if id == "finder" {
            return Ok(());
        }
        let script = format!("tell application \"{name}\" to quit");
        let status = std::process::Command::new("osascript")
            .args(["-e", &script])
            .status()
            .map_err(|err| CoreError::Input(err.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(CoreError::Input(format!("failed to close {name}")))
        }
    }
    #[cfg(not(windows))]
    fn macos_app_name(id: &str) -> Result<&'static str, CoreError> {
        Ok(match id {
            "safari" | "browser" => "Safari",
            "chrome" => "Google Chrome",
            "firefox" => "Firefox",
            "finder" => "Finder",
            "notes" => "Notes",
            "calculator" => "Calculator",
            "terminal" => "Terminal",
            "spotify" => "Spotify",
            "telegram" => "Telegram",
            "vscode" => "Visual Studio Code",
            "mail" => "Mail",
            "music" => "Music",
            "messages" => "Messages",
            "slack" => "Slack",
            "discord" => "Discord",
            _ => return Err(CoreError::Input(format!("app is not allowed: {id}"))),
        })
    }

    #[cfg(windows)]
    fn tap_virtual_key(code: u8) -> Result<(), CoreError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
            KEYEVENTF_KEYUP, VIRTUAL_KEY,
        };
        // Media / volume keys need the extended flag; keybd_event alone is unreliable.
        let press = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(u16::from(code)),
                    wScan: 0,
                    dwFlags: KEYEVENTF_EXTENDEDKEY,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let release = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(u16::from(code)),
                    wScan: 0,
                    dwFlags: KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let sent = unsafe { SendInput(&[press, release], size_of::<INPUT>() as i32) };
        if sent == 0 {
            return Err(CoreError::Input("SendInput volume/media failed".into()));
        }
        Ok(())
    }

    #[cfg(windows)]
    fn show_foreground(command: i32) -> Result<(), CoreError> {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, ShowWindow, SHOW_WINDOW_CMD,
        };
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return Err(CoreError::Input("no foreground window".into()));
        }
        unsafe {
            let _ = ShowWindow(hwnd, SHOW_WINDOW_CMD(command));
        }
        Ok(())
    }
}

fn new_note(executor: &mut InputExecutor, text: &str) -> Result<(), CoreError> {
    platform::open_app("notes")?;
    thread::sleep(Duration::from_millis(700));
    #[cfg(not(windows))]
    executor.chord(&["cmd".into(), "n".into()])?;
    let text = text.trim();
    if !text.is_empty() {
        thread::sleep(Duration::from_millis(200));
        executor
            .enigo
            .text(text)
            .map_err(|err| CoreError::Input(err.to_string()))?;
    }
    Ok(())
}

fn page_scroll(executor: &mut InputExecutor, down: bool, pages: u8) -> Result<(), CoreError> {
    hide_self_if_front();
    let notches: i32 = if down { 18 } else { -18 };
    let bursts = i32::from(pages.clamp(1, 4)) * 4;
    for index in 0..bursts {
        executor
            .enigo
            .scroll(notches, Axis::Vertical)
            .map_err(|err| CoreError::Input(err.to_string()))?;
        if index + 1 < bursts {
            thread::sleep(Duration::from_millis(16));
        }
    }
    Ok(())
}

fn web_search(executor: &mut InputExecutor, query: &str) -> Result<(), CoreError> {
    let spoken = spoken_text(query);
    let query = if spoken.is_empty() {
        hide_self_if_front();
        selected_text(executor).unwrap_or_default()
    } else {
        spoken
    };
    if !query.is_empty() {
        remember_query(&query);
    }
    let url = if query.is_empty() {
        "https://www.google.com/".to_string()
    } else {
        format!(
            "https://www.google.com/search?q={}",
            percent_encode(&query)
        )
    };
    open_google(&url)
}

fn remember_query(query: &str) {
    if let Ok(mut slot) = LAST_SEARCH.lock() {
        *slot = query.chars().take(120).collect();
    }
}

fn last_query() -> String {
    LAST_SEARCH.lock().map(|slot| slot.clone()).unwrap_or_default()
}

fn retell() {
    thread::spawn(|| {
        let query = last_query();
        if query.is_empty() {
            say("Сначала скажите, что загуглить");
            return;
        }
        match fetch_summary(&query) {
            Some(text) => {
                notify(&text);
                say_long(&text);
            }
            None => say("Не нашёл, что рассказать"),
        }
    });
}

fn fetch_summary(query: &str) -> Option<String> {
    let search = format!(
        "https://ru.wikipedia.org/w/api.php?action=opensearch&search={}&limit=1&namespace=0&format=json",
        percent_encode(query)
    );
    let found = curl_bytes(&search)?;
    let value: serde_json::Value = serde_json::from_slice(&found).ok()?;
    let title = value
        .get(1)?
        .as_array()?
        .first()?
        .as_str()?
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let summary = format!(
        "https://ru.wikipedia.org/api/rest_v1/page/summary/{}",
        percent_encode(&title.replace(' ', "_"))
    );
    let page = curl_bytes(&summary)?;
    let page: serde_json::Value = serde_json::from_slice(&page).ok()?;
    let extract = page.get("extract")?.as_str()?.trim();
    if extract.chars().count() < 20 {
        return None;
    }
    Some(shorten_summary(extract))
}

fn shorten_summary(text: &str) -> String {
    let mut end = text.chars().count();
    if end > 420 {
        end = 420;
        let prefix: String = text.chars().take(end).collect();
        if let Some(cut) = prefix.rfind(['.', '!', '?']) {
            if cut > 80 {
                return prefix[..=cut].trim().to_string();
            }
        }
        return prefix.trim().to_string();
    }
    text.to_string()
}

fn curl_bytes(url: &str) -> Option<Vec<u8>> {
    let output = std::process::Command::new("curl")
        .args([
            "-fsS",
            "--max-time",
            "8",
            "-A",
            "AirControl/1.0 (voice summary)",
            url,
        ])
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

static LAST_SEARCH: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn hide_self_if_front() {
    if foreground_process().0 != std::process::id() {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"System Events\" to set visible of first process whose unix id is {} to false",
            std::process::id()
        );
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .status();
        thread::sleep(Duration::from_millis(180));
    }
}

fn selected_text(executor: &mut InputExecutor) -> Option<String> {
    let previous = read_clipboard().unwrap_or_default();
    executor.chord(&["ctrl".into(), "c".into()]).ok()?;
    thread::sleep(Duration::from_millis(140));
    let copied = read_clipboard().unwrap_or_default();
    let _ = write_clipboard(&previous);
    if copied.trim().is_empty() || copied == previous {
        return None;
    }
    let clean = spoken_text(&copied);
    let clean: String = clean.chars().take(120).collect();
    (!clean.is_empty()).then_some(clean)
}

fn read_clipboard() -> Option<String> {
    #[cfg(windows)]
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-Clipboard -Raw"])
        .output()
        .ok()?;
    #[cfg(not(windows))]
    let output = std::process::Command::new("pbpaste").output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.chars().take(2_000).collect())
}

fn write_clipboard(text: &str) -> Result<(), CoreError> {
    use std::io::Write;
    #[cfg(windows)]
    {
        let _ = text;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|err| CoreError::Input(err.to_string()))?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|err| CoreError::Input(err.to_string()))?;
        }
        let _ = child.wait();
        Ok(())
    }
}

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(*byte as char),
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn open_google(url: &str) -> Result<(), CoreError> {
    if !url.starts_with("https://www.google.com/") {
        return Err(CoreError::Rejected("search url is not allowed".into()));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let status = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .creation_flags(0x0800_0000)
            .status()
            .map_err(|err| CoreError::Input(err.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(CoreError::Input("failed to open the browser".into()))
        }
    }
    #[cfg(not(windows))]
    {
        let status = std::process::Command::new("open")
            .arg(url)
            .status()
            .map_err(|err| CoreError::Input(err.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(CoreError::Input("failed to open the browser".into()))
        }
    }
}

fn start_timer(seconds: u32) {
    let seconds = seconds.clamp(5, 12 * 3600);
    let label = timer_spoken(seconds);
    thread::spawn(move || {
        say(&format!("Таймер на {label}"));
        thread::sleep(Duration::from_secs(u64::from(seconds)));
        notify("Таймер");
        say("Таймер");
        chime();
    });
}

fn set_alarm(hour: Option<u8>, minute: u8) {
    let Some(hour) = hour else {
        thread::spawn(|| {
            open_clock();
            say("Открываю будильник");
        });
        return;
    };
    let hour = hour.min(23);
    let minute = minute.min(59);
    let wait = seconds_until(hour, minute);
    let phrase = if minute == 0 {
        format!("Будильник на {hour}")
    } else {
        format!("Будильник на {hour} {minute}")
    };
    thread::spawn(move || {
        say(&phrase);
        thread::sleep(Duration::from_secs(wait));
        notify("Будильник");
        say("Будильник");
        chime();
    });
}

fn speak_weather() {
    thread::spawn(|| {
        let line = fetch_weather().unwrap_or_else(|| "Не удалось узнать погоду".into());
        notify(&line);
        say(&line);
    });
}

fn timer_spoken(seconds: u32) -> String {
    if seconds >= 3600 && seconds % 3600 == 0 {
        format!("{} час", seconds / 3600)
    } else if seconds >= 60 && seconds % 60 == 0 {
        format!("{} минут", seconds / 60)
    } else {
        format!("{seconds} секунд")
    }
}

fn seconds_until(hour: u8, minute: u8) -> u64 {
    let Some((now_h, now_m, now_s)) = local_hms() else {
        return 60;
    };
    let now = u64::from(now_h) * 3600 + u64::from(now_m) * 60 + u64::from(now_s);
    let target = u64::from(hour) * 3600 + u64::from(minute) * 60;
    let wait = if target > now {
        target - now
    } else {
        24 * 3600 - (now - target)
    };
    wait.max(15)
}

fn local_hms() -> Option<(u32, u32, u32)> {
    #[cfg(windows)]
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-Date -Format HH:mm:ss"])
        .output()
        .ok()?;
    #[cfg(not(windows))]
    let output = std::process::Command::new("date")
        .arg("+%H:%M:%S")
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let mut parts = text.trim().split(':');
    let hour = parts.next()?.parse().ok()?;
    let minute = parts.next()?.parse().ok()?;
    let second = parts.next()?.parse().ok()?;
    (hour < 24 && minute < 60 && second < 60).then_some((hour, minute, second))
}

fn fetch_weather() -> Option<String> {
    let output = std::process::Command::new("curl")
        .args([
            "-fsS",
            "--max-time",
            "8",
            "-A",
            "AirControl",
            "https://wttr.in/?format=%l:+%C,+%t&lang=ru",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    if raw.contains('<') || raw.chars().count() > 180 {
        return None;
    }
    let clean = spoken_text(&raw);
    (clean.chars().count() >= 3).then_some(clean)
}

fn spoken_text(text: &str) -> String {
    spoken_limit(text, 180)
}

fn spoken_limit(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|ch| {
            ch.is_alphanumeric()
                || ch.is_whitespace()
                || matches!(*ch, ',' | '.' | '+' | '-' | '°' | ':' | '%')
        })
        .take(limit)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn say(text: &str) {
    say_raw(&spoken_text(text));
}

pub fn speak_reply(text: &str) {
    let clean: String = text.chars().filter(|ch| !ch.is_control()).take(320).collect();
    say_raw(clean.trim());
}

fn say_long(text: &str) {
    say_raw(&spoken_limit(text, 480));
}

fn say_raw(text: &str) {
    if text.is_empty() {
        return;
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).Speak($env:AIR_SAY)",
            ])
            .env("AIR_SAY", &text)
            .status();
    }
    #[cfg(not(windows))]
    {
        for voice in ["Milena", "Katya", "Yuri"] {
            let ok = std::process::Command::new("say")
                .args(["-v", voice, &text])
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            if ok {
                return;
            }
        }
        let _ = std::process::Command::new("say").arg(&text).status();
    }
}

fn notify(message: &str) {
    let message = spoken_text(message);
    if message.is_empty() {
        return;
    }
    #[cfg(not(windows))]
    {
        let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("display notification \"{escaped}\" with title \"AIR CONTROL\"");
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .status();
    }
    #[cfg(windows)]
    {
        let _ = message;
    }
}

fn chime() {
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .status();
    }
}

fn open_clock() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", "ms-clock:"])
            .creation_flags(0x0800_0000)
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("open")
            .args(["-a", "Clock"])
            .status();
    }
}

pub fn enable_dpi_awareness() {
    #[cfg(windows)]
    {
        use windows::Win32::UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }
}

pub fn virtual_screen() -> ScreenRect {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        };
        unsafe {
            let rect = ScreenRect {
                x: GetSystemMetrics(SM_XVIRTUALSCREEN),
                y: GetSystemMetrics(SM_YVIRTUALSCREEN),
                w: GetSystemMetrics(SM_CXVIRTUALSCREEN),
                h: GetSystemMetrics(SM_CYVIRTUALSCREEN),
            };
            if rect.w <= 0 || rect.h <= 0 {
                ScreenRect::fallback()
            } else {
                rect
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use core_graphics::display::{CGDisplay, CGMainDisplayID};
        let display = CGDisplay::new(unsafe { CGMainDisplayID() });
        let bounds = display.bounds();
        let w = bounds.size.width.round() as i32;
        let h = bounds.size.height.round() as i32;
        if w <= 0 || h <= 0 {
            ScreenRect::fallback()
        } else {
            ScreenRect {
                x: bounds.origin.x.round() as i32,
                y: bounds.origin.y.round() as i32,
                w,
                h,
            }
        }
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        ScreenRect::fallback()
    }
}

pub fn foreground_process() -> (u32, String) {
    #[cfg(windows)]
    {
        use windows::core::PWSTR;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return (0, String::new());
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return (0, String::new());
            }
            let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return (pid, String::new());
            };
            let mut buffer = [0u16; 512];
            let mut length = buffer.len() as u32;
            let name = if QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
            .is_ok()
            {
                String::from_utf16_lossy(&buffer[..length as usize])
            } else {
                String::new()
            };
            let _ = CloseHandle(handle);
            let file = name.rsplit(['\\', '/']).next().unwrap_or("").to_string();
            (pid, file)
        }
    }
    #[cfg(target_os = "macos")]
    {
        let pid = std::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get unix id of first process whose frontmost is true",
            ])
            .output()
            .ok()
            .and_then(|out| {
                String::from_utf8(out.stdout)
                    .ok()
                    .and_then(|text| text.trim().parse().ok())
            })
            .unwrap_or(0);
        let name = std::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get name of first process whose frontmost is true",
            ])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|text| text.trim().to_string())
            .unwrap_or_default();
        (pid, name)
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        (0, String::new())
    }
}

pub struct HotkeyHandle {
    #[cfg(windows)]
    thread_id: u32,
}

impl Drop for HotkeyHandle {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.thread_id != 0 {
            use windows::Win32::Foundation::{LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }
}

pub fn spawn_hotkey(
    on_toggle: impl Fn() + Send + 'static,
    on_error: impl Fn(String) + Send + 'static,
) -> HotkeyHandle {
    #[cfg(windows)]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        thread::Builder::new()
            .name("air-hotkey".into())
            .spawn(move || {
                use windows::Win32::System::Threading::GetCurrentThreadId;
                use windows::Win32::UI::Input::KeyboardAndMouse::{
                    RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_F12,
                };
                use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
                let thread_id = unsafe { GetCurrentThreadId() };
                let _ = tx.send(thread_id);
                let modifiers = MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT;
                let registered = unsafe { RegisterHotKey(None, 1, modifiers, VK_F12.0 as u32) };
                if let Err(err) = registered {
                    on_error(format!("глобальная пауза Ctrl+Shift+F12 недоступна: {err}"));
                    return;
                }
                let mut message = MSG::default();
                while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
                    if message.message == WM_HOTKEY {
                        on_toggle();
                    }
                }
                unsafe {
                    let _ = UnregisterHotKey(None, 1);
                }
            })
            .ok();
        HotkeyHandle {
            thread_id: rx.recv_timeout(Duration::from_secs(1)).unwrap_or(0),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = on_toggle;
        on_error(
            "Глобальная пауза Ctrl+Shift+F12 на этой ОС пока через кнопку в окне. На macOS разрешите Универсальный доступ для управления курсором."
                .into(),
        );
        HotkeyHandle {}
    }
}

#[cfg(target_os = "macos")]
fn prompt_accessibility_if_needed() {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    let trusted = unsafe { AXIsProcessTrusted() };
    if trusted {
        return;
    }
    // Open the Privacy pane so the user can enable AirControl. Recreating the
    // .app bundle used to drop this grant — keep the bundle path stable.
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}
