//! AirControl desktop shell. The window shows engine state; it does not invent it.

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use air_control_core::{
    app_dir, log_dir, profile, start_engine, CommandSender, EngineCommand, PreviewImage,
    COMMAND_CHOICES, VERSION,
};
use anyhow::Result;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

slint::include_modules!();

fn profile_mark(id: &str) -> i32 {
    match id {
        "desktop" => 16,
        "browser" => 17,
        "presentation" => 18,
        "media" => 19,
        "explorer" => 2,
        "air_keyboard" => 20,
        _ => 9,
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("AirControl {VERSION}");
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--diagnose") {
        println!("version: {VERSION}");
        println!("data: {}", app_dir().display());
        println!("logs: {}", log_dir().display());
        println!(
            "vision: {}",
            air_control_core::locate_vision()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "не найдена".into())
        );
        println!(
            "python: {}",
            air_control_core::locate_python(&air_control_core::locate_vision().unwrap_or_default())
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "не найден".into())
        );
        return Ok(());
    }

    air_control_core::enable_per_monitor_dpi();
    init_tracing();
    let mut engine = start_engine()?;
    let commands = engine.commands();
    let snapshot = engine.snapshot();
    let ui = AppWindow::new()?;
    ui.set_version(VERSION.into());
    ui.set_voice_phrases(air_control_core::voice_phrase_catalog().into());
    let choices: Vec<SharedString> = COMMAND_CHOICES
        .iter()
        .map(|(_, label)| SharedString::from(*label))
        .collect();
    ui.set_command_choices(ModelRc::new(VecModel::from(choices)));

    wire(&ui, commands.clone());
    let _ = commands.blocking_send(EngineCommand::SetUiPage(ui.get_page()));
    let weak = ui.as_weak();
    let seen_settings = Rc::new(Cell::new(0u64));
    let seen_gestures = Rc::new(Cell::new(0u64));
    let seen_cameras = Rc::new(Cell::new(0u64));
    let seen_profiles = Rc::new(Cell::new(0u64));
    let seen_frame = Rc::new(Cell::new(0u64));
    let seen_play_frame = Rc::new(Cell::new(0u64));
    let seen_play = Rc::new(Cell::new(0u64));
    let mut seen_camera = 0u64;
    let mut video_times: VecDeque<Instant> = VecDeque::new();
    let timer = slint::Timer::default();
    // 50ms capped the on-screen camera at ~20 FPS even when vision ran faster.
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let snap = snapshot
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .clone();
            ui.set_system_status(snap.system_status.into());
            ui.set_camera_status(snap.camera_status.into());
            ui.set_hand_status(snap.hand_status.into());
            ui.set_gesture_name(snap.gesture.into());
            ui.set_confidence(snap.confidence);
            ui.set_command_name(snap.command.into());
            ui.set_command_status(snap.command_status.into());
            ui.set_rejection(rejection_text(&snap.rejection).into());
            ui.set_profile_name(snap.profile.into());
            ui.set_context_line(snap.context_line.into());
            ui.set_fps(snap.fps);
            ui.set_latency_ms(snap.latency_ms);
            ui.set_capture_ms(snap.capture_ms);
            ui.set_inference_ms(snap.inference_ms);
            ui.set_ipc_ms(snap.ipc_ms);
            ui.set_gesture_ms(snap.gesture_ms);
            ui.set_dispatch_ms(snap.dispatch_ms);
            ui.set_control_on(snap.control_on);
            ui.set_camera_on(snap.camera_on);
            ui.set_paused(snap.paused);
            ui.set_ipc_ok(snap.ipc_ok);
            ui.set_cpu(snap.cpu);
            ui.set_ram_mb(snap.ram_mb);
            ui.set_vision_pid(snap.vision_pid as i32);
            ui.set_error_text(snap.error.into());
            ui.set_calibration_title(snap.calibration_title.into());
            ui.set_calibration_hint(snap.calibration_hint.into());
            ui.set_calibration_progress(snap.calibration_progress);
            ui.set_calibration_active(snap.calibration_active);
            ui.set_coach_line(snap.coach_line.into());
            ui.set_tracking_health(snap.tracking_health.into());
            ui.set_dwell_click(snap.dwell_click);
            ui.set_snap_ui(snap.snap_ui);
            ui.set_snap_radius_px(snap.snap_radius_px);
            ui.set_snap_target(snap.snap_target.into());
            ui.set_session_recording(snap.session_recording);
            ui.set_session_frames(snap.session_frames.min(i32::MAX as u64) as i32);
            ui.set_threshold_suggest(snap.threshold_suggest.into());
            ui.set_suggested_confidence(snap.suggested_confidence);
            ui.set_exam_active(snap.exam_active);
            ui.set_exam_hint(snap.exam_hint.into());
            ui.set_exam_progress(snap.exam_progress);
            ui.set_voice_ready(snap.voice_ready);
            ui.set_voice_listening(snap.voice_listening);
            ui.set_voice_line(snap.voice_line.into());
            ui.set_stats_gestures(snap.stats_gestures.min(i32::MAX as u64) as i32);
            ui.set_stats_commands(snap.stats_commands.min(i32::MAX as u64) as i32);
            ui.set_stats_clicks(snap.stats_clicks.min(i32::MAX as u64) as i32);
            ui.set_stats_scrolls(snap.stats_scrolls.min(i32::MAX as u64) as i32);
            ui.set_stats_rejected(snap.stats_rejected.min(i32::MAX as u64) as i32);
            ui.set_stats_reconnects(snap.stats_reconnects.min(i32::MAX as u64) as i32);
            ui.set_stats_uptime(snap.stats_uptime.min(i32::MAX as u64) as i32);
            ui.set_log_text(snap.log_text.into());
            ui.set_air_keyboard(snap.air_keyboard);
            ui.set_auto_context(snap.auto_context);
            ui.set_play_mode(snap.play_mode);
            ui.set_play_instrument(snap.play_instrument);
            ui.set_play_element(snap.play_element);
            ui.set_play_brush(snap.play_brush);
            ui.set_play_color(snap.play_color);
            ui.set_play_preset(snap.play_preset);
            ui.set_play_guitar_chord(snap.play_guitar_chord);
            ui.set_play_guitar_mode(snap.play_guitar_mode);
            ui.set_play_guitar_capo(snap.play_guitar_capo);
            ui.set_play_guitar_s0(snap.play_guitar_strings[0]);
            ui.set_play_guitar_s1(snap.play_guitar_strings[1]);
            ui.set_play_guitar_s2(snap.play_guitar_strings[2]);
            ui.set_play_guitar_s3(snap.play_guitar_strings[3]);
            ui.set_play_guitar_s4(snap.play_guitar_strings[4]);
            ui.set_play_guitar_s5(snap.play_guitar_strings[5]);
            ui.set_play_muted(snap.play_muted);
            ui.set_play_label(snap.play_label.into());
            ui.set_play_hint(snap.play_hint.into());
            if seen_play.get() != snap.play_revision {
                ui.set_play_volume(snap.play_volume);
                ui.set_play_sensitivity(snap.play_sensitivity);
                ui.set_play_thickness(snap.play_thickness);
                ui.set_play_fade(snap.play_fade);
                seen_play.set(snap.play_revision);
            }
            if seen_settings.get() != snap.settings_revision {
                ui.set_show_camera(snap.show_camera);
                ui.set_mirror(snap.mirror);
                ui.set_speed(snap.speed);
                ui.set_smoothness(snap.smoothness);
                ui.set_acceleration(snap.acceleration);
                ui.set_deadzone(snap.deadzone);
                ui.set_confidence_threshold(snap.confidence_threshold);
                ui.set_depth_cursor(snap.depth_cursor);
                ui.set_depth_gain(snap.depth_gain);
                ui.set_quality_index(snap.quality_index);
                ui.set_hand_index(snap.hand_index);
                ui.set_sensitivity_index(snap.sensitivity_index);
                seen_settings.set(snap.settings_revision);
            }
            if seen_gestures.get() != snap.gestures_revision {
                let rows: Vec<GestureRow> = snap
                    .gestures
                    .iter()
                    .map(|row| GestureRow {
                        id: row.id.clone().into(),
                        title: row.title.clone().into(),
                        command: row.command.clone().into(),
                        detail: row.detail.clone().into(),
                        how: row.how.clone().into(),
                        custom: row.custom,
                    })
                    .collect();
                ui.set_gestures(ModelRc::new(VecModel::from(rows)));
                seen_gestures.set(snap.gestures_revision);
            }
            if seen_cameras.get() != snap.cameras_revision {
                let rows: Vec<CameraRow> = snap
                    .cameras
                    .iter()
                    .map(|row| CameraRow {
                        index: row.index,
                        label: row.label.clone().into(),
                        selected: row.selected,
                    })
                    .collect();
                ui.set_cameras(ModelRc::new(VecModel::from(rows)));
                seen_cameras.set(snap.cameras_revision);
            }
            if seen_profiles.get() != snap.profiles_revision {
                let rows: Vec<ProfileRow> = snap
                    .profiles
                    .iter()
                    .map(|row| ProfileRow {
                        id: row.id.clone().into(),
                        title: row.title.clone().into(),
                        active: row.active,
                        mark: profile_mark(&row.id),
                        override_tune: row.override_tune,
                        speed: row.speed,
                        smoothness: row.smoothness,
                        deadzone: row.deadzone,
                        confidence: row.confidence,
                    })
                    .collect();
                let titles: Vec<SharedString> = snap
                    .profiles
                    .iter()
                    .map(|row| SharedString::from(row.title.as_str()))
                    .collect();
                ui.set_profile_id_choices(ModelRc::new(VecModel::from(titles)));
                let rule_rows: Vec<ContextRuleRow> = snap
                    .context_rules
                    .iter()
                    .map(|rule| ContextRuleRow {
                        pattern: rule.pattern.clone().into(),
                        profile_id: rule.profile_id.clone().into(),
                        profile_title: rule.profile_title.clone().into(),
                        enabled: rule.enabled,
                    })
                    .collect();
                ui.set_context_rules(ModelRc::new(VecModel::from(rule_rows)));
                ui.set_profiles(ModelRc::new(VecModel::from(rows)));
                let selected = ui.get_selected_profile_id().to_string();
                let tune = snap
                    .profiles
                    .iter()
                    .find(|row| row.id == selected || (selected.is_empty() && row.active))
                    .or_else(|| snap.profiles.iter().find(|row| row.active));
                if let Some(row) = tune {
                    if selected.is_empty() {
                        ui.set_selected_profile_id(row.id.clone().into());
                    }
                    ui.set_profile_tune_override(row.override_tune);
                    ui.set_profile_tune_speed(row.speed);
                    ui.set_profile_tune_smoothness(row.smoothness);
                    ui.set_profile_tune_deadzone(row.deadzone);
                    ui.set_profile_tune_confidence(row.confidence);
                }
                seen_profiles.set(snap.profiles_revision);
            }
            // Push camera bitmaps only for pages that actually display them.
            // UI FPS samples only successful image pushes (revision change + set_*),
            // never bare timer ticks.
            let page = ui.get_page();
            let show_frame = matches!(page, 0 | 1 | 3 | 7 | 10);
            let show_play = page == 12;
            if show_frame && seen_frame.get() != snap.frame_revision {
                if let Some(preview) = snap.preview.as_ref() {
                    let image = if profile::enabled() {
                        let started = Instant::now();
                        let image = frame_image(preview);
                        profile::write_ui_sample(
                            profile::ms_f32(started.elapsed()),
                            "frame",
                            preview.width,
                            preview.height,
                        );
                        image
                    } else {
                        frame_image(preview)
                    };
                    ui.set_frame(image);
                }
                seen_frame.set(snap.frame_revision);
            }
            ui.set_play_frame_ready(show_play && snap.play_frame_ready);
            if show_play && seen_play_frame.get() != snap.play_frame_revision {
                if let Some(preview) = snap.play_preview.as_ref() {
                    let image = if profile::enabled() {
                        let started = Instant::now();
                        let image = frame_image(preview);
                        profile::write_ui_sample(
                            profile::ms_f32(started.elapsed()),
                            "play_frame",
                            preview.width,
                            preview.height,
                        );
                        image
                    } else {
                        frame_image(preview)
                    };
                    ui.set_play_frame(image);
                    if seen_camera != snap.play_camera_revision && snap.show_camera && snap.camera_on {
                        seen_camera = snap.play_camera_revision;
                        video_times.push_back(Instant::now());
                    }
                }
                seen_play_frame.set(snap.play_frame_revision);
            }
            let now = Instant::now();
            while video_times.front().is_some_and(|time| now.duration_since(*time).as_secs_f32() > 1.5) {
                video_times.pop_front();
            }
            if !show_play || !snap.show_camera || !snap.camera_on {
                video_times.clear();
                seen_camera = snap.play_camera_revision;
            }
            let video_fps = match (video_times.front(), video_times.back()) {
                (Some(first), Some(last)) if video_times.len() > 1
                    && now.duration_since(*last) < Duration::from_millis(500) => {
                    (video_times.len() - 1) as f32 / last.duration_since(*first).as_secs_f32().max(0.001)
                }
                _ => 0.0,
            };
            ui.set_preview_fps(video_fps);
            #[cfg(windows)]
            {
                let hwnd = air_hwnd();
                let iconic = hwnd != 0 && unsafe { IsIconic(hwnd) != 0 };
                if iconic && !ui.get_mini() {
                    unsafe {
                        ShowWindow(hwnd, SW_RESTORE);
                    }
                    place_window(&ui, true);
                    let _ = ui.show();
                    park_mini(hwnd);
                }
            }
        },
    );

    ui.run()?;
    drop(timer);
    engine.shutdown();
    Ok(())
}

fn wire(ui: &AppWindow, commands: CommandSender) {
    ui.on_start_camera({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::StartCamera);
        }
    });
    ui.on_stop_camera({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::StopCamera);
        }
    });
    ui.on_set_control({
        let tx = commands.clone();
        move |enabled| {
            let _ = tx.blocking_send(EngineCommand::SetControl(enabled));
        }
    });
    ui.on_toggle_pause({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::TogglePause);
        }
    });
    ui.on_emergency_stop({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::EmergencyStop);
        }
    });
    ui.on_select_camera({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SelectCamera(index));
        }
    });
    ui.on_select_profile({
        let tx = commands.clone();
        move |id| {
            let _ = tx.blocking_send(EngineCommand::SetProfile(id.to_string()));
        }
    });
    ui.on_set_auto_context({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetAutoContext(value));
        }
    });
    ui.on_set_air_keyboard({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetAirKeyboard(value));
        }
    });
    ui.on_set_dwell_click({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetDwellClick(value));
        }
    });
    ui.on_set_snap_ui({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetSnapUi(value));
        }
    });
    ui.on_set_snap_radius({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetSnapRadius(value));
        }
    });
    ui.on_toggle_session_record({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::ToggleSessionRecord);
        }
    });
    ui.on_add_context_rule({
        let tx = commands.clone();
        let ui_weak = ui.as_weak();
        move |pattern, profile_title| {
            let profile_id = ui_weak
                .upgrade()
                .and_then(|ui| {
                    let model = ui.get_profiles();
                    for i in 0..model.row_count() {
                        if let Some(row) = model.row_data(i) {
                            if row.title == profile_title {
                                return Some(row.id.to_string());
                            }
                        }
                    }
                    None
                })
                .unwrap_or_else(|| profile_title.to_string());
            let _ = tx.blocking_send(EngineCommand::AddContextRule {
                pattern: pattern.to_string(),
                profile_id,
            });
        }
    });
    ui.on_toggle_context_rule({
        let tx = commands.clone();
        move |pattern| {
            let _ = tx.blocking_send(EngineCommand::ToggleContextRule {
                pattern: pattern.to_string(),
            });
        }
    });
    ui.on_remove_context_rule({
        let tx = commands.clone();
        move |pattern| {
            let _ = tx.blocking_send(EngineCommand::RemoveContextRule {
                pattern: pattern.to_string(),
            });
        }
    });
    ui.on_reset_profile({
        let tx = commands.clone();
        move |id| {
            let _ = tx.blocking_send(EngineCommand::ResetProfile(id.to_string()));
        }
    });
    ui.on_set_profile_tune({
        let tx = commands.clone();
        move |profile_id, override_tune, speed, smoothness, deadzone, confidence| {
            let _ = tx.blocking_send(EngineCommand::SetProfileTune {
                profile_id: profile_id.to_string(),
                override_tune,
                speed,
                smoothness,
                deadzone,
                confidence,
            });
        }
    });
    ui.on_suggest_thresholds({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::SuggestThresholds);
        }
    });
    ui.on_apply_suggested_confidence({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::ApplySuggestedConfidence(value));
        }
    });
    ui.on_start_exam({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::StartExam);
        }
    });
    ui.on_set_show_camera({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetShowCamera(value));
        }
    });
    ui.on_set_mirror({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetMirror(value));
        }
    });
    ui.on_set_quality({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SetQuality(index));
        }
    });
    ui.on_set_hand({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SetPrimaryHand(index));
        }
    });
    ui.on_set_sens_profile({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SetSensitivityProfile(index));
        }
    });
    ui.on_set_speed({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetSpeed(value));
        }
    });
    ui.on_set_smoothness({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetSmoothness(value));
        }
    });
    ui.on_set_acceleration({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetAcceleration(value));
        }
    });
    ui.on_set_deadzone({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetDeadzone(value));
        }
    });
    ui.on_set_confidence({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetConfidence(value));
        }
    });
    ui.on_set_depth_cursor({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetDepthCursor(value));
        }
    });
    ui.on_set_depth_gain({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetDepthGain(value));
        }
    });
    ui.on_calibration_start({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::CalibrationStart);
        }
    });
    ui.on_calibration_next({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::CalibrationNext);
        }
    });
    ui.on_calibration_cancel({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::CalibrationCancel);
        }
    });
    ui.on_calibration_drill({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::CalibrationDrill);
        }
    });
    ui.on_record_gesture({
        let tx = commands.clone();
        move |name, threshold| {
            let _ = tx.blocking_send(EngineCommand::RecordGesture {
                name: name.to_string(),
                threshold,
            });
        }
    });
    ui.on_delete_gesture({
        let tx = commands.clone();
        move |name| {
            let _ = tx.blocking_send(EngineCommand::DeleteGesture(name.to_string()));
        }
    });
    ui.on_assign_command({
        let tx = commands.clone();
        move |gesture, label| {
            let _ = tx.blocking_send(EngineCommand::AssignCommand {
                gesture: gesture.to_string(),
                label: label.to_string(),
            });
        }
    });
    ui.on_export_gestures({
        let tx = commands.clone();
        move || {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("air-control-gestures.json")
                .add_filter("JSON", &["json"])
                .save_file()
            {
                let _ = tx.blocking_send(EngineCommand::ExportGestures(path));
            }
        }
    });
    ui.on_import_gestures({
        let tx = commands.clone();
        move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_file()
            {
                let _ = tx.blocking_send(EngineCommand::ImportGestures(path));
            }
        }
    });
    ui.on_open_logs({
        move || {
            let directory = log_dir();
            let _ = std::fs::create_dir_all(&directory);
            let _ = std::process::Command::new("explorer")
                .arg(directory)
                .spawn();
        }
    });
    ui.on_refresh_cameras({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::RefreshCameras);
        }
    });
    ui.on_open_camera_settings({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::OpenCameraSettings);
        }
    });
    ui.on_set_play_mode({
        let tx = commands.clone();
        move |mode| {
            let _ = tx.blocking_send(EngineCommand::SetPlayMode(mode));
        }
    });
    ui.on_set_ui_page({
        let tx = commands.clone();
        move |page| {
            let _ = tx.blocking_send(EngineCommand::SetUiPage(page));
        }
    });
    ui.on_set_play_instrument({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SetPlayInstrument(index));
        }
    });
    ui.on_set_play_element({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SetPlayElement(index));
        }
    });
    ui.on_set_play_brush({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SetPlayBrush(index));
        }
    });
    ui.on_set_play_color({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SetPlayColor(index));
        }
    });
    ui.on_set_play_preset({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SetPlayPreset(index));
        }
    });
    ui.on_set_guitar_chord({
        let tx = commands.clone();
        move |index| {
            let _ = tx.blocking_send(EngineCommand::SetGuitarChord(index));
        }
    });
    ui.on_set_guitar_mode({
        let tx = commands.clone();
        move |mode| {
            let _ = tx.blocking_send(EngineCommand::SetGuitarMode(mode));
        }
    });
    ui.on_set_guitar_capo({
        let tx = commands.clone();
        move |capo| {
            let _ = tx.blocking_send(EngineCommand::SetGuitarCapo(capo));
        }
    });
    ui.on_pluck_guitar_string({
        let tx = commands.clone();
        move |string| {
            let _ = tx.blocking_send(EngineCommand::PluckGuitarString(string));
        }
    });
    ui.on_set_play_muted({
        let tx = commands.clone();
        move |muted| {
            let _ = tx.blocking_send(EngineCommand::SetPlayMuted(muted));
        }
    });
    ui.on_set_play_volume({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetPlayVolume(value));
        }
    });
    ui.on_set_play_sensitivity({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetPlaySensitivity(value));
        }
    });
    ui.on_set_play_thickness({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetPlayThickness(value));
        }
    });
    ui.on_set_play_fade({
        let tx = commands.clone();
        move |value| {
            let _ = tx.blocking_send(EngineCommand::SetPlayFade(value));
        }
    });
    ui.on_clear_art({
        let tx = commands.clone();
        move || {
            let _ = tx.blocking_send(EngineCommand::ClearArt);
        }
    });
    ui.on_set_voice_listen({
        let tx = commands.clone();
        move |enabled| {
            let _ = tx.blocking_send(EngineCommand::SetVoiceListen(enabled));
        }
    });
    ui.on_toggle_mini({
        let weak = ui.as_weak();
        move || {
            let Some(ui) = weak.upgrade() else { return };
            place_window(&ui, !ui.get_mini());
        }
    });
}

fn place_window(ui: &AppWindow, mini: bool) {
    ui.set_mini(mini);
    let window = ui.window();
    if mini {
        window.set_size(slint::WindowSize::Logical(slint::LogicalSize::new(
            280.0, 430.0,
        )));
        let (x, y) = mini_corner();
        window.set_position(slint::WindowPosition::Physical(
            slint::PhysicalPosition::new(x, y),
        ));
    } else {
        window.set_size(slint::WindowSize::Logical(slint::LogicalSize::new(
            1240.0, 800.0,
        )));
    }
}

fn mini_corner() -> (i32, i32) {
    #[cfg(windows)]
    {
        let width = unsafe { GetSystemMetrics(0) };
        return ((width - 312).max(16), 64);
    }
    #[cfg(not(windows))]
    {
        (64, 64)
    }
}

#[cfg(windows)]
fn air_hwnd() -> isize {
    let title: Vec<u16> = "AIR CONTROL\0".encode_utf16().collect();
    unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }
}

#[cfg(windows)]
fn park_mini(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    let (x, y) = mini_corner();
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, 420, 640, SWP_SHOWWINDOW);
    }
}

#[cfg(windows)]
const SW_RESTORE: i32 = 9;
#[cfg(windows)]
const HWND_TOPMOST: isize = -1;
#[cfg(windows)]
const SWP_SHOWWINDOW: u32 = 0x0040;

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetSystemMetrics(index: i32) -> i32;
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> isize;
    fn IsIconic(hwnd: isize) -> i32;
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn SetWindowPos(
        hwnd: isize,
        insert_after: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

fn frame_image(preview: &PreviewImage) -> slint::Image {
    let mut buffer =
        slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(preview.width, preview.height);
    buffer.make_mut_bytes().copy_from_slice(&preview.rgb);
    slint::Image::from_rgb8(buffer)
}

fn rejection_text(code: &str) -> String {
    match code {
        "" => String::new(),
        "paused" => "Управление на паузе".into(),
        "low_confidence" => "Недостаточная уверенность".into(),
        "cooldown" => "Защита от повторного срабатывания".into(),
        "control_off" => "Управление выключено".into(),
        "hand_lost" => "Рука потеряна".into(),
        "not_allowed" => "Команда вне разрешённого списка".into(),
        "conflict" => "Окно AirControl не сворачивается жестом".into(),
        "edge" => "Рука слишком близко к краю кадра".into(),
        "shake" => "Рука дрожит: подержите жест спокойнее".into(),
        "tracking" => "Плохой сигнал камеры: темно, смаз или мерцание".into(),
        "input_unavailable" => "Системный ввод недоступен".into(),
        "too_short" => "Жест ещё слишком короткий".into(),
        other => other.to_string(),
    }
}

fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let _ = std::fs::create_dir_all(log_dir());
    let file = tracing_appender::rolling::daily(log_dir(), "air-control.log");
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(fmt::layer().with_ansi(false).with_writer(file))
        .init();
}
