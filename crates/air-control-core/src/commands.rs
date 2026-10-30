use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Commands the dispatcher is allowed to perform.
/// There is no shell, file, or elevated-process variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandSpec {
    None,
    Cursor,
    LeftClick,
    RightClick,
    DoubleClick,
    Drag,
    Scroll { notches: i32 },
    VolumeUp,
    VolumeDown,
    Mute,
    KeyChord { keys: Vec<String> },
    MinimizeWindow,
    MaximizeWindow,
    RestoreWindow,
    NextWindow,
    PreviousWindow,
    MediaPlayPause,
    MediaNext,
    MediaPrevious,
    ZoomIn,
    ZoomOut,
    VirtualDesktopLeft,
    VirtualDesktopRight,
}

impl Default for CommandSpec {
    fn default() -> Self {
        Self::None
    }
}

pub const COMMAND_CHOICES: &[(&str, &str)] = &[
    ("none", "Нет"),
    ("cursor", "Курсор"),
    ("left_click", "Левый клик"),
    ("right_click", "Правый клик"),
    ("double_click", "Двойной клик"),
    ("drag", "Перетаскивание"),
    ("scroll_up", "Прокрутка вверх"),
    ("scroll_down", "Прокрутка вниз"),
    ("volume_up", "Громкость +"),
    ("volume_down", "Громкость −"),
    ("mute", "Без звука"),
    ("key:enter", "Enter"),
    ("key:escape", "Escape"),
    ("key:tab", "Tab"),
    ("key:space", "Space"),
    ("key:ctrl+c", "Ctrl+C"),
    ("key:ctrl+v", "Ctrl+V"),
    ("key:ctrl+z", "Ctrl+Z"),
    ("key:alt+tab", "Alt+Tab"),
    ("key:up", "Стрелка вверх"),
    ("key:down", "Стрелка вниз"),
    ("key:left", "Стрелка влево"),
    ("key:right", "Стрелка вправо"),
    ("key:ctrl+t", "Новая вкладка"),
    ("key:ctrl+w", "Закрыть вкладку"),
    ("key:ctrl+tab", "Следующая вкладка"),
    ("key:ctrl+shift+tab", "Предыдущая вкладка"),
    ("key:right", "Следующий слайд"),
    ("key:left", "Предыдущий слайд"),
    ("key:f5", "Запуск презентации"),
    ("media_play", "Play/Pause"),
    ("media_next", "Следующий трек"),
    ("media_prev", "Предыдущий трек"),
    ("minimize", "Свернуть окно"),
    ("maximize", "Развернуть окно"),
    ("restore", "Восстановить окно"),
    ("next_window", "Следующее окно"),
    ("zoom_in", "Масштаб +"),
    ("zoom_out", "Масштаб −"),
    ("desktop_left", "Рабочий стол влево"),
    ("desktop_right", "Рабочий стол вправо"),
];

pub fn command_from_id(id: &str) -> Result<CommandSpec, CoreError> {
    if let Some(rest) = id.strip_prefix("key:") {
        return Ok(CommandSpec::KeyChord {
            keys: parse_chord(rest)?,
        });
    }
    let command = match id {
        "none" => CommandSpec::None,
        "cursor" => CommandSpec::Cursor,
        "left_click" => CommandSpec::LeftClick,
        "right_click" => CommandSpec::RightClick,
        "double_click" => CommandSpec::DoubleClick,
        "drag" => CommandSpec::Drag,
        "scroll_up" => CommandSpec::Scroll { notches: 3 },
        "scroll_down" => CommandSpec::Scroll { notches: -3 },
        "volume_up" => CommandSpec::VolumeUp,
        "volume_down" => CommandSpec::VolumeDown,
        "mute" => CommandSpec::Mute,
        "media_play" => CommandSpec::MediaPlayPause,
        "media_next" => CommandSpec::MediaNext,
        "media_prev" => CommandSpec::MediaPrevious,
        "minimize" => CommandSpec::MinimizeWindow,
        "maximize" => CommandSpec::MaximizeWindow,
        "restore" => CommandSpec::RestoreWindow,
        "next_window" => CommandSpec::NextWindow,
        "prev_window" => CommandSpec::PreviousWindow,
        "zoom_in" => CommandSpec::ZoomIn,
        "zoom_out" => CommandSpec::ZoomOut,
        "desktop_left" => CommandSpec::VirtualDesktopLeft,
        "desktop_right" => CommandSpec::VirtualDesktopRight,
        _ => return Err(CoreError::Rejected(format!("command is not allowed: {id}"))),
    };
    Ok(command)
}

pub fn command_id_from_label(label: &str) -> Option<&'static str> {
    COMMAND_CHOICES
        .iter()
        .find(|(_, title)| *title == label)
        .map(|(id, _)| *id)
}

pub fn command_label(command: &CommandSpec) -> String {
    match command {
        CommandSpec::None => "Нет".into(),
        CommandSpec::Cursor => "Курсор".into(),
        CommandSpec::LeftClick => "Левый клик".into(),
        CommandSpec::RightClick => "Правый клик".into(),
        CommandSpec::DoubleClick => "Двойной клик".into(),
        CommandSpec::Drag => "Перетаскивание".into(),
        CommandSpec::Scroll { notches } if *notches > 0 => "Прокрутка вверх".into(),
        CommandSpec::Scroll { .. } => "Прокрутка вниз".into(),
        CommandSpec::VolumeUp => "Громкость +".into(),
        CommandSpec::VolumeDown => "Громкость −".into(),
        CommandSpec::Mute => "Без звука".into(),
        CommandSpec::MinimizeWindow => "Свернуть окно".into(),
        CommandSpec::MaximizeWindow => "Развернуть окно".into(),
        CommandSpec::RestoreWindow => "Восстановить окно".into(),
        CommandSpec::NextWindow => "Следующее окно".into(),
        CommandSpec::PreviousWindow => "Предыдущее окно".into(),
        CommandSpec::MediaPlayPause => "Play/Pause".into(),
        CommandSpec::MediaNext => "Следующий трек".into(),
        CommandSpec::MediaPrevious => "Предыдущий трек".into(),
        CommandSpec::ZoomIn => "Масштаб +".into(),
        CommandSpec::ZoomOut => "Масштаб −".into(),
        CommandSpec::VirtualDesktopLeft => "Рабочий стол влево".into(),
        CommandSpec::VirtualDesktopRight => "Рабочий стол вправо".into(),
        CommandSpec::KeyChord { keys } => match keys.as_slice() {
            [a, b] if a == "ctrl" && b == "c" => "Ctrl+C".into(),
            [a, b] if a == "ctrl" && b == "v" => "Ctrl+V".into(),
            [a, b] if a == "ctrl" && b == "z" => "Ctrl+Z".into(),
            [a, b] if a == "alt" && b == "tab" => "Alt+Tab".into(),
            [a, b] if a == "ctrl" && b == "t" => "Новая вкладка".into(),
            [a, b] if a == "ctrl" && b == "w" => "Закрыть вкладку".into(),
            [a, b] if a == "ctrl" && b == "tab" => "Следующая вкладка".into(),
            [a, b, c] if a == "ctrl" && b == "shift" && c == "tab" => "Предыдущая вкладка".into(),
            [a] if a == "enter" => "Enter".into(),
            [a] if a == "escape" => "Escape".into(),
            [a] if a == "tab" => "Tab".into(),
            [a] if a == "space" => "Space".into(),
            [a] if a == "up" => "Стрелка вверх".into(),
            [a] if a == "down" => "Стрелка вниз".into(),
            [a] if a == "left" => "Стрелка влево".into(),
            [a] if a == "right" => "Следующий слайд".into(),
            [a] if a == "f5" => "Запуск презентации".into(),
            other => other.join("+"),
        },
    }
}

pub fn parse_chord(raw: &str) -> Result<Vec<String>, CoreError> {
    let parts: Vec<&str> = raw
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() || parts.len() > 4 {
        return Err(CoreError::Rejected(
            "key chord length is not allowed".into(),
        ));
    }
    let mut keys = Vec::new();
    for part in parts {
        let token = part.to_ascii_lowercase();
        if !allowed_key(&token) {
            return Err(CoreError::Rejected(format!("key is not allowed: {token}")));
        }
        keys.push(token);
    }
    Ok(keys)
}

fn allowed_key(token: &str) -> bool {
    matches!(
        token,
        "ctrl"
            | "shift"
            | "alt"
            | "win"
            | "enter"
            | "escape"
            | "tab"
            | "space"
            | "up"
            | "down"
            | "left"
            | "right"
            | "f5"
            | "pageup"
            | "pagedown"
    ) || (token.len() == 1
        && token
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase()))
}

pub fn gesture_title(id: &str) -> String {
    match id {
        "index_point" => "Указательный палец".into(),
        "pinch" => "Щипок".into(),
        "double_pinch" => "Двойной щипок".into(),
        "open_palm" => "Открытая ладонь".into(),
        "closed_fist" => "Кулак".into(),
        "v_sign" => "Знак V".into(),
        "thumb_up" => "Большой палец вверх".into(),
        "thumb_down" => "Большой палец вниз".into(),
        "swipe_left" => "Свайп влево".into(),
        "swipe_right" => "Свайп вправо".into(),
        "swipe_up" => "Свайп вверх".into(),
        "swipe_down" => "Свайп вниз".into(),
        "two_hand_zoom_in" => "Масштаб двумя руками +".into(),
        "two_hand_zoom_out" => "Масштаб двумя руками −".into(),
        "two_hand_move_left" => "Две руки влево".into(),
        "two_hand_move_right" => "Две руки вправо".into(),
        other => other.to_string(),
    }
}

pub fn is_pointer_command(command: &CommandSpec) -> bool {
    matches!(
        command,
        CommandSpec::Cursor | CommandSpec::LeftClick | CommandSpec::Drag | CommandSpec::DoubleClick
    )
}

/// Safety decision shared by the engine and tests.
pub fn review(
    paused: bool,
    confidence: f32,
    min_confidence: f32,
    cooldown_elapsed: bool,
) -> Result<(), &'static str> {
    if paused {
        Err("paused")
    } else if !confidence.is_finite() || confidence < min_confidence {
        Err("low_confidence")
    } else if !cooldown_elapsed {
        Err("cooldown")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_accepts_shortcuts_and_rejects_shell() {
        assert!(matches!(
            command_from_id("key:ctrl+c").unwrap(),
            CommandSpec::KeyChord { .. }
        ));
        assert!(command_from_id("key:ctrl+cmd.exe").is_err());
        assert!(command_from_id("shell:calc").is_err());
        assert!(command_from_id("key:enter;calc").is_err());
        for (id, label) in COMMAND_CHOICES {
            let command = command_from_id(id).unwrap();
            if *id == "key:right" {
                assert_eq!(command_label(&command), "Следующий слайд");
            } else if *id == "key:left" {
                assert_eq!(command_label(&command), "Стрелка влево");
            } else {
                assert_eq!(command_label(&command), *label);
            }
            assert_eq!(command_id_from_label(label), Some(*id));
        }
    }

    #[test]
    fn pause_and_confidence_block_commands() {
        assert_eq!(review(true, 0.99, 0.7, true), Err("paused"));
        assert_eq!(review(false, 0.4, 0.7, true), Err("low_confidence"));
        assert_eq!(review(false, 0.9, 0.7, false), Err("cooldown"));
        assert!(review(false, 0.9, 0.7, true).is_ok());
    }
}
