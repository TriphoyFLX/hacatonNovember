use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::commands::command_from_id;
use crate::error::CoreError;
use crate::gestures::BUILTIN_GESTURES;
use crate::model::{
    CalibrationProfile, ContextRule, CustomGesture, GestureBinding, PointerTune, Profile,
    UsageStatistics, UserSettings,
};

pub fn app_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("AirControl")
}

pub fn log_dir() -> PathBuf {
    app_dir().join("logs")
}

pub fn load_all() -> (
    UserSettings,
    CalibrationProfile,
    Vec<Profile>,
    Vec<CustomGesture>,
    UsageStatistics,
) {
    let settings = read_or_default(app_dir().join("settings.json"));
    let calibration = read_or_default(app_dir().join("calibration.json"));
    let profiles = read_or_default::<Vec<Profile>>(app_dir().join("profiles.json"));
    let profiles = if profiles.is_empty() {
        default_profiles()
    } else {
        profiles
    };
    let custom = read_or_default(app_dir().join("gestures.json"));
    let stats = read_or_default(app_dir().join("statistics.json"));
    (settings, calibration, profiles, custom, stats)
}

pub fn save_settings(settings: &UserSettings) -> Result<(), CoreError> {
    write_json(&app_dir().join("settings.json"), settings)
}

pub fn save_calibration(calibration: &CalibrationProfile) -> Result<(), CoreError> {
    write_json(&app_dir().join("calibration.json"), calibration)
}

pub fn save_profiles(profiles: &[Profile]) -> Result<(), CoreError> {
    write_json(&app_dir().join("profiles.json"), &profiles)
}

pub fn save_gestures(gestures: &[CustomGesture]) -> Result<(), CoreError> {
    write_json(&app_dir().join("gestures.json"), &gestures)
}

pub fn save_stats(stats: &UsageStatistics) -> Result<(), CoreError> {
    write_json(&app_dir().join("statistics.json"), stats)
}

pub fn load_context_rules() -> Vec<ContextRule> {
    let path = app_dir().join("context_rules.json");
    let rules = read_or_default::<Vec<ContextRule>>(path);
    if rules.is_empty() {
        default_context_rules()
    } else {
        rules
    }
}

pub fn save_context_rules(rules: &[ContextRule]) -> Result<(), CoreError> {
    write_json(&app_dir().join("context_rules.json"), &rules)
}

pub fn default_context_rules() -> Vec<ContextRule> {
    vec![
        ContextRule {
            pattern: "explorer.exe".into(),
            profile_id: "explorer".into(),
            enabled: true,
        },
        ContextRule {
            pattern: "chrome.exe".into(),
            profile_id: "browser".into(),
            enabled: true,
        },
        ContextRule {
            pattern: "msedge.exe".into(),
            profile_id: "browser".into(),
            enabled: true,
        },
        ContextRule {
            pattern: "firefox.exe".into(),
            profile_id: "browser".into(),
            enabled: true,
        },
        ContextRule {
            pattern: "powerpnt.exe".into(),
            profile_id: "presentation".into(),
            enabled: true,
        },
        ContextRule {
            pattern: "spotify.exe".into(),
            profile_id: "media".into(),
            enabled: true,
        },
    ]
}

/// Prefer user context rules, then built-in executable map.
pub fn resolve_profile_id(executable: &str, rules: &[ContextRule]) -> String {
    let lower = executable.to_ascii_lowercase();
    for rule in rules {
        if !rule.enabled || rule.pattern.is_empty() {
            continue;
        }
        let pat = rule.pattern.to_ascii_lowercase();
        if lower.contains(&pat) || lower == pat {
            return rule.profile_id.clone();
        }
    }
    profile_for_executable(executable).to_string()
}

fn read_or_default<T: for<'de> serde::Deserialize<'de> + Default>(path: PathBuf) -> T {
    match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(path = %path.display(), error = %err, "kept previous file and loaded defaults");
                let backup = path.with_extension("json.bak");
                let _ = fs::copy(&path, backup);
                T::default()
            }
        },
        Err(_) => T::default(),
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| CoreError::Storage(err.to_string()))?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|err| CoreError::Storage(err.to_string()))?;
    if path.exists() {
        fs::remove_file(path).map_err(|err| CoreError::Storage(err.to_string()))?;
    }
    fs::rename(&temporary, path).map_err(|err| CoreError::Storage(err.to_string()))?;
    Ok(())
}

pub fn export_gestures(
    path: &Path,
    gestures: &[CustomGesture],
    profiles: &[Profile],
) -> Result<(), CoreError> {
    write_json(
        path,
        &serde_json::json!({ "version": 1, "gestures": gestures, "profiles": profiles }),
    )
}

pub fn import_gestures(
    path: &Path,
    gestures: &mut Vec<CustomGesture>,
    profiles: &mut [Profile],
) -> Result<usize, CoreError> {
    let bytes = fs::read(path).map_err(|err| CoreError::Storage(err.to_string()))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    if value.get("version").and_then(|v| v.as_u64()) != Some(1) {
        return Err(CoreError::Storage(
            "gesture file version is not supported".into(),
        ));
    }
    let incoming: Vec<CustomGesture> = serde_json::from_value(
        value
            .get("gestures")
            .cloned()
            .unwrap_or(serde_json::Value::Array(Vec::new())),
    )?;
    let mut added = 0;
    for gesture in incoming {
        validate_custom(&gesture)?;
        if let Some(existing) = gestures.iter_mut().find(|item| item.name == gesture.name) {
            *existing = gesture;
        } else {
            gestures.push(gesture);
        }
        added += 1;
    }
    if let Some(saved_profiles) = value.get("profiles") {
        let saved: Vec<Profile> = serde_json::from_value(saved_profiles.clone())?;
        for saved in saved {
            if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == saved.id) {
                for (name, binding) in saved.mappings {
                    if profile.binding_mut(&name).is_some() {
                        if let Some(slot) = profile.binding_mut(&name) {
                            slot.command = binding.command;
                        }
                    }
                }
            }
        }
    }
    Ok(added)
}

pub fn validate_custom(gesture: &CustomGesture) -> Result<(), CoreError> {
    if !valid_name(&gesture.name) || BUILTIN_GESTURES.contains(&gesture.name.as_str()) {
        return Err(CoreError::Rejected(
            "gesture name is reserved or invalid".into(),
        ));
    }
    if !(4..=24).contains(&gesture.frames.len()) {
        return Err(CoreError::Rejected(
            "gesture template length is invalid".into(),
        ));
    }
    if gesture.templates.len() > 4 {
        return Err(CoreError::Rejected(
            "too many gesture templates".into(),
        ));
    }
    if !gesture.threshold.is_finite() || !(0.05..=2.0).contains(&gesture.threshold) {
        return Err(CoreError::Rejected("gesture threshold is invalid".into()));
    }
    for frame in &gesture.frames {
        if frame.len() != 63 || frame.iter().any(|value| !value.is_finite()) {
            return Err(CoreError::Rejected("gesture frame is invalid".into()));
        }
    }
    for template in &gesture.templates {
        if !(4..=24).contains(&template.len()) {
            return Err(CoreError::Rejected(
                "gesture template length is invalid".into(),
            ));
        }
        for frame in template {
            if frame.len() != 63 || frame.iter().any(|value| !value.is_finite()) {
                return Err(CoreError::Rejected("gesture frame is invalid".into()));
            }
        }
    }
    Ok(())
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == ' ' || ch == '_' || ch == '-')
}

pub fn default_profiles() -> Vec<Profile> {
    vec![
        profile(
            "desktop",
            "Рабочий стол",
            &[
                ("index_point", "cursor", 0.55, 0, 0),
                ("pinch", "left_click", 0.72, 0, 180),
                ("double_pinch", "double_click", 0.72, 0, 400),
                ("v_sign", "right_click", 0.6, 100, 500),
                ("thumb_up", "volume_up", 0.6, 100, 400),
                ("thumb_down", "volume_down", 0.6, 100, 400),
                ("swipe_up", "scroll_up", 0.7, 0, 380),
                ("swipe_down", "scroll_down", 0.7, 0, 380),
                ("swipe_left", "prev_window", 0.72, 0, 700),
                ("swipe_right", "next_window", 0.72, 0, 700),
                ("open_palm", "none", 0.6, 0, 0),
                ("closed_fist", "none", 0.6, 0, 0),
                ("two_hand_zoom_in", "zoom_in", 0.7, 0, 280),
                ("two_hand_zoom_out", "zoom_out", 0.7, 0, 280),
                ("two_hand_move_left", "prev_window", 0.72, 0, 800),
                ("two_hand_move_right", "next_window", 0.72, 0, 800),
            ],
        ),
        profile(
            "browser",
            "Браузер",
            &[
                ("index_point", "cursor", 0.55, 0, 0),
                ("pinch", "left_click", 0.72, 0, 180),
                ("double_pinch", "double_click", 0.72, 0, 400),
                ("v_sign", "right_click", 0.75, 220, 650),
                ("thumb_up", "key:ctrl+tab", 0.78, 200, 500),
                ("thumb_down", "key:ctrl+shift+tab", 0.78, 200, 500),
                ("swipe_left", "key:ctrl+shift+tab", 0.74, 0, 500),
                ("swipe_right", "key:ctrl+tab", 0.74, 0, 500),
                ("swipe_up", "scroll_up", 0.7, 0, 320),
                ("swipe_down", "scroll_down", 0.7, 0, 320),
                ("open_palm", "none", 0.6, 0, 0),
                ("closed_fist", "key:ctrl+t", 0.82, 280, 800),
                ("two_hand_zoom_in", "zoom_in", 0.7, 0, 280),
                ("two_hand_zoom_out", "zoom_out", 0.7, 0, 280),
                ("two_hand_move_left", "key:ctrl+shift+tab", 0.74, 0, 700),
                ("two_hand_move_right", "key:ctrl+tab", 0.74, 0, 700),
            ],
        ),
        profile(
            "presentation",
            "Презентация",
            &[
                ("index_point", "cursor", 0.55, 0, 0),
                ("pinch", "left_click", 0.75, 0, 250),
                ("double_pinch", "double_click", 0.75, 0, 500),
                ("v_sign", "key:f5", 0.8, 280, 1200),
                ("thumb_up", "key:right", 0.75, 180, 500),
                ("thumb_down", "key:left", 0.75, 180, 500),
                ("swipe_left", "key:left", 0.72, 0, 550),
                ("swipe_right", "key:right", 0.72, 0, 550),
                ("swipe_up", "key:right", 0.72, 0, 550),
                ("swipe_down", "key:left", 0.72, 0, 550),
                ("open_palm", "none", 0.6, 0, 0),
                ("closed_fist", "none", 0.6, 0, 0),
                ("two_hand_zoom_in", "zoom_in", 0.7, 0, 400),
                ("two_hand_zoom_out", "zoom_out", 0.7, 0, 400),
                ("two_hand_move_left", "key:left", 0.75, 0, 700),
                ("two_hand_move_right", "key:right", 0.75, 0, 700),
            ],
        ),
        profile(
            "media",
            "Медиа",
            &[
                ("index_point", "cursor", 0.55, 0, 0),
                ("pinch", "left_click", 0.72, 0, 180),
                ("double_pinch", "double_click", 0.72, 0, 400),
                ("v_sign", "media_play", 0.76, 200, 700),
                ("thumb_up", "volume_up", 0.74, 160, 350),
                ("thumb_down", "volume_down", 0.74, 160, 350),
                ("swipe_left", "media_prev", 0.72, 0, 700),
                ("swipe_right", "media_next", 0.72, 0, 700),
                ("swipe_up", "volume_up", 0.72, 0, 400),
                ("swipe_down", "volume_down", 0.72, 0, 400),
                ("open_palm", "none", 0.6, 0, 0),
                ("closed_fist", "mute", 0.8, 250, 800),
                ("two_hand_zoom_in", "volume_up", 0.7, 0, 350),
                ("two_hand_zoom_out", "volume_down", 0.7, 0, 350),
                ("two_hand_move_left", "media_prev", 0.72, 0, 800),
                ("two_hand_move_right", "media_next", 0.72, 0, 800),
            ],
        ),
        profile(
            "explorer",
            "Проводник",
            &[
                ("index_point", "cursor", 0.55, 0, 0),
                ("pinch", "left_click", 0.72, 0, 180),
                ("double_pinch", "double_click", 0.72, 0, 400),
                ("v_sign", "key:alt+enter", 0.78, 200, 700),
                ("thumb_up", "key:alt+right", 0.76, 160, 450),
                ("thumb_down", "key:alt+left", 0.76, 160, 450),
                ("swipe_left", "key:alt+left", 0.72, 0, 500),
                ("swipe_right", "key:alt+right", 0.72, 0, 500),
                ("swipe_up", "scroll_up", 0.7, 0, 320),
                ("swipe_down", "scroll_down", 0.7, 0, 320),
                ("open_palm", "none", 0.6, 0, 0),
                ("closed_fist", "key:win+e", 0.82, 280, 900),
                ("two_hand_zoom_in", "zoom_in", 0.7, 0, 280),
                ("two_hand_zoom_out", "zoom_out", 0.7, 0, 280),
                ("two_hand_move_left", "key:alt+left", 0.74, 0, 700),
                ("two_hand_move_right", "key:alt+right", 0.74, 0, 700),
            ],
        ),
        profile(
            "air_keyboard",
            "Воздушная клавиатура",
            &[
                ("index_point", "cursor", 0.55, 0, 0),
                ("pinch", "left_click", 0.74, 0, 200),
                ("double_pinch", "double_click", 0.74, 0, 400),
                ("v_sign", "key:space", 0.76, 180, 400),
                ("thumb_up", "key:enter", 0.76, 180, 400),
                ("thumb_down", "key:escape", 0.76, 180, 400),
                ("closed_fist", "key:tab", 0.8, 220, 450),
                ("open_palm", "none", 0.6, 0, 0),
                ("swipe_left", "key:left", 0.7, 0, 280),
                ("swipe_right", "key:right", 0.7, 0, 280),
                ("swipe_up", "key:up", 0.7, 0, 280),
                ("swipe_down", "key:down", 0.7, 0, 280),
                ("two_hand_zoom_in", "key:ctrl+v", 0.75, 0, 600),
                ("two_hand_zoom_out", "key:ctrl+c", 0.75, 0, 600),
                ("two_hand_move_left", "key:ctrl+z", 0.75, 0, 700),
                ("two_hand_move_right", "key:alt+tab", 0.75, 0, 700),
            ],
        ),
    ]
}

fn profile(id: &str, title: &str, rows: &[(&str, &str, f32, u64, u64)]) -> Profile {
    let mappings = rows
        .iter()
        .filter_map(|(gesture, command, confidence, duration, cooldown)| {
            let command = command_from_id(command).ok()?;
            Some((
                (*gesture).to_string(),
                GestureBinding::new(command, *confidence, *duration, *cooldown),
            ))
        })
        .collect();
    Profile {
        id: id.to_string(),
        title: title.to_string(),
        mappings,
        tune: PointerTune::default(),
    }
}

pub fn factory_profile(id: &str) -> Option<Profile> {
    default_profiles()
        .into_iter()
        .find(|profile| profile.id == id)
}

pub fn profile_for_executable(executable: &str) -> &'static str {
    match executable.to_ascii_lowercase().as_str() {
        "explorer.exe" => "explorer",
        "powerpnt.exe" | "pptview.exe" => "presentation",
        "chrome.exe" | "msedge.exe" | "firefox.exe" | "brave.exe" | "opera.exe" => "browser",
        "spotify.exe" | "vlc.exe" | "wmplayer.exe" => "media",
        _ => "desktop",
    }
}

pub fn profile_coach_line(profile_id: &str) -> &'static str {
    match profile_id {
        "browser" => "Профиль: Браузер — большой палец листает вкладки.",
        "presentation" => "Профиль: Презентация — большой палец листает слайды.",
        "media" => "Профиль: Медиа — большой палец громкость, свайп меняет трек.",
        "explorer" => "Профиль: Проводник — свайп назад/вперёд по папкам, кулак — новое окно.",
        "air_keyboard" => "Профиль: Воздушная клавиатура — большой палец = Enter.",
        _ => "Профиль: Рабочий стол — палец громкость, свайп в сторону = Alt+Tab.",
    }
}
