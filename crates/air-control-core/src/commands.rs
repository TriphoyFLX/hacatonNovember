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
    /// Typed into the focused field. Not a shell command and not a macro step.
    TypeText { text: String },
    /// Allowlisted sequence; steps themselves cannot nest another Macro.
    Macro { steps: Vec<MacroStep> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacroStep {
    Delay { ms: u64 },
    Do { command: CommandSpec },
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
    ("key:win+e", "Проводник"),
    ("key:alt+left", "Назад в папке"),
    ("key:alt+right", "Вперёд в папке"),
    ("key:alt+enter", "Свойства"),
    ("key:up", "Стрелка вверх"),
    ("key:down", "Стрелка вниз"),
    ("key:left", "Стрелка влево"),
    ("key:right", "Стрелка вправо"),
    ("key:ctrl+t", "Новая вкладка"),
    ("key:ctrl+w", "Закрыть вкладку"),
    ("key:ctrl+tab", "Следующая вкладка"),
    ("key:ctrl+shift+tab", "Предыдущая вкладка"),
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
    ("macro:key:ctrl+c", "Макрос: копировать"),
    ("macro:key:ctrl+v", "Макрос: вставить"),
    ("macro:key:win+e", "Макрос: открыть проводник"),
    ("macro:left_click;delay:120;key:ctrl+c", "Макрос: клик и копировать"),
];

pub fn command_from_id(id: &str) -> Result<CommandSpec, CoreError> {
    if let Some(rest) = id.strip_prefix("macro:") {
        return parse_macro(rest);
    }
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
        "scroll_up" => CommandSpec::Scroll { notches: 10 },
        "scroll_down" => CommandSpec::Scroll { notches: -10 },
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

fn parse_macro(raw: &str) -> Result<CommandSpec, CoreError> {
    let tokens: Vec<&str> = raw
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if tokens.is_empty() || tokens.len() > 8 {
        return Err(CoreError::Rejected("macro length is not allowed".into()));
    }
    let mut steps = Vec::new();
    let mut actions = 0u32;
    for token in tokens {
        if let Some(ms) = token.strip_prefix("delay:") {
            let ms: u64 = ms
                .parse()
                .map_err(|_| CoreError::Rejected("macro delay is invalid".into()))?;
            if ms > 1_500 {
                return Err(CoreError::Rejected("macro delay is too long".into()));
            }
            steps.push(MacroStep::Delay { ms });
            continue;
        }
        let command = command_from_id(token)?;
        if matches!(command, CommandSpec::Macro { .. }) {
            return Err(CoreError::Rejected("nested macros are not allowed".into()));
        }
        actions += 1;
        if actions > 6 {
            return Err(CoreError::Rejected("too many macro actions".into()));
        }
        steps.push(MacroStep::Do { command });
    }
    if actions == 0 {
        return Err(CoreError::Rejected("macro has no actions".into()));
    }
    Ok(CommandSpec::Macro { steps })
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
        CommandSpec::TypeText { text } => format!("Написать «{text}»"),
        CommandSpec::Macro { steps } => {
            let actions = steps
                .iter()
                .filter(|step| matches!(step, MacroStep::Do { .. }))
                .count();
            format!("Макрос ({actions})")
        }
        CommandSpec::KeyChord { keys } => match keys.as_slice() {
            [a, b] if a == "ctrl" && b == "c" => "Ctrl+C".into(),
            [a, b] if a == "ctrl" && b == "x" => "Вырезать".into(),
            [a, b] if a == "ctrl" && b == "a" => "Выделить всё".into(),
            [a, b] if a == "ctrl" && b == "s" => "Сохранить".into(),
            [a, b] if a == "ctrl" && b == "f" => "Найти".into(),
            [a] if a == "backspace" => "Стереть символ".into(),
            [a, b] if a == "ctrl" && b == "v" => "Ctrl+V".into(),
            [a, b] if a == "ctrl" && b == "z" => "Ctrl+Z".into(),
            [a, b] if a == "alt" && b == "tab" => "Alt+Tab".into(),
            [a, b] if a == "win" && b == "e" => "Проводник".into(),
            [a, b] if a == "alt" && b == "left" => "Назад в папке".into(),
            [a, b] if a == "alt" && b == "right" => "Вперёд в папке".into(),
            [a, b] if a == "alt" && b == "enter" => "Свойства".into(),
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
            | "meta"
            | "cmd"
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
            | "backspace"
    ) || (token.len() == 1
        && token
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase()))
}

pub fn gesture_title(id: &str) -> String {
    match id {
        "index_point" => "Указательный палец".into(),
        "index_tap" => "Тычок".into(),
        "pinch" => "Щипок".into(),
        "double_pinch" => "Двойной щипок".into(),
        "open_palm" => "Открытая ладонь".into(),
        "closed_fist" => "Кулак".into(),
        "squeeze" => "Сжатие пальцев".into(),
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

/// Shared how-to for every built-in motion. The action itself comes from the active profile.
pub fn gesture_how(id: &str) -> &'static str {
    match id {
        "index_point" => "Выпрямите указательный. Остальные пальцы подогните.",
        "index_tap" => "Указательный чуть согните и сразу разогните. Глубокий сгиб не нужен.",
        "pinch" => "Щипок больше не кликает. Для щелчка чуть согните указательный от прямого положения.",
        "double_pinch" => "Два коротких тычка указательным на том же месте.",
        "open_palm" => "Наведите палец на страницу, раскройте ладонь и проведите. Курсор остаётся на месте.",
        "closed_fist" => "Сожмите кулак и подержите.",
        "v_sign" => "Указательный и средний врозь, остальные в кулаке. Подержите.",
        "thumb_up" => "Кулак, большой палец вверх. Кончика указательного не касайтесь.",
        "thumb_down" => "Тот же кулак, большой палец вниз. Подержите.",
        "swipe_left" => "Открытой ладонью резко проведите влево.",
        "swipe_right" => "Открытой ладонью резко проведите вправо.",
        "swipe_up" => "Открытой ладонью резко проведите вверх.",
        "swipe_down" => "Открытой ладонью резко проведите вниз.",
        "two_hand_zoom_in" => "Две открытые ладони разведите в стороны.",
        "two_hand_zoom_out" => "Две открытые ладони сведите.",
        "two_hand_move_left" => "Обе открытые ладони ведите влево.",
        "two_hand_move_right" => "Обе открытые ладони ведите вправо.",
        _ => "Покажите движение так, как запомнили его.",
    }
}

pub fn is_pointer_command(command: &CommandSpec) -> bool {
    matches!(
        command,
        CommandSpec::Cursor | CommandSpec::LeftClick | CommandSpec::Drag | CommandSpec::DoubleClick
    )
}

/// Spoken forms the microphone is allowed to hear. Longest match wins.
const VOICE_PHRASES: &[(&str, &str)] = &[
    ("открой проводник", "key:win+e"),
    ("открой файлы", "key:win+e"),
    ("проводник", "key:win+e"),
    ("скопируй", "key:ctrl+c"),
    ("копировать", "key:ctrl+c"),
    ("вставь", "key:ctrl+v"),
    ("вставить", "key:ctrl+v"),
    ("отмени", "key:ctrl+z"),
    ("отменить", "key:ctrl+z"),
    ("без звука", "mute"),
    ("выключи звук", "mute"),
    ("следующий трек", "media_next"),
    ("следующая песня", "media_next"),
    ("предыдущий трек", "media_prev"),
    ("предыдущая песня", "media_prev"),
    ("следующее окно", "next_window"),
    ("предыдущее окно", "prev_window"),
    ("сверни окно", "minimize"),
    ("разверни окно", "maximize"),
    ("правый клик", "right_click"),
    ("двойной клик", "double_click"),
    ("прокрути вверх", "scroll_up"),
    ("прокрути вниз", "scroll_down"),
    ("прокрутка вверх", "scroll_up"),
    ("прокрутка вниз", "scroll_down"),
    ("листай вверх", "scroll_up"),
    ("листай вниз", "scroll_down"),
    ("страницу вверх", "scroll_up"),
    ("страницу вниз", "scroll_down"),
    ("предыдущая вкладка", "key:ctrl+shift+tab"),
    ("следующая вкладка", "key:ctrl+tab"),
    ("новая вкладка", "key:ctrl+t"),
    ("закрой вкладку", "key:ctrl+w"),
    ("выдели все", "key:ctrl+a"),
    ("вырежи", "key:ctrl+x"),
    ("сохрани", "key:ctrl+s"),
    ("найди", "key:ctrl+f"),
    ("шаг назад", "key:alt+left"),
    ("шаг вперед", "key:alt+right"),
    ("восстанови окно", "restore"),
    ("масштаб больше", "zoom_in"),
    ("масштаб меньше", "zoom_out"),
    ("удали символ", "key:backspace"),
    ("сотри", "key:backspace"),
    ("энтер", "key:enter"),
    ("ввод", "key:enter"),
    ("пробел", "key:space"),
    ("эскейп", "key:escape"),
    ("громче", "volume_up"),
    ("тише", "volume_down"),
    ("клик", "left_click"),
    ("нажми", "left_click"),
    ("пауза", "media_play"),
    ("плей", "media_play"),
    ("play", "media_play"),
];

/// Short list shown on the Voice page. Same phrases the recognizer accepts.
pub fn voice_phrase_catalog() -> &'static str {
    "напиши … — вставит текст в строку, где стоит курсор\nвниз, ниже, вверх, выше, прокрути вниз, листай вверх\nоткрой проводник\nгромче, тише, без звука\nскопируй, вставь, вырежи, отмени, сохрани, найди\nклик, правый клик, двойной клик, энтер, пробел\nследующее окно, предыдущее окно, новая вкладка\nсверни окно, разверни окно, восстанови окно\nплей, следующий трек, предыдущий трек"
}

fn dictate_text(text: &str) -> Option<String> {
    const PREFIXES: &[&str] = &["напиши", "напечатай", "введи", "впиши"];
    for prefix in PREFIXES {
        let Some(rest) = text.strip_prefix(prefix) else {
            continue;
        };
        let rest = rest
            .trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ':' | '-' | '—'));
        let typed: String = rest
            .chars()
            .filter(|ch| !ch.is_control())
            .take(80)
            .collect();
        let typed = typed.trim().to_string();
        if !typed.is_empty() {
            return Some(typed);
        }
        return None;
    }
    None
}

/// Map a spoken Russian phrase to an allowlisted command.
/// «напиши …» types the rest into the focused field. Otherwise the longest known phrase wins.
pub fn intent_from_phrase(phrase: &str) -> Option<CommandSpec> {
    let text = phrase.trim().to_lowercase().replace('ё', "е");
    if text.is_empty() || text == "[unk]" {
        return None;
    }
    if let Some(typed) = dictate_text(&text) {
        return Some(CommandSpec::TypeText { text: typed });
    }
    if let Some(id) = scroll_word(&text) {
        return command_from_id(id).ok();
    }
    let mut best: Option<&str> = None;
    let mut best_len = 0usize;
    for (spoken, id) in VOICE_PHRASES {
        if has_phrase(&text, spoken) && spoken.chars().count() > best_len {
            best = Some(*id);
            best_len = spoken.chars().count();
        }
    }
    command_from_id(best?).ok()
}

/// Direction words the small model actually emits, including inside a noisy line.
fn scroll_word(text: &str) -> Option<&'static str> {
    let mut down = false;
    let mut up = false;
    let mut previous = "";
    for raw in text.split_whitespace() {
        let word = raw.trim_matches(|ch: char| !ch.is_alphabetic());
        match word {
            "вниз" | "ниже" | "пониже" | "книзу" | "опусти" | "опустить" => down = true,
            "низ" if previous == "в" => down = true,
            "вверх" | "выше" | "наверх" | "кверху" | "подними" | "поднять" => up = true,
            "верх" if previous == "в" => up = true,
            _ => {}
        }
        previous = word;
    }
    match (down, up) {
        (true, false) => Some("scroll_down"),
        (false, true) => Some("scroll_up"),
        _ => None,
    }
}

fn has_phrase(text: &str, phrase: &str) -> bool {
    let padded = format!(" {text} ");
    padded.contains(&format!(" {phrase} "))
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
        assert!(matches!(
            command_from_id("macro:left_click;delay:120;key:ctrl+c").unwrap(),
            CommandSpec::Macro { .. }
        ));
        assert!(command_from_id("macro:shell:calc").is_err());
        for (id, label) in COMMAND_CHOICES {
            let command = command_from_id(id).unwrap();
            if id.starts_with("macro:") {
                assert!(command_label(&command).starts_with("Макрос"));
                continue;
            }
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

    #[test]
    fn every_builtin_gesture_has_a_howto() {
        for id in [
            "index_point",
            "pinch",
            "double_pinch",
            "v_sign",
            "thumb_up",
            "thumb_down",
            "swipe_left",
            "swipe_right",
            "swipe_up",
            "swipe_down",
            "open_palm",
            "closed_fist",
            "two_hand_zoom_in",
            "two_hand_zoom_out",
            "two_hand_move_left",
            "two_hand_move_right",
        ] {
            assert!(!gesture_how(id).is_empty(), "{id}");
            assert_ne!(gesture_title(id), id);
        }
    }

    #[test]
    fn voice_intent_maps_russian_phrases() {
        assert!(matches!(
            intent_from_phrase("открой проводник").unwrap(),
            CommandSpec::KeyChord { .. }
        ));
        assert!(matches!(
            intent_from_phrase("ну громче пожалуйста").unwrap(),
            CommandSpec::VolumeUp
        ));
        assert!(matches!(
            intent_from_phrase("правый клик").unwrap(),
            CommandSpec::RightClick
        ));
        assert!(matches!(
            intent_from_phrase("напиши привет мир").unwrap(),
            CommandSpec::TypeText { text } if text == "привет мир"
        ));
        assert!(intent_from_phrase("напиши клик сюда").is_some_and(|command| {
            matches!(command, CommandSpec::TypeText { text } if text == "клик сюда")
        }));
        assert!(matches!(
            intent_from_phrase("листай вниз").unwrap(),
            CommandSpec::Scroll { notches } if notches < 0
        ));
        assert!(matches!(
            intent_from_phrase("груди вниз вниз").unwrap(),
            CommandSpec::Scroll { notches } if notches < 0
        ));
        assert!(matches!(
            intent_from_phrase("ниже").unwrap(),
            CommandSpec::Scroll { notches } if notches < 0
        ));
        assert!(matches!(
            intent_from_phrase("вверх").unwrap(),
            CommandSpec::Scroll { notches } if notches > 0
        ));
        assert!(intent_from_phrase("напиши вниз").is_some_and(|command| {
            matches!(command, CommandSpec::TypeText { .. })
        }));
        assert!(intent_from_phrase("удали систему").is_none());
        assert!(intent_from_phrase("[unk]").is_none());
    }
}
