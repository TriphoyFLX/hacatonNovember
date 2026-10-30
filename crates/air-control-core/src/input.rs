//! The only place that injects mouse, keyboard, volume, and window commands.
//! Gesture code never calls these functions directly.

use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

use crate::commands::CommandSpec;
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
        .spawn(move || match WindowsExecutor::new() {
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

fn input_loop(mut executor: WindowsExecutor, rx: Receiver<InputMsg>) {
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

fn dispatch(executor: &mut WindowsExecutor, message: InputMsg) {
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

struct WindowsExecutor {
    enigo: Enigo,
    left_down: bool,
    left_since: Option<Instant>,
    held_keys: Vec<Key>,
}

impl WindowsExecutor {
    fn new() -> Result<Self, CoreError> {
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
            .button(button, Direction::Click)
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
            CommandSpec::VolumeUp => tap_virtual_key(0xAF),
            CommandSpec::VolumeDown => tap_virtual_key(0xAE),
            CommandSpec::Mute => tap_virtual_key(0xAD),
            CommandSpec::MediaPlayPause => tap_virtual_key(0xB3),
            CommandSpec::MediaNext => tap_virtual_key(0xB0),
            CommandSpec::MediaPrevious => tap_virtual_key(0xB1),
            CommandSpec::KeyChord { keys } => self.chord(keys),
            CommandSpec::ZoomIn => self.chord_scroll(&["ctrl"], 2),
            CommandSpec::ZoomOut => self.chord_scroll(&["ctrl"], -2),
            CommandSpec::MinimizeWindow => {
                show_foreground(windows::Win32::UI::WindowsAndMessaging::SW_MINIMIZE.0)
            }
            CommandSpec::MaximizeWindow => {
                show_foreground(windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE.0)
            }
            CommandSpec::RestoreWindow => {
                show_foreground(windows::Win32::UI::WindowsAndMessaging::SW_RESTORE.0)
            }
            CommandSpec::NextWindow => self.chord(&["alt".into(), "tab".into()]),
            CommandSpec::PreviousWindow => {
                self.chord(&["alt".into(), "shift".into(), "tab".into()])
            }
            CommandSpec::VirtualDesktopLeft => {
                self.chord(&["win".into(), "ctrl".into(), "left".into()])
            }
            CommandSpec::VirtualDesktopRight => {
                self.chord(&["win".into(), "ctrl".into(), "right".into()])
            }
        }
    }

    fn chord_scroll(&mut self, modifiers: &[&str], notches: i32) -> Result<(), CoreError> {
        let keys: Vec<String> = modifiers.iter().map(|key| (*key).to_string()).collect();
        self.press_keys(&keys)?;
        let scrolled = self
            .enigo
            .scroll(notches, Axis::Vertical)
            .map_err(|err| CoreError::Input(err.to_string()));
        self.release_keys();
        scrolled
    }

    fn chord(&mut self, keys: &[String]) -> Result<(), CoreError> {
        self.press_keys(keys)?;
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

impl Drop for WindowsExecutor {
    fn drop(&mut self) {
        self.release_all();
    }
}

fn map_key(name: &str) -> Result<Key, CoreError> {
    let key = match name {
        "ctrl" => Key::Control,
        "shift" => Key::Shift,
        "alt" => Key::Alt,
        "win" => Key::Meta,
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
        other if other.len() == 1 => {
            let ch = other.chars().next().unwrap_or(' ');
            if ch.is_ascii_lowercase() {
                Key::Unicode(ch)
            } else {
                return Err(CoreError::Rejected(format!("key is not allowed: {other}")));
            }
        }
        other => return Err(CoreError::Rejected(format!("key is not allowed: {other}"))),
    };
    Ok(key)
}

fn tap_virtual_key(code: u8) -> Result<(), CoreError> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    };
    unsafe {
        keybd_event(code, 0, KEYBD_EVENT_FLAGS(0), 0);
        keybd_event(code, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}

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

pub fn enable_dpi_awareness() {
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

pub fn virtual_screen() -> ScreenRect {
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

pub fn foreground_process() -> (u32, String) {
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

pub struct HotkeyHandle {
    thread_id: u32,
}

impl Drop for HotkeyHandle {
    fn drop(&mut self) {
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
