use std::time::Duration;

use crate::timer::{Action, Mode, TimerApp, TimerEvent, TimerInfo};
use crate::Settings;

fn basic(info: TimerInfo) -> (Mode, u64, bool) {
    (info.mode, info.remaining, info.paused)
}

fn simple_timer() -> TimerApp {
    TimerApp::new(
        Settings {
            work_minutes: 1,
            break_minutes: 1,
            idle_reset: false,
            tips: vec!["走动一下".into()],
            ..Settings::default()
        },
        fastrand::Rng::with_seed(42),
    )
}

#[test]
fn upgrading_old_settings_keeps_simple_timer_and_all_shared_preferences() {
    let path =
        std::env::temp_dir().join(format!("sit-break-old-settings-{}.json", fastrand::u64(..)));
    std::fs::write(
        &path,
        r#"{"work_minutes":30,"break_minutes":3,"sound":false,"show_widget":false,"autostart":true,"idle_reset":false,"language":"en","widget_size":96,"tips":["Walk"]}"#,
    )
    .unwrap();
    let app = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 1800);
    let settings = serde_json::to_value(app.settings()).unwrap();
    assert_eq!(settings["use_schedule"], false);
    assert_eq!(settings["schedules"], serde_json::json!([]));
    assert_eq!(settings["break_minutes"], 3);
    assert_eq!(settings["sound"], false);
    assert_eq!(settings["show_widget"], false);
    assert_eq!(settings["autostart"], true);
    assert_eq!(settings["idle_reset"], false);
    assert_eq!(settings["language"], "en");
    assert_eq!(settings["widget_size"], 96);
    assert_eq!(settings["tips"][0]["text"], "Walk");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn migrated_reminder_entries_can_be_edited_and_removed_independently_after_reload() {
    let path = std::env::temp_dir().join(format!(
        "sit-break-reminder-entries-{}.json",
        fastrand::u64(..)
    ));
    let mut old = serde_json::to_value(scheduled_settings("12:00")).unwrap();
    old["tips"] = serde_json::json!(["相同文字", "相同文字", "<b>伸展</b>"]);
    old["sound"] = false.into();
    std::fs::write(&path, serde_json::to_string(&old).unwrap()).unwrap();
    let mut app = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    let mut draft = serde_json::to_value(app.settings()).unwrap();
    assert_eq!(draft["tips"][0]["text"], "相同文字");
    let first_id = draft["tips"][0]["id"].as_str().unwrap().to_owned();
    let second_id = draft["tips"][1]["id"].as_str().unwrap().to_owned();
    assert!(!first_id.is_empty());
    assert_ne!(first_id, second_id);
    draft["tips"][1]["text"] = "只修改第二条".into();
    draft["tips"].as_array_mut().unwrap().remove(0);
    let saved = app.dispatch(
        Action::SaveSettings(serde_json::from_value(draft).unwrap()),
        Duration::ZERO,
    );
    assert!(saved.validation_errors.is_empty());
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut reopened = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    let settings = serde_json::to_value(reopened.settings()).unwrap();
    assert_eq!(settings["tips"][0]["id"], second_id);
    assert_eq!(settings["tips"][0]["text"], "只修改第二条");
    assert_eq!(settings["tips"][1]["text"], "<b>伸展</b>");
    assert_eq!(settings["tips"].as_array().unwrap().len(), 2);
    assert_eq!(settings["schedules"], old["schedules"]);
    assert_eq!(settings["sound"], false);
    assert!(settings["use_schedule"].as_bool().unwrap());
    let mut other_preferences = reopened.settings().clone();
    other_preferences.language = "en".into();
    reopened.dispatch(Action::SaveSettings(other_preferences), Duration::ZERO);
    assert_eq!(
        serde_json::to_value(reopened.settings()).unwrap()["tips"],
        settings["tips"]
    );
    let mut cleared = reopened.settings().clone();
    cleared.tips.clear();
    cleared.use_schedule = false;
    let saved = reopened.dispatch(Action::SaveSettings(cleared), Duration::ZERO);
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut fallback = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    fallback.dispatch(
        Action::Tick {
            elapsed_secs: 2700,
            idle_secs: 0,
        },
        Duration::from_secs(2700),
    );
    assert!(crate::TIPS_EN.contains(&fallback.info().tip.as_deref().unwrap()));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn empty_reminder_text_cannot_replace_saved_entries_or_reset_the_current_timer() {
    let path = std::env::temp_dir().join(format!(
        "sit-break-empty-reminder-{}.json",
        fastrand::u64(..)
    ));
    let mut app = simple_timer();
    app.settings().save_to(&path).unwrap();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 12,
            idle_secs: 0,
        },
        Duration::from_secs(12),
    );
    let before = app.info();
    let mut draft = app.settings().clone();
    draft.tips[0].text = " \n ".into();
    let saved = app.dispatch(Action::SaveSettings(draft.clone()), Duration::from_secs(12));
    assert!(saved
        .validation_errors
        .iter()
        .any(|error| { error.field == "tips.0.text" && error.code == "reminder_empty" }));
    assert!(saved.persist_settings.is_none());
    assert_eq!(app.info(), before);
    assert!(draft.save_to(&path).is_err());
    let reopened = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert_eq!(reopened.settings().tips[0].text, "走动一下");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn saving_reminder_entries_rejects_missing_or_duplicate_identities() {
    let mut app = simple_timer();
    for missing in [false, true] {
        let mut draft = app.settings().clone();
        if missing {
            draft.tips[0].id.clear();
        } else {
            draft.tips.push(draft.tips[0].clone());
        }
        let saved = app.dispatch(Action::SaveSettings(draft), Duration::ZERO);
        assert!(saved
            .validation_errors
            .iter()
            .any(|error| error.code == "reminder_identity"));
        assert!(saved.persist_settings.is_none());
        assert_eq!(app.settings().tips.len(), 1);
    }
}

#[test]
fn reminder_text_is_displayed_verbatim_and_stays_selected_when_preferences_are_saved() {
    let mut app = simple_timer();
    let mut draft = app.settings().clone();
    draft.tips[0].text = "  <b>用户自己的提示</b>\n第二行  ".into();
    app.dispatch(Action::SaveSettings(draft), Duration::ZERO);
    let reminder = app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    assert!(reminder.events.iter().any(|event| matches!(event,
        TimerEvent::Reminder(payload) if payload.tip == "  <b>用户自己的提示</b>\n第二行  "
    )));
    app.dispatch(Action::StartBreak, Duration::from_secs(60));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 15,
            idle_secs: 0,
        },
        Duration::from_secs(75),
    );
    let before = app.info();
    let mut draft = app.settings().clone();
    draft.language = "en".into();
    draft.sound = !draft.sound;
    draft.tips[0].text = "下一次提醒".into();
    app.dispatch(Action::SaveSettings(draft), Duration::from_secs(75));
    assert_eq!(app.info(), before);
    assert_eq!(app.info().remaining, 45);
}

#[test]
fn invalid_online_video_addresses_leave_the_previous_saved_reminder_and_timer_intact() {
    let path = std::env::temp_dir().join(format!(
        "sit-break-online-invalid-{}.json",
        fastrand::u64(..)
    ));
    let mut app = simple_timer();
    app.settings().save_to(&path).unwrap();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 12,
            idle_secs: 0,
        },
        Duration::from_secs(12),
    );
    let previous = app.info();
    for address in [
        "https://user:secret@example.com/demo.mp4",
        "https://example.com/\nvideo.mp4",
        "javascript:alert(1)",
        "file:///tmp/video.mp4",
        "https://",
        "not a url",
        " https://example.com/demo.mp4",
        "https://example.com/%0Avideo.mp4",
        "https:example.com/video.mp4",
        "https://example.com\\video.mp4",
    ] {
        let mut draft = serde_json::to_value(app.settings()).unwrap();
        draft["tips"][0]["guidance"] =
            serde_json::json!({"kind":"video","source":{"type":"online","url":address}});
        let draft: Settings = serde_json::from_value(draft).unwrap();
        let output = app.dispatch(Action::SaveSettings(draft.clone()), Duration::from_secs(12));
        assert!(
            output
                .validation_errors
                .iter()
                .any(|error| error.code == "guidance_invalid"),
            "accepted {address:?}"
        );
        assert!(output.persist_settings.is_none());
        assert!(draft.save_to(&path).is_err());
        assert_eq!(app.info(), previous);
        assert!(app.settings().tips[0].guidance.is_none());
    }
    let reopened = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert!(reopened.settings().tips[0].guidance.is_none());
    assert_eq!(reopened.settings().tips[0].text, "走动一下");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn online_video_reminder_is_saved_reloaded_and_kept_for_the_active_break() {
    let path = std::env::temp_dir().join(format!("sit-break-online-{}.json", fastrand::u64(..)));
    let mut app = simple_timer();
    let mut draft = serde_json::to_value(app.settings()).unwrap();
    let guidance = serde_json::json!({"kind":"video","source":{"type":"online","url":"http://127.0.0.1:34567/demo.mp4?shared=1#clip"}});
    draft["tips"][0]["text"] = "".into();
    draft["tips"][0]["guidance"] = guidance.clone();
    let saved = app.dispatch(
        Action::SaveSettings(serde_json::from_value(draft).unwrap()),
        Duration::ZERO,
    );
    assert!(saved.validation_errors.is_empty());
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut app = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    assert_eq!(
        serde_json::to_value(app.info()).unwrap()["guidance"],
        guidance
    );
    assert_eq!(app.info().tip.as_deref(), Some(""));
    app.dispatch(Action::StartBreak, Duration::from_secs(60));
    let mut replacement = app.settings().clone();
    replacement.tips[0].guidance = None;
    replacement.tips[0].text = "下轮纯文字".into();
    app.dispatch(Action::SaveSettings(replacement), Duration::from_secs(60));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 15,
            idle_secs: 0,
        },
        Duration::from_secs(75),
    );
    assert_eq!(app.info().remaining, 45);
    assert_eq!(
        serde_json::to_value(app.info()).unwrap()["guidance"],
        guidance
    );
    app.dispatch(Action::EndBreak, Duration::from_secs(75));
    assert!(app.info().guidance.is_none());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn online_html_cannot_replace_a_saved_guide_with_an_application_protocol_address() {
    let mut app = simple_timer();
    let previous = app.settings().clone();
    for address in [
        "http://tauri.localhost/reminder.html",
        "http://ipc.localhost/invoke",
        "https://guidance.localhost/local-0123456789abcdef0123456789abcdef",
    ] {
        let mut draft = serde_json::to_value(app.settings()).unwrap();
        draft["tips"][0]["guidance"] =
            serde_json::json!({"kind":"html","source":{"type":"online","url":address}});
        let result = app.dispatch(
            Action::SaveSettings(serde_json::from_value(draft).unwrap()),
            Duration::ZERO,
        );
        assert!(!result.validation_errors.is_empty(), "accepted {address}");
        assert!(result.persist_settings.is_none());
        assert_eq!(
            serde_json::to_value(app.settings()).unwrap(),
            serde_json::to_value(&previous).unwrap()
        );
    }
}

#[test]
fn online_html_only_reminder_is_saved_reopened_and_kept_when_its_address_is_replaced() {
    let path =
        std::env::temp_dir().join(format!("sit-break-online-html-{}.json", fastrand::u64(..)));
    let mut app = simple_timer();
    let mut draft = serde_json::to_value(app.settings()).unwrap();
    let guide = serde_json::json!({"kind":"html","source":{"type":"online","url":"http://127.0.0.1:34567/guide.html?shared=1#steps"}});
    draft["tips"][0]["text"] = "".into();
    draft["tips"][0]["guidance"] = guide.clone();
    let saved = app.dispatch(
        Action::SaveSettings(serde_json::from_value(draft).unwrap()),
        Duration::ZERO,
    );
    assert!(
        saved.validation_errors.is_empty(),
        "{:?}",
        saved.validation_errors
    );
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut app = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert_eq!(
        serde_json::to_value(app.settings()).unwrap()["tips"][0]["guidance"],
        guide
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    app.dispatch(Action::StartBreak, Duration::from_secs(60));
    let mut replacement = app.settings().clone();
    replacement.tips[0].guidance = Some(serde_json::from_value(serde_json::json!({"kind":"html","source":{"type":"online","url":"https://example.com/new-guide"}})).unwrap());
    let saved = app.dispatch(Action::SaveSettings(replacement), Duration::from_secs(60));
    assert!(saved.validation_errors.is_empty());
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 15,
            idle_secs: 0,
        },
        Duration::from_secs(75),
    );
    assert_eq!(app.info().remaining, 45);
    assert_eq!(app.info().tip.as_deref(), Some(""));
    assert_eq!(serde_json::to_value(app.info()).unwrap()["guidance"], guide);
    let mut reopened = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    let mut removed = reopened.settings().clone();
    removed.tips[0].guidance = None;
    removed.tips[0].text = "下一轮文字".into();
    let saved = reopened.dispatch(Action::SaveSettings(removed), Duration::ZERO);
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    assert!(TimerApp::load_from(&path, fastrand::Rng::with_seed(42))
        .settings()
        .tips[0]
        .guidance
        .is_none());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn builtin_html_can_be_selected_for_a_reminder_and_a_scheduled_round_after_reopening() {
    let path =
        std::env::temp_dir().join(format!("sit-break-builtin-html-{}.json", fastrand::u64(..)));
    let guide = serde_json::json!({"kind":"html", "source":{"type":"builtin","id":"stretch-demo"}});
    let mut app = simple_timer();
    let mut draft = serde_json::to_value(app.settings()).unwrap();
    draft["tips"] = serde_json::json!([{"id":"builtin-html-entry","text":"","guidance":guide}]);
    let saved = app.dispatch(
        Action::SaveSettings(serde_json::from_value(draft).unwrap()),
        Duration::ZERO,
    );
    assert!(saved.validation_errors.is_empty());
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut reopened = TimerApp::load_from(&path, fastrand::Rng::with_seed(1));
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    reopened.dispatch(Action::StartBreak, Duration::from_secs(60));
    assert_eq!(reopened.info().tip.as_deref(), Some(""));
    assert_eq!(
        serde_json::to_value(reopened.info()).unwrap()["guidance"],
        guide
    );

    let mut scheduled = serde_json::to_value(scheduled_settings("12:00")).unwrap();
    scheduled["schedules"][0]["periods"][0]["overrides"] =
        serde_json::json!([{"round":1,"guidance":guide}]);
    let configured: Settings = serde_json::from_value(scheduled).unwrap();
    configured.save_to(&path).unwrap();
    let app = TimerApp::new_at(
        Settings::load_from(&path),
        fastrand::Rng::with_seed(1),
        at(9, 0),
    );
    assert_eq!(serde_json::to_value(app.info()).unwrap()["guidance"], guide);
    assert!(app.info().tip.as_deref().unwrap_or_default().is_empty());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn html_only_reminder_keeps_its_guide_after_save_reopen_and_starting_a_break() {
    let path = std::env::temp_dir().join(format!("sit-break-html-{}.json", fastrand::u64(..)));
    let mut draft = serde_json::to_value(Settings::default()).unwrap();
    let guide = serde_json::json!({"kind":"html", "source":{"type":"local","id":"local-0123456789abcdef0123456789abcdef"}});
    draft["tips"] = serde_json::json!([{"id":"html-entry","text":"","guidance":guide}]);
    draft["work_minutes"] = serde_json::json!(1);
    draft["idle_reset"] = serde_json::json!(false);
    let configured: Settings = serde_json::from_value(draft).unwrap();
    configured.save_to(&path).unwrap();
    let mut app = TimerApp::new(Settings::load_from(&path), fastrand::Rng::with_seed(1));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    app.dispatch(Action::StartBreak, Duration::from_secs(60));
    assert_eq!(app.info().mode, Mode::Break);
    assert_eq!(app.info().tip.as_deref(), Some(""));
    assert_eq!(serde_json::to_value(app.info()).unwrap()["guidance"], guide);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn guidance_only_reminder_can_be_saved_reopened_and_shown_without_added_text() {
    let path = std::env::temp_dir().join(format!("sit-break-guidance-{}.json", fastrand::u64(..)));
    let mut app = simple_timer();
    let mut draft = serde_json::to_value(app.settings()).unwrap();
    draft["tips"][0]["text"] = "".into();
    let guidance =
        serde_json::json!({"kind":"video","source":{"type":"builtin","id":"squat-demo"}});
    draft["tips"][0]["guidance"] = guidance.clone();
    let saved = app.dispatch(
        Action::SaveSettings(serde_json::from_value(draft).unwrap()),
        Duration::ZERO,
    );
    assert!(saved.validation_errors.is_empty());
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut reopened = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert_eq!(
        serde_json::to_value(reopened.settings()).unwrap()["tips"][0]["guidance"],
        guidance
    );
    let output = reopened.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    let state = serde_json::to_value(output.state.unwrap()).unwrap();
    assert_eq!(state["tip"], "");
    assert_eq!(state["guidance"], guidance);
    let TimerEvent::Reminder(payload) = &output.events[0] else {
        panic!("expected reminder");
    };
    assert_eq!(serde_json::to_value(payload).unwrap()["guidance"], guidance);
    reopened.dispatch(Action::StartBreak, Duration::from_secs(60));
    assert_eq!(
        serde_json::to_value(reopened.info()).unwrap()["guidance"],
        guidance
    );
    let active = reopened.info();
    let mut replacement = reopened.settings().clone();
    replacement.tips[0].text = "下一次的纯文字".into();
    replacement.tips[0].guidance = None;
    replacement.language = "en".into();
    reopened.dispatch(Action::SaveSettings(replacement), Duration::from_secs(60));
    assert_eq!(
        reopened.info(),
        active,
        "保存其他偏好或替换配置不打断当前休息"
    );
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 15,
            idle_secs: 0,
        },
        Duration::from_secs(75),
    );
    assert_eq!(reopened.info().remaining, 45);
    assert_eq!(
        serde_json::to_value(reopened.info()).unwrap()["guidance"],
        guidance
    );
    reopened.dispatch(Action::EndBreak, Duration::from_secs(75));
    assert_eq!(reopened.info().mode, Mode::Work);
    assert!(reopened.info().guidance.is_none());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn simple_postponed_reminders_rotate_complete_entries_even_when_text_matches() {
    let mut app = simple_timer();
    let mut draft = serde_json::to_value(app.settings()).unwrap();
    draft["tips"] = serde_json::json!([
        {"id":"video","text":"下蹲","guidance":{"kind":"video","source":{"type":"builtin","id":"squat-demo"}}},
        {"id":"text","text":"下蹲","guidance":null}
    ]);
    app.dispatch(
        Action::SaveSettings(serde_json::from_value(draft).unwrap()),
        Duration::ZERO,
    );
    let mut previous = None;
    for n in 1..=12 {
        let now = Duration::from_secs(n * 60);
        app.dispatch(
            Action::Tick {
                elapsed_secs: 60,
                idle_secs: 0,
            },
            now,
        );
        let state = serde_json::to_value(app.info()).unwrap();
        assert_eq!(state["tip"], "下蹲");
        if let Some(previous) = previous {
            assert_ne!(state["guidance"], previous, "推迟后应轮换完整条目");
        }
        previous = Some(state["guidance"].clone());
        app.dispatch(Action::Postpone, now);
    }
}

#[test]
fn empty_schedule_stays_idle_and_mode_switch_keeps_manual_pause() {
    let mut app = simple_timer();
    app.dispatch(Action::TogglePause, Duration::ZERO);
    let mut settings = app.settings().clone();
    settings.use_schedule = true;
    let saved = app.dispatch(Action::SaveSettings(settings), Duration::ZERO);
    assert!(saved.update_windows);
    assert_eq!(basic(app.info()), (Mode::Idle, 0, true));
    app.dispatch(Action::Reset, Duration::ZERO);
    app.dispatch(Action::StartBreak, Duration::ZERO);
    app.dispatch(Action::Postpone, Duration::ZERO);
    app.dispatch(Action::EndBreak, Duration::ZERO);
    app.dispatch(Action::ApplyInterval(3), Duration::ZERO);
    app.dispatch(Action::TogglePause, Duration::ZERO);
    let tick = app.dispatch(
        Action::Tick {
            elapsed_secs: 1000,
            idle_secs: 1000,
        },
        Duration::from_secs(1000),
    );
    assert_eq!(app.info().mode, Mode::Idle);
    assert!(tick.events.is_empty());
    app.dispatch(Action::TogglePause, Duration::from_secs(1000));
    let mut settings = app.settings().clone();
    settings.use_schedule = false;
    app.dispatch(Action::SaveSettings(settings), Duration::from_secs(1000));
    assert_eq!(basic(app.info()), (Mode::Work, 180, true));
}

#[test]
fn switching_to_schedule_closes_reminder_but_allows_started_break_to_finish() {
    let mut app = simple_timer();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    let mut settings = app.settings().clone();
    settings.use_schedule = true;
    let saved = app.dispatch(Action::SaveSettings(settings), Duration::from_secs(60));
    assert_eq!(app.info().mode, Mode::Idle);
    assert!(saved.update_windows);
    assert!(saved.events.is_empty());

    let mut settings = app.settings().clone();
    settings.use_schedule = false;
    app.dispatch(Action::SaveSettings(settings), Duration::from_secs(60));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(120),
    );
    app.dispatch(Action::StartBreak, Duration::from_secs(120));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 20,
            idle_secs: 0,
        },
        Duration::from_secs(140),
    );
    app.dispatch(Action::TogglePause, Duration::from_secs(140));
    let mut settings = app.settings().clone();
    settings.use_schedule = true;
    settings.break_minutes = 5;
    app.dispatch(Action::SaveSettings(settings), Duration::from_secs(140));
    assert_eq!(basic(app.info()), (Mode::Break, 40, true));
    app.dispatch(Action::TogglePause, Duration::from_secs(140));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 39,
            idle_secs: 0,
        },
        Duration::from_secs(179),
    );
    assert_eq!(app.info().remaining, 1);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        Duration::from_secs(180),
    );
    assert_eq!(app.info().mode, Mode::Idle);
}

#[test]
fn saving_shared_settings_and_shortcuts_retains_both_modes_after_reload() {
    let path = std::env::temp_dir().join(format!(
        "sit-break-schedule-settings-{}.json",
        fastrand::u64(..)
    ));
    let schedule = serde_json::json!({"id":"saved-plan","name":"晨间安排","enabled":true,"repeat":{"kind":"once","date":"2026-10-14"},"periods":[]});
    let mut app = simple_timer();
    let mut settings = app.settings().clone();
    settings.use_schedule = true;
    settings.schedules = serde_json::from_value(serde_json::json!([schedule.clone()])).unwrap();
    app.dispatch(Action::SaveSettings(settings), Duration::ZERO);
    app.dispatch(Action::ApplyInterval(30), Duration::ZERO);
    app.dispatch(Action::ApplyBreak(3), Duration::ZERO);
    let mut settings = app.settings().clone();
    settings.language = "en".into();
    settings.sound = false;
    let saved = app.dispatch(Action::SaveSettings(settings), Duration::ZERO);
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut reloaded = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert_eq!(reloaded.info().mode, Mode::Idle);
    assert_eq!(
        serde_json::to_value(&reloaded.settings().schedules).unwrap(),
        serde_json::json!([schedule.clone()])
    );
    assert_eq!(reloaded.settings().break_minutes, 3);
    assert_eq!(reloaded.settings().language, "en");
    assert!(!reloaded.settings().sound);
    let mut settings = reloaded.settings().clone();
    settings.use_schedule = false;
    let saved = reloaded.dispatch(Action::SaveSettings(settings), Duration::ZERO);
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let reloaded = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert_eq!(reloaded.info().mode, Mode::Work);
    assert_eq!(reloaded.info().remaining, 1800);
    assert_eq!(
        serde_json::to_value(&reloaded.settings().schedules).unwrap(),
        serde_json::json!([schedule])
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn manual_pause_freezes_time_and_reset_preserves_pause() {
    let mut app = simple_timer();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 10,
            idle_secs: 0,
        },
        Duration::from_secs(10),
    );
    app.dispatch(Action::TogglePause, Duration::from_secs(10));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 90,
            idle_secs: 600,
        },
        Duration::from_secs(100),
    );
    assert_eq!(basic(app.info()), (Mode::Work, 50, true));
    let reset = app.dispatch(Action::Reset, Duration::from_secs(100));
    assert_eq!(reset.state.map(basic), Some((Mode::Work, 60, true)));
    app.dispatch(Action::TogglePause, Duration::from_secs(100));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(160),
    );
    app.dispatch(Action::StartBreak, Duration::from_secs(160));
    app.dispatch(Action::TogglePause, Duration::from_secs(160));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 30,
            idle_secs: 0,
        },
        Duration::from_secs(190),
    );
    assert_eq!(basic(app.info()), (Mode::Break, 60, true));
    app.dispatch(Action::Reset, Duration::from_secs(190));
    assert_eq!(basic(app.info()), (Mode::Work, 60, true));
}

#[test]
fn postponing_is_bounded_and_ending_a_break_starts_full_work() {
    let mut app = simple_timer();
    app.dispatch(Action::StartBreak, Duration::ZERO);
    app.dispatch(Action::Postpone, Duration::ZERO);
    app.dispatch(Action::EndBreak, Duration::ZERO);
    assert_eq!(app.info().mode, Mode::Work);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    let output = app.dispatch(Action::Postpone, Duration::from_secs(60));
    assert_eq!(output.state.map(basic), Some((Mode::Work, 60, false)));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(120),
    );
    app.dispatch(Action::StartBreak, Duration::from_secs(120));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 20,
            idle_secs: 0,
        },
        Duration::from_secs(140),
    );
    app.dispatch(Action::EndBreak, Duration::from_secs(140));
    assert_eq!(basic(app.info()), (Mode::Work, 60, false));

    let mut app = TimerApp::new(
        Settings {
            work_minutes: 8,
            idle_reset: false,
            ..Settings::default()
        },
        fastrand::Rng::with_seed(42),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 480,
            idle_secs: 0,
        },
        Duration::from_secs(480),
    );
    app.dispatch(Action::Postpone, Duration::from_secs(480));
    assert_eq!(app.info().remaining, 300);
}

#[test]
fn idle_resets_work_or_finishes_reminder_with_break_length_grace() {
    let mut app = TimerApp::new(
        Settings {
            work_minutes: 2,
            break_minutes: 1,
            ..Settings::default()
        },
        fastrand::Rng::with_seed(42),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 30,
            idle_secs: 0,
        },
        Duration::from_secs(30),
    );
    assert_eq!(app.info().remaining, 90);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 60,
        },
        Duration::from_secs(31),
    );
    assert_eq!(app.info().remaining, 120);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 120,
            idle_secs: 0,
        },
        Duration::from_secs(151),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 60,
        },
        Duration::from_secs(152),
    );
    assert_eq!(basic(app.info()), (Mode::Work, 120, false));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 60,
        },
        Duration::from_secs(153),
    );
    assert_eq!(app.info().remaining, 119);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 60,
        },
        Duration::from_secs(212),
    );
    assert_eq!(app.info().remaining, 120);

    let mut disabled = simple_timer();
    disabled.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 600,
        },
        Duration::from_secs(60),
    );
    disabled.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 600,
        },
        Duration::from_secs(120),
    );
    assert_eq!(disabled.info().mode, Mode::Remind);
}

#[test]
fn global_settings_and_shortcuts_preserve_active_break_and_refresh_next_work() {
    let mut app = simple_timer();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 10,
            idle_secs: 0,
        },
        Duration::from_secs(10),
    );
    let mut settings = app.settings().clone();
    settings.sound = false;
    let saved = app.dispatch(Action::SaveSettings(settings), Duration::from_secs(10));
    assert_eq!(app.info().remaining, 50);
    assert!(saved.settings_changed);
    assert!(!saved.persist_settings.unwrap().sound);
    app.dispatch(Action::ApplyInterval(3), Duration::from_secs(10));
    assert_eq!(app.info().remaining, 180);
    app.dispatch(Action::ApplyBreak(2), Duration::from_secs(10));
    let remind = app.dispatch(
        Action::Tick {
            elapsed_secs: 180,
            idle_secs: 0,
        },
        Duration::from_secs(190),
    );
    assert!(remind.events.iter().any(|event| matches!(event, TimerEvent::Reminder(payload) if payload.break_seconds == 120 && !payload.sound)));
    app.dispatch(Action::StartBreak, Duration::from_secs(190));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 20,
            idle_secs: 0,
        },
        Duration::from_secs(210),
    );
    app.dispatch(Action::ApplyBreak(3), Duration::from_secs(210));
    app.dispatch(Action::ApplyInterval(4), Duration::from_secs(210));
    let mut settings = app.settings().clone();
    settings.work_minutes = 5;
    app.dispatch(Action::SaveSettings(settings), Duration::from_secs(210));
    assert_eq!(basic(app.info()), (Mode::Break, 100, false));
    app.dispatch(Action::EndBreak, Duration::from_secs(210));
    assert_eq!(basic(app.info()), (Mode::Work, 300, false));
}

#[test]
fn simple_cycle_waits_for_user_then_returns_to_work_after_full_break() {
    let mut app = simple_timer();
    assert_eq!(basic(app.info()), (Mode::Work, 60, false));
    let output = app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    assert_eq!(app.info().mode, Mode::Remind);
    assert_eq!(output.state, Some(app.info()));
    assert!(output.events.iter().any(|event| matches!(event, TimerEvent::Reminder(payload) if payload.tip == "走动一下" && payload.break_seconds == 60 && payload.postpone_seconds == 60 && payload.sound)));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 0,
        },
        Duration::from_secs(660),
    );
    assert_eq!(app.info().mode, Mode::Remind);
    let output = app.dispatch(Action::StartBreak, Duration::from_secs(660));
    assert_eq!(output.state.map(basic), Some((Mode::Break, 60, false)));
    assert!(output
        .events
        .contains(&TimerEvent::BreakStarted { seconds: 60 }));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 59,
            idle_secs: 0,
        },
        Duration::from_secs(719),
    );
    assert_eq!(app.info().remaining, 1);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        Duration::from_secs(720),
    );
    assert_eq!(basic(app.info()), (Mode::Work, 60, false));
}

#[test]
fn a_once_schedule_with_stable_period_identity_survives_real_save_and_reload() {
    let path = std::env::temp_dir().join(format!("sit-break-once-{}.json", fastrand::u64(..)));
    let mut app = simple_timer();
    let mut settings = app.settings().clone();
    settings.use_schedule = true;
    settings.schedules = serde_json::from_value(serde_json::json!([{
        "id":"morning", "name":"晨间安排", "enabled":true,
        "repeat":{"kind":"once","date":"2026-10-14"},
        "periods":[{"id":"morning-period","start":"09:00","end":"12:00","work_minutes":50,"break_minutes":10}]
    }])).unwrap();
    let saved = app.dispatch(Action::SaveSettings(settings), Duration::ZERO);
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let reloaded = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert_eq!(
        reloaded.settings().schedules[0].periods[0].id,
        "morning-period"
    );
    assert_eq!(reloaded.settings().schedules[0].name, "晨间安排");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn invalid_schedule_save_reports_its_field_without_changing_settings_or_progress() {
    let mut app = simple_timer();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 12,
            idle_secs: 0,
        },
        Duration::from_secs(12),
    );
    let before = app.info();
    let before_settings = serde_json::to_value(app.settings()).unwrap();
    let mut invalid = app.settings().clone();
    invalid.autostart = true;
    invalid.use_schedule = true;
    invalid.schedules = serde_json::from_value(serde_json::json!([{
        "id":"morning", "name":"晨间安排", "enabled":true,
        "repeat":{"kind":"once","date":"2026-10-14"},
        "periods":[{"id":"morning-period","start":"09:00","end":"08:00","work_minutes":50,"break_minutes":10}]
    }])).unwrap();
    let result = app.dispatch(Action::SaveSettings(invalid), Duration::from_secs(12));
    assert_eq!(
        result.validation_errors[0].schedule_id.as_deref(),
        Some("morning")
    );
    assert_eq!(
        result.validation_errors[0].period_id.as_deref(),
        Some("morning-period")
    );
    assert_eq!(result.validation_errors[0].field, "end");
    assert!(result.persist_settings.is_none());
    assert!(!result.settings_changed);
    assert!(!result.update_windows);
    assert_eq!(app.info(), before);
    assert_eq!(
        serde_json::to_value(app.settings()).unwrap(),
        before_settings
    );
}

fn scheduled_settings(end: &str) -> Settings {
    let mut settings = Settings {
        use_schedule: true,
        idle_reset: false,
        tips: vec!["走动一下".into(), "喝水".into()],
        ..Settings::default()
    };
    settings.schedules = serde_json::from_value(serde_json::json!([{
        "id":"morning", "name":"晨间安排", "enabled":true,
        "repeat":{"kind":"once","date":"2026-10-14"},
        "periods":[{"id":"morning-period","start":"09:00","end":end,"work_minutes":50,"break_minutes":10}]
    }])).unwrap();
    settings
}

fn at(hour: u32, minute: u32) -> crate::timer::TimerTime {
    crate::timer::TimerTime {
        monotonic: Duration::from_secs(u64::from(hour * 3600 + minute * 60)),
        local: Some(crate::timer::LocalTime {
            date: "2026-10-14".into(),
            seconds_of_day: hour * 3600 + minute * 60,
        }),
    }
}

#[test]
fn a_late_started_break_gets_ten_full_minutes_then_starts_round_two() {
    let mut app = TimerApp::new_at(
        scheduled_settings("12:00"),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    assert_eq!(app.info().remaining, 3000);
    assert_eq!(app.info().work_seconds, 3000);
    assert_eq!(app.info().break_seconds, 600);
    assert_eq!(app.info().round.unwrap().number, 1);
    let reminder = app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(9, 50),
    );
    assert_eq!(app.info().mode, Mode::Remind);
    assert!(reminder.events.iter().any(|event| matches!(event, TimerEvent::Reminder(payload) if payload.break_seconds == 600 && payload.sat_minutes == 50)));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at(9, 55),
    );
    app.dispatch(Action::StartBreak, at(9, 55));
    assert_eq!(app.info().remaining, 600);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 599,
            idle_secs: 0,
        },
        at(10, 4),
    );
    assert_eq!(app.info().mode, Mode::Break);
    assert_eq!(app.info().remaining, 1);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        at(10, 5),
    );
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 3000);
    assert_eq!(app.info().round.unwrap().number, 2);
}

#[test]
fn schedule_work_starts_at_the_first_round_only_inside_its_local_date_and_period() {
    let before = TimerApp::new_at(
        scheduled_settings("12:00"),
        fastrand::Rng::with_seed(42),
        at(8, 59),
    );
    assert_eq!(before.info().mode, Mode::Idle);
    let mut midway = TimerApp::new_at(
        scheduled_settings("12:00"),
        fastrand::Rng::with_seed(42),
        at(10, 30),
    );
    assert_eq!(midway.info().remaining, 3000);
    assert_eq!(midway.info().round.unwrap().number, 1);
    midway.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(12, 0),
    );
    assert_eq!(midway.info().mode, Mode::Idle);
    let mut next_day = at(10, 0);
    next_day.local.as_mut().unwrap().date = "2026-10-15".into();
    let expired = TimerApp::new_at(
        scheduled_settings("12:00"),
        fastrand::Rng::with_seed(42),
        next_day,
    );
    assert_eq!(expired.info().mode, Mode::Idle);
}

#[test]
fn repeated_postpones_preserve_the_round_tip_and_actual_lengths() {
    let video = serde_json::json!({"kind":"video","source":{"type":"builtin","id":"squat-demo"}});
    let mut settings = serde_json::to_value(scheduled_settings("12:00")).unwrap();
    settings["tips"] = serde_json::json!([{"id":"global-video", "text":"", "guidance":video}]);
    let mut app = TimerApp::new_at(
        serde_json::from_value(settings).unwrap(),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    let tip = app.info().tip.unwrap();
    assert_eq!(tip, "");
    assert_eq!(serde_json::to_value(app.info()).unwrap()["guidance"], video);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(9, 50),
    );
    for minute in [50, 55] {
        app.dispatch(Action::Postpone, at(9, minute));
        assert_eq!(app.info().remaining, 300);
        let output = app.dispatch(
            Action::Tick {
                elapsed_secs: 300,
                idle_secs: 0,
            },
            if minute == 50 { at(9, 55) } else { at(10, 0) },
        );
        assert_eq!(app.info().round.unwrap().number, 1);
        assert_eq!(app.info().tip.as_deref(), Some(tip.as_str()));
        assert_eq!(serde_json::to_value(app.info()).unwrap()["guidance"], video);
        assert!(output.events.iter().any(|event| matches!(event, TimerEvent::Reminder(payload) if payload.tip == tip && payload.break_seconds == 600)));
    }
    app.dispatch(Action::StartBreak, at(10, 0));
    assert_eq!(app.info().tip.as_deref(), Some(tip.as_str()));
    assert_eq!(serde_json::to_value(app.info()).unwrap()["guidance"], video);
    assert_eq!(app.info().remaining, 600);
    let serialized = serde_json::to_value(app.info()).unwrap();
    assert_eq!(serialized["round"]["period_id"], "morning-period");
    assert_eq!(serialized["work_seconds"], 3000);
    assert_eq!(serialized["break_seconds"], 600);
}

#[test]
fn period_end_precedes_zero_work_and_closes_unstarted_reminders() {
    let mut app = TimerApp::new_at(
        scheduled_settings("09:50"),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    let output = app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(9, 50),
    );
    assert_eq!(app.info().mode, Mode::Idle);
    assert!(output.events.is_empty());
    assert!(output.update_windows);
    let mut app = TimerApp::new_at(
        scheduled_settings("09:55"),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(9, 50),
    );
    assert_eq!(app.info().mode, Mode::Remind);
    let output = app.dispatch(Action::StartBreak, at(9, 55));
    assert_eq!(app.info().mode, Mode::Idle);
    assert!(output.events.is_empty());
    assert!(output.update_windows);
}

#[test]
fn a_started_break_finishes_in_full_after_the_period_has_ended() {
    let mut app = TimerApp::new_at(
        scheduled_settings("10:00"),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(9, 50),
    );
    app.dispatch(Action::StartBreak, at(9, 55));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at(10, 0),
    );
    assert_eq!(app.info().mode, Mode::Break);
    assert_eq!(app.info().remaining, 300);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at(10, 5),
    );
    assert_eq!(app.info().mode, Mode::Idle);
    assert_eq!(app.info().remaining, 0);
}

#[test]
fn invalid_dates_times_and_lengths_preserve_the_last_saved_arrangement() {
    let path =
        std::env::temp_dir().join(format!("sit-break-validation-{}.json", fastrand::u64(..)));
    let valid = scheduled_settings("12:00");
    valid.save_to(&path).unwrap();
    let mut app = TimerApp::new_at(valid.clone(), fastrand::Rng::with_seed(42), at(9, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 1),
    );
    let progress = app.info();
    for (field, value) in [
        ("start", serde_json::json!("24:00")),
        ("end", serde_json::json!("25:00")),
        ("work_minutes", serde_json::json!(0)),
        ("work_minutes", serde_json::json!(241)),
        ("break_minutes", serde_json::json!(0)),
        ("break_minutes", serde_json::json!(61)),
    ] {
        let mut json = serde_json::to_value(&valid).unwrap();
        json["schedules"][0]["periods"][0][field] = value;
        let invalid: Settings = serde_json::from_value(json).unwrap();
        assert!(invalid.save_to(&path).is_err());
        let result = app.dispatch(Action::SaveSettings(invalid), at(12, 1));
        assert!(result
            .validation_errors
            .iter()
            .any(|error| error.field == field));
        assert!(result.persist_settings.is_none());
        assert_eq!(app.info(), progress);
        let reloaded = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
        assert_eq!(
            serde_json::to_value(reloaded.settings()).unwrap(),
            serde_json::to_value(&valid).unwrap()
        );
    }
    let mut invalid = valid.clone();
    invalid.schedules[0].repeat = crate::schedule::Repeat::Once {
        date: "2026-02-30".into(),
    };
    assert!(app
        .dispatch(Action::SaveSettings(invalid), at(9, 1))
        .validation_errors
        .iter()
        .any(|error| error.field == "date"));
    assert_eq!(app.info(), progress);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn day_end_is_supported_and_simple_shortcuts_do_not_change_schedule_progress() {
    let mut app = TimerApp::new_at(
        scheduled_settings("24:00"),
        fastrand::Rng::with_seed(42),
        at(23, 0),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(23, 1),
    );
    let progress = app.info();
    app.dispatch(Action::ApplyInterval(15), at(23, 1));
    app.dispatch(Action::ApplyBreak(3), at(23, 1));
    assert_eq!(app.info(), progress);
    assert_eq!(app.settings().work_minutes, 15);
    assert_eq!(app.settings().break_minutes, 3);
}

#[test]
fn entering_the_period_starts_full_work_and_short_work_bounds_every_postpone() {
    let mut settings = scheduled_settings("12:00");
    settings.schedules[0].periods[0].work_minutes = 1;
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(8, 59));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 0),
    );
    assert_eq!(app.info().remaining, 60);
    assert_eq!(app.info().round.unwrap().number, 1);
    for minute in 1..=3 {
        app.dispatch(
            Action::Tick {
                elapsed_secs: 60,
                idle_secs: 0,
            },
            at(9, minute),
        );
        app.dispatch(Action::Postpone, at(9, minute));
        assert_eq!(app.info().remaining, 60);
        assert_eq!(app.info().round.unwrap().number, 1);
    }
}

#[test]
fn simple_timer_keeps_its_existing_tip_rotation_when_postponed() {
    let mut app = TimerApp::new(
        Settings {
            work_minutes: 1,
            idle_reset: false,
            tips: vec!["走动".into(), "喝水".into()],
            ..Settings::default()
        },
        fastrand::Rng::with_seed(42),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(60),
    );
    let first_tip = app.info().tip;
    app.dispatch(Action::Postpone, Duration::from_secs(60));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        Duration::from_secs(120),
    );
    assert_ne!(app.info().tip, first_tip);
}

fn two_period_settings() -> Settings {
    let mut settings = scheduled_settings("12:00");
    settings.schedules[0].periods[0].work_minutes = 55;
    settings.schedules[0].periods[0].break_minutes = 5;
    settings.schedules[0].periods.push(serde_json::from_value(serde_json::json!({
        "id":"afternoon-period", "start":"13:30", "end":"18:00", "work_minutes":60, "break_minutes":5
    })).unwrap());
    settings
}

#[test]
fn overlapping_periods_are_located_and_rejected_without_changing_running_progress() {
    let mut app = TimerApp::new_at(
        two_period_settings(),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 1),
    );
    let progress = app.info();
    let saved = serde_json::to_value(app.settings()).unwrap();
    let path = std::env::temp_dir().join(format!("sit-break-overlap-{}.json", fastrand::u64(..)));
    app.settings().save_to(&path).unwrap();
    let mut draft = app.settings().clone();
    draft.schedules[0].periods[1].start = "11:30".into();
    assert!(draft.save_to(&path).is_err());
    let result = app.dispatch(Action::SaveSettings(draft), at(9, 1));
    assert!(result
        .validation_errors
        .iter()
        .any(|error| error.code == "overlap"
            && error.schedule_id.as_deref() == Some("morning")
            && error.period_id.as_deref() == Some("afternoon-period")));
    assert!(result.persist_settings.is_none());
    assert_eq!(app.info(), progress);
    assert_eq!(serde_json::to_value(app.settings()).unwrap(), saved);
    let reloaded = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert_eq!(serde_json::to_value(reloaded.settings()).unwrap(), saved);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn morning_and_afternoon_keep_lunch_quiet_and_start_independent_rounds() {
    let mut app = TimerApp::new_at(
        two_period_settings(),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    assert_eq!(app.info().work_seconds, 3300);
    assert_eq!(app.info().break_seconds, 300);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3300,
            idle_secs: 0,
        },
        at(9, 55),
    );
    app.dispatch(Action::StartBreak, at(9, 55));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at(10, 0),
    );
    assert_eq!(app.info().round.unwrap().number, 2);
    let noon = app.dispatch(
        Action::Tick {
            elapsed_secs: 7200,
            idle_secs: 0,
        },
        at(12, 0),
    );
    assert_eq!(app.info().mode, Mode::Idle);
    assert!(noon.events.is_empty());
    let lunch = app.dispatch(
        Action::Tick {
            elapsed_secs: 5340,
            idle_secs: 0,
        },
        at(13, 29),
    );
    assert_eq!(app.info().mode, Mode::Idle);
    assert!(lunch.events.is_empty());
    let afternoon = app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(13, 30),
    );
    assert!(afternoon.events.is_empty());
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 3600);
    assert_eq!(app.info().work_seconds, 3600);
    assert_eq!(app.info().break_seconds, 300);
    let round = app.info().round.unwrap();
    assert_eq!(round.period_id, "afternoon-period");
    assert_eq!(round.number, 1);
}

#[test]
fn a_long_break_crosses_lunch_and_next_period_without_starting_a_second_timer() {
    let mut settings = two_period_settings();
    settings.schedules[0].periods[0].break_minutes = 60;
    settings.schedules[0].periods[1].start = "12:30".into();
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(11, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3300,
            idle_secs: 0,
        },
        at(11, 55),
    );
    app.dispatch(Action::StartBreak, at(11, 55));
    let after_noon = app.dispatch(
        Action::Tick {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at(12, 0),
    );
    assert_eq!(app.info().mode, Mode::Break);
    assert_eq!(app.info().remaining, 3300);
    assert!(after_noon.events.is_empty());
    let during_afternoon = app.dispatch(
        Action::Tick {
            elapsed_secs: 1800,
            idle_secs: 0,
        },
        at(12, 30),
    );
    assert_eq!(app.info().mode, Mode::Break);
    assert_eq!(app.info().remaining, 1500);
    assert_eq!(app.info().round.unwrap().period_id, "morning-period");
    assert!(during_afternoon.events.is_empty());
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1500,
            idle_secs: 0,
        },
        at(12, 55),
    );
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 3600);
    let round = app.info().round.unwrap();
    assert_eq!(round.period_id, "afternoon-period");
    assert_eq!(round.number, 1);
}

#[test]
fn finishing_a_morning_break_in_lunch_stays_idle_until_afternoon() {
    let mut app = TimerApp::new_at(
        two_period_settings(),
        fastrand::Rng::with_seed(42),
        at(11, 3),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3300,
            idle_secs: 0,
        },
        at(11, 58),
    );
    app.dispatch(Action::StartBreak, at(11, 58));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at(12, 3),
    );
    assert_eq!(app.info().mode, Mode::Idle);
    assert!(app.info().round.is_none());
    app.dispatch(
        Action::Tick {
            elapsed_secs: 5220,
            idle_secs: 0,
        },
        at(13, 30),
    );
    assert_eq!(app.info().remaining, 3600);
    assert_eq!(app.info().round.unwrap().number, 1);
}

#[test]
fn adjacent_periods_are_allowed_and_boundary_selects_only_the_new_first_round() {
    let mut settings = two_period_settings();
    settings.schedules[0].periods[1].start = "12:00".into();
    let mut app = TimerApp::new_at(Settings::default(), fastrand::Rng::with_seed(42), at(9, 0));
    let saved = app.dispatch(Action::SaveSettings(settings), at(11, 50));
    assert!(saved.validation_errors.is_empty());
    let boundary = app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 0,
        },
        at(12, 0),
    );
    assert!(boundary.events.is_empty());
    assert_eq!(app.info().remaining, 3600);
    assert_eq!(app.info().round.unwrap().period_id, "afternoon-period");
    assert_eq!(app.info().round.unwrap().number, 1);
}

#[test]
fn jumping_over_a_whole_period_never_catches_up_its_work_or_reminders() {
    let mut app = TimerApp::new_at(
        two_period_settings(),
        fastrand::Rng::with_seed(42),
        at(8, 59),
    );
    let skipped_morning = app.dispatch(
        Action::Tick {
            elapsed_secs: 18060,
            idle_secs: 0,
        },
        at(14, 0),
    );
    assert!(skipped_morning.events.is_empty());
    assert_eq!(app.info().remaining, 3600);
    assert_eq!(app.info().round.unwrap().period_id, "afternoon-period");
    assert_eq!(app.info().round.unwrap().number, 1);
    let skipped_afternoon = app.dispatch(
        Action::Tick {
            elapsed_secs: 18000,
            idle_secs: 0,
        },
        at(19, 0),
    );
    assert!(skipped_afternoon.events.is_empty());
    assert_eq!(app.info().mode, Mode::Idle);
    let mut tomorrow = at(9, 0);
    tomorrow.local.as_mut().unwrap().date = "2026-10-15".into();
    let next_day = app.dispatch(
        Action::Tick {
            elapsed_secs: 50400,
            idle_secs: 0,
        },
        tomorrow,
    );
    assert!(next_day.events.is_empty());
    assert_eq!(app.info().mode, Mode::Idle);
}

#[test]
fn multiple_periods_keep_their_independent_lengths_and_identities_after_reload() {
    let path = std::env::temp_dir().join(format!("sit-break-periods-{}.json", fastrand::u64(..)));
    let mut app = simple_timer();
    let saved = app.dispatch(Action::SaveSettings(two_period_settings()), at(9, 0));
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let reloaded = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    let periods = &reloaded.settings().schedules[0].periods;
    assert_eq!(periods.len(), 2);
    assert_eq!(
        (
            &periods[0].id,
            periods[0].work_minutes,
            periods[0].break_minutes
        ),
        (&"morning-period".to_string(), 55, 5)
    );
    assert_eq!(
        (
            &periods[1].id,
            periods[1].work_minutes,
            periods[1].break_minutes
        ),
        (&"afternoon-period".to_string(), 60, 5)
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn overlap_reports_both_ranges_even_with_unsorted_nested_periods() {
    let mut settings = two_period_settings();
    settings.schedules[0].periods[1].start = "08:00".into();
    settings.schedules[0].periods[1].end = "20:00".into();
    settings.schedules[0].periods.reverse();
    let mut app = simple_timer();
    let rejected = app.dispatch(Action::SaveSettings(settings), at(9, 0));
    let error = rejected
        .validation_errors
        .iter()
        .find(|error| error.code == "overlap")
        .unwrap();
    assert_eq!(error.period_id.as_deref(), Some("morning-period"));
    let conflict = error.conflict.as_ref().unwrap();
    assert_eq!(conflict.period_id, "afternoon-period");
    assert_eq!(conflict.start, "08:00");
    assert_eq!(conflict.end, "20:00");
}

#[test]
fn a_day_end_period_is_valid_but_a_single_overnight_period_must_be_split() {
    let mut settings = two_period_settings();
    settings.schedules[0].periods[1].start = "22:00".into();
    settings.schedules[0].periods[1].end = "24:00".into();
    let mut app = simple_timer();
    let valid = app.dispatch(Action::SaveSettings(settings.clone()), at(23, 0));
    assert!(valid.validation_errors.is_empty());
    let progress = app.info();
    settings.schedules[0].periods[1].end = "01:00".into();
    let invalid = app.dispatch(Action::SaveSettings(settings), at(23, 0));
    assert!(invalid
        .validation_errors
        .iter()
        .any(|error| error.code == "end_before_start"
            && error.period_id.as_deref() == Some("afternoon-period")));
    assert_eq!(app.info(), progress);
    let mut tomorrow = at(0, 0);
    tomorrow.local.as_mut().unwrap().date = "2026-10-15".into();
    let boundary = app.dispatch(
        Action::Tick {
            elapsed_secs: 3600,
            idle_secs: 0,
        },
        tomorrow,
    );
    assert!(boundary.events.is_empty());
    assert_eq!(app.info().mode, Mode::Idle);
}

fn settings_with_overrides(overrides: serde_json::Value) -> Settings {
    let mut json = serde_json::to_value(scheduled_settings("15:00")).unwrap();
    let period = &mut json["schedules"][0]["periods"][0];
    period["work_minutes"] = serde_json::json!(55);
    period["break_minutes"] = serde_json::json!(5);
    period["overrides"] = overrides;
    serde_json::from_value(json).unwrap()
}

#[test]
fn scheduled_rounds_save_and_use_each_text_guidance_combination_without_mixing() {
    let video = serde_json::json!({"kind":"video","source":{"type":"builtin","id":"squat-demo"}});
    let mut draft = serde_json::to_value(settings_with_overrides(serde_json::json!([
        {"round":1,"content":"只读文字"},
        {"round":2,"guidance":video},
        {"round":3,"content":"下蹲，再喝水\n保持原文", "guidance":video},
        {"round":4,"content":"  "}
    ])))
    .unwrap();
    draft["tips"] = serde_json::json!([
        {"id":"global", "text":"全局文字", "guidance":video}
    ]);
    let mut app = TimerApp::new_at(Settings::default(), fastrand::Rng::with_seed(42), at(9, 0));
    let saved = app.dispatch(
        Action::SaveSettings(serde_json::from_value(draft).unwrap()),
        at(9, 0),
    );
    assert!(saved.validation_errors.is_empty());
    let path = std::env::temp_dir().join(format!(
        "sit-break-round-guidance-{}.json",
        fastrand::u64(..)
    ));
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut app = TimerApp::new_at(
        Settings::load_from(&path),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    for (number, text, guidance) in [
        (1, "只读文字", serde_json::Value::Null),
        (2, "", video.clone()),
        (3, "下蹲，再喝水\n保持原文", video.clone()),
        (4, "全局文字", video.clone()),
    ] {
        assert_eq!(app.info().round.unwrap().number, number);
        assert_eq!(app.info().tip.as_deref(), Some(text));
        assert_eq!(
            serde_json::to_value(app.info()).unwrap()["guidance"],
            guidance
        );
        let reminder = app.dispatch(
            Action::Tick {
                elapsed_secs: 3300,
                idle_secs: 0,
            },
            at(number + 8, 55),
        );
        assert!(
            matches!(reminder.events.first(), Some(TimerEvent::Reminder(payload))
            if payload.tip == text && serde_json::to_value(&payload.guidance).unwrap() == guidance)
        );
        app.dispatch(Action::StartBreak, at(number + 8, 55));
        assert_eq!(app.info().tip.as_deref(), Some(text));
        assert_eq!(
            serde_json::to_value(app.info()).unwrap()["guidance"],
            guidance
        );
        app.dispatch(Action::EndBreak, at(number + 9, 0));
    }
    let mut empty = app.settings().clone();
    empty.tips.clear();
    empty.schedules[0].periods[0].overrides.clear();
    let fallback = TimerApp::new_at(empty, fastrand::Rng::with_seed(42), at(9, 0));
    assert!(fallback.info().tip.is_some_and(|text| !text.is_empty()));
    assert!(fallback.info().guidance.is_none());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_first_round_length_override_leaves_later_rounds_at_the_default_rhythm() {
    let mut app = TimerApp::new_at(
        settings_with_overrides(serde_json::json!([
            {"round":1,"work_minutes":50,"break_minutes":10},
            {"round":2,"content":"深蹲"}
        ])),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    assert_eq!(
        (app.info().work_seconds, app.info().break_seconds),
        (3000, 600)
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(9, 50),
    );
    app.dispatch(Action::StartBreak, at(9, 55));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 0,
        },
        at(10, 5),
    );
    assert_eq!(app.info().round.unwrap().number, 2);
    assert_eq!(
        (app.info().work_seconds, app.info().break_seconds),
        (3300, 300)
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3300,
            idle_secs: 0,
        },
        at(11, 0),
    );
    app.dispatch(Action::StartBreak, at(11, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at(11, 5),
    );
    assert_eq!(app.info().round.unwrap().number, 3);
    assert_eq!(
        (app.info().work_seconds, app.info().break_seconds),
        (3300, 300)
    );
}

#[test]
fn custom_round_content_survives_save_language_change_postpones_and_one_full_break() {
    let content = "  爬楼 <script>alert('x')</script> & 深蹲\n补水 ✅\n  ";
    let path = std::env::temp_dir().join(format!("sit-break-overrides-{}.json", fastrand::u64(..)));
    let settings = settings_with_overrides(serde_json::json!([
        {"round":1,"work_minutes":50,"break_minutes":10,"content":content},
        {"round":2,"content":"深蹲"}
    ]));
    let mut saving = simple_timer();
    let saved = saving.dispatch(Action::SaveSettings(settings), at(8, 59));
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut app = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 0,
            idle_secs: 0,
        },
        at(9, 0),
    );
    assert_eq!(app.info().tip.as_deref(), Some(content));
    let reminder = app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(9, 50),
    );
    assert!(reminder.events.iter().any(|event| matches!(event, TimerEvent::Reminder(payload) if payload.tip == content && payload.break_seconds == 600)));
    let mut settings = app.settings().clone();
    settings.language = "en".into();
    app.dispatch(Action::SaveSettings(settings), at(9, 50));
    for (from, to) in [(at(9, 50), at(9, 55)), (at(9, 55), at(10, 0))] {
        app.dispatch(Action::Postpone, from);
        let reminder = app.dispatch(
            Action::Tick {
                elapsed_secs: 300,
                idle_secs: 0,
            },
            to,
        );
        assert_eq!(app.info().round.unwrap().number, 1);
        assert_eq!(app.info().tip.as_deref(), Some(content));
        assert!(reminder
            .events
            .iter()
            .any(|event| matches!(event, TimerEvent::Reminder(payload) if payload.tip == content)));
    }
    app.dispatch(Action::StartBreak, at(10, 0));
    assert_eq!(app.info().remaining, 600);
    assert_eq!(app.info().tip.as_deref(), Some(content));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 599,
            idle_secs: 0,
        },
        at(10, 9),
    );
    assert_eq!((app.info().mode, app.info().remaining), (Mode::Break, 1));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        at(10, 10),
    );
    assert_eq!(app.info().round.unwrap().number, 2);
    assert_eq!(app.info().tip.as_deref(), Some("深蹲"));
    assert_eq!(
        (app.info().work_seconds, app.info().break_seconds),
        (3300, 300)
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn invalid_round_adjustments_report_their_field_and_preserve_saved_progress() {
    let path = std::env::temp_dir().join(format!(
        "sit-break-override-validation-{}.json",
        fastrand::u64(..)
    ));
    let valid = settings_with_overrides(
        serde_json::json!([{"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}]),
    );
    valid.save_to(&path).unwrap();
    let mut app = TimerApp::new_at(valid.clone(), fastrand::Rng::with_seed(42), at(9, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 1),
    );
    let progress = app.info();
    for (overrides, field, code) in [
        (
            serde_json::json!([{"round":0}]),
            "overrides.0.round",
            "round_number",
        ),
        (
            serde_json::json!([{"round":1},{"round":1}]),
            "overrides.1.round",
            "duplicate_round",
        ),
        (
            serde_json::json!([{"round":1,"work_minutes":0}]),
            "overrides.0.work_minutes",
            "work_range",
        ),
        (
            serde_json::json!([{"round":1,"work_minutes":241}]),
            "overrides.0.work_minutes",
            "work_range",
        ),
        (
            serde_json::json!([{"round":1,"break_minutes":0}]),
            "overrides.0.break_minutes",
            "break_range",
        ),
        (
            serde_json::json!([{"round":1,"break_minutes":61}]),
            "overrides.0.break_minutes",
            "break_range",
        ),
    ] {
        let invalid = settings_with_overrides(overrides);
        let output = app.dispatch(Action::SaveSettings(invalid.clone()), at(15, 1));
        assert!(output
            .validation_errors
            .iter()
            .any(|error| error.period_id.as_deref() == Some("morning-period")
                && error.field == field
                && error.code == code));
        assert!(output.persist_settings.is_none());
        assert_eq!(app.info(), progress);
        assert!(invalid.save_to(&path).is_err());
        let loaded = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
        assert_eq!(
            serde_json::to_value(loaded.settings()).unwrap(),
            serde_json::to_value(&valid).unwrap()
        );
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn removing_an_adjustment_restores_inherited_lengths_and_a_stable_default_tip() {
    let path = std::env::temp_dir().join(format!(
        "sit-break-removed-override-{}.json",
        fastrand::u64(..)
    ));
    let settings = settings_with_overrides(serde_json::json!([
        {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}
    ]));
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
    let mut settings = app.settings().clone();
    settings.schedules[0].periods[0].overrides.clear();
    let saved = app.dispatch(Action::SaveSettings(settings), at(9, 10));
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut reloaded = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    reloaded.dispatch(
        Action::Tick {
            elapsed_secs: 0,
            idle_secs: 0,
        },
        at(9, 0),
    );
    assert_eq!(
        (reloaded.info().work_seconds, reloaded.info().break_seconds),
        (3300, 300)
    );
    let tip = reloaded.info().tip.unwrap();
    assert!(matches!(tip.as_str(), "走动一下" | "喝水"));
    reloaded.dispatch(
        Action::Tick {
            elapsed_secs: 3300,
            idle_secs: 0,
        },
        at(9, 55),
    );
    reloaded.dispatch(Action::Postpone, at(9, 55));
    reloaded.dispatch(
        Action::Tick {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at(10, 0),
    );
    reloaded.dispatch(Action::StartBreak, at(10, 0));
    assert_eq!(reloaded.info().tip.as_deref(), Some(tip.as_str()));
    assert_eq!(reloaded.info().remaining, 300);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn individual_duration_overrides_inherit_the_other_length_and_blank_content_uses_tips() {
    for (adjustment, work, rest) in [
        (serde_json::json!({"round":1,"work_minutes":50}), 3000, 300),
        (
            serde_json::json!({"round":1,"break_minutes":10,"content":" \n "}),
            3300,
            600,
        ),
    ] {
        let mut app = TimerApp::new_at(
            settings_with_overrides(serde_json::json!([adjustment])),
            fastrand::Rng::with_seed(42),
            at(9, 0),
        );
        assert_eq!(
            (app.info().work_seconds, app.info().break_seconds),
            (work, rest)
        );
        let tip = app.info().tip.unwrap();
        assert!(matches!(tip.as_str(), "走动一下" | "喝水"));
        app.dispatch(
            Action::Tick {
                elapsed_secs: work,
                idle_secs: 0,
            },
            at(9, 55),
        );
        app.dispatch(Action::Postpone, at(9, 55));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 300,
                idle_secs: 0,
            },
            at(10, 0),
        );
        assert_eq!(app.info().tip.as_deref(), Some(tip.as_str()));
        app.dispatch(Action::StartBreak, at(10, 0));
        assert_eq!(app.info().remaining, rest);
    }
}

#[test]
fn each_period_uses_its_own_first_round_adjustment() {
    let mut json = serde_json::to_value(settings_with_overrides(serde_json::json!([
        {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}
    ])))
    .unwrap();
    json["schedules"][0]["periods"][0]["end"] = serde_json::json!("10:00");
    json["schedules"][0]["periods"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id":"next-period","start":"10:00","end":"12:00","work_minutes":55,"break_minutes":5,
            "overrides":[{"round":1,"work_minutes":40,"break_minutes":3,"content":"伸展"}]
        }));
    let mut app = TimerApp::new_at(
        serde_json::from_value(json).unwrap(),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(9, 50),
    );
    app.dispatch(Action::StartBreak, at(9, 50));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 0,
        },
        at(10, 0),
    );
    let round = app.info().round.unwrap();
    assert_eq!((round.period_id.as_str(), round.number), ("next-period", 1));
    assert_eq!(
        (app.info().work_seconds, app.info().break_seconds),
        (2400, 180)
    );
    assert_eq!(app.info().tip.as_deref(), Some("伸展"));
}

#[test]
fn resetting_a_paused_break_in_lunch_returns_to_paused_idle() {
    let mut app = TimerApp::new_at(
        two_period_settings(),
        fastrand::Rng::with_seed(42),
        at(11, 3),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3300,
            idle_secs: 0,
        },
        at(11, 58),
    );
    app.dispatch(Action::StartBreak, at(11, 58));
    app.dispatch(Action::TogglePause, at(11, 58));
    let reset = app.dispatch(Action::Reset, at(12, 1));
    assert_eq!(app.info().mode, Mode::Idle);
    assert_eq!(app.info().remaining, 0);
    assert!(app.info().paused);
    assert!(app.info().round.is_none());
    assert!(reset.events.is_empty());
    assert!(reset.update_windows);
    assert_eq!(reset.state, Some(app.info()));
}

#[test]
fn resetting_a_paused_old_break_in_a_new_period_selects_its_first_round_snapshot() {
    let mut settings = two_period_settings();
    settings.schedules[0].periods[0].break_minutes = 60;
    settings.schedules[0].periods[1].start = "12:30".into();
    settings.schedules[0].periods[1].overrides = serde_json::from_value(serde_json::json!([
        {"round":1,"work_minutes":40,"break_minutes":3,"content":"下午伸展"}
    ]))
    .unwrap();
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(11, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3300,
            idle_secs: 0,
        },
        at(11, 55),
    );
    app.dispatch(Action::StartBreak, at(11, 55));
    app.dispatch(Action::TogglePause, at(11, 55));
    let reset = app.dispatch(Action::Reset, at(12, 30));
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 2400);
    assert_eq!(app.info().work_seconds, 2400);
    assert_eq!(app.info().break_seconds, 180);
    assert_eq!(app.info().tip.as_deref(), Some("下午伸展"));
    assert_eq!(app.info().round.unwrap().period_id, "afternoon-period");
    assert_eq!(app.info().round.unwrap().number, 1);
    assert!(app.info().paused);
    assert!(reset.events.is_empty());
    assert_eq!(reset.state, Some(app.info()));
}

#[test]
fn resetting_work_reminder_or_break_keeps_the_special_round_and_manual_pause() {
    for phase in [Mode::Work, Mode::Remind, Mode::Break] {
        let mut app = TimerApp::new_at(
            settings_with_overrides(serde_json::json!([
                {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}
            ])),
            fastrand::Rng::with_seed(42),
            at(9, 0),
        );
        let now = if phase == Mode::Work {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 600,
                    idle_secs: 0,
                },
                at(9, 10),
            );
            at(9, 10)
        } else {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 3000,
                    idle_secs: 0,
                },
                at(9, 50),
            );
            if phase == Mode::Break {
                app.dispatch(Action::StartBreak, at(9, 50));
                app.dispatch(
                    Action::Tick {
                        elapsed_secs: 60,
                        idle_secs: 0,
                    },
                    at(9, 51),
                );
                at(9, 51)
            } else {
                at(9, 50)
            }
        };
        assert_eq!(app.info().mode, phase);
        app.dispatch(Action::TogglePause, now.clone());
        let reset = app.dispatch(Action::Reset, now);
        assert_eq!(app.info().mode, Mode::Work);
        assert_eq!(app.info().remaining, 3000);
        assert_eq!(
            (app.info().work_seconds, app.info().break_seconds),
            (3000, 600)
        );
        assert_eq!(app.info().round.unwrap().number, 1);
        assert_eq!(app.info().tip.as_deref(), Some("爬楼"));
        assert!(app.info().paused);
        assert_eq!(reset.state, Some(app.info()));
        assert!(reset.events.is_empty());
        app.dispatch(
            Action::Tick {
                elapsed_secs: 600,
                idle_secs: 10000,
            },
            at(10, 0),
        );
        assert_eq!(app.info().remaining, 3000);
        assert_eq!(app.info().round.unwrap().number, 1);
    }
}

#[test]
fn manual_pause_freezes_schedule_countdown_and_idle_processing_until_resume() {
    for phase in [Mode::Work, Mode::Remind] {
        let mut settings = settings_with_overrides(serde_json::json!([
            {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}
        ]));
        settings.idle_reset = true;
        let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
        let elapsed = if phase == Mode::Work { 600 } else { 3000 };
        let minute = if phase == Mode::Work { 10 } else { 50 };
        app.dispatch(
            Action::Tick {
                elapsed_secs: elapsed,
                idle_secs: 0,
            },
            at(9, minute),
        );
        app.dispatch(Action::TogglePause, at(9, minute));
        let paused = app.info();
        let tick = app.dispatch(
            Action::Tick {
                elapsed_secs: 600,
                idle_secs: 10000,
            },
            at(10, 0),
        );
        assert_eq!(app.info(), paused);
        assert_eq!(tick.state, Some(paused));
        assert!(tick.events.is_empty());
        app.dispatch(Action::TogglePause, at(10, 0));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 60,
                idle_secs: 0,
            },
            at(10, 1),
        );
        assert!(!app.info().paused);
        assert_eq!(app.info().round.unwrap().number, 1);
        assert_eq!(app.info().mode, phase);
        assert_eq!(
            app.info().remaining,
            if phase == Mode::Work { 2340 } else { 0 }
        );
    }
}

#[test]
fn paused_work_and_reminders_expire_at_lunch_and_new_periods_remain_paused() {
    for phase in [Mode::Work, Mode::Remind] {
        let mut app = TimerApp::new_at(
            two_period_settings(),
            fastrand::Rng::with_seed(42),
            at(11, 0),
        );
        if phase == Mode::Remind {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 3300,
                    idle_secs: 0,
                },
                at(11, 55),
            );
        }
        app.dispatch(Action::TogglePause, at(11, 55));
        let boundary = app.dispatch(
            Action::Tick {
                elapsed_secs: 300,
                idle_secs: 10000,
            },
            at(12, 0),
        );
        assert_eq!(app.info().mode, Mode::Idle);
        assert!(app.info().paused);
        assert!(app.info().round.is_none());
        assert!(boundary.events.is_empty());
        assert!(boundary.update_windows);
        app.dispatch(Action::StartBreak, at(12, 0));
        app.dispatch(Action::Reset, at(12, 0));
        assert_eq!(app.info().mode, Mode::Idle);
        assert!(app.info().paused);
        app.dispatch(
            Action::Tick {
                elapsed_secs: 5400,
                idle_secs: 10000,
            },
            at(13, 30),
        );
        assert_eq!(app.info().mode, Mode::Work);
        assert_eq!(app.info().remaining, 3600);
        assert_eq!(app.info().round.unwrap().period_id, "afternoon-period");
        assert_eq!(app.info().round.unwrap().number, 1);
        assert!(app.info().paused);
        app.dispatch(
            Action::Tick {
                elapsed_secs: 1800,
                idle_secs: 10000,
            },
            at(14, 0),
        );
        assert_eq!(app.info().remaining, 3600);
        app.dispatch(Action::TogglePause, at(14, 0));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 60,
                idle_secs: 0,
            },
            at(14, 1),
        );
        assert_eq!(app.info().remaining, 3540);
        assert!(!app.info().paused);
    }
}

#[test]
fn paused_work_and_reminders_never_restore_after_their_once_date_expires() {
    for phase in [Mode::Work, Mode::Remind] {
        let mut settings = two_period_settings();
        settings.schedules[0].periods[1].end = "24:00".into();
        settings.schedules[0].periods[1].work_minutes = 30;
        let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(23, 0));
        if phase == Mode::Remind {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 1800,
                    idle_secs: 0,
                },
                at(23, 30),
            );
        }
        app.dispatch(Action::TogglePause, at(23, 30));
        let mut tomorrow = at(0, 0);
        tomorrow.monotonic = Duration::from_secs(24 * 3600);
        tomorrow.local.as_mut().unwrap().date = "2026-10-15".into();
        let boundary = app.dispatch(
            Action::Tick {
                elapsed_secs: 1800,
                idle_secs: 10000,
            },
            tomorrow.clone(),
        );
        assert_eq!(app.info().mode, Mode::Idle);
        assert!(app.info().paused);
        assert!(boundary.events.is_empty());
        app.dispatch(Action::TogglePause, tomorrow.clone());
        app.dispatch(Action::Reset, tomorrow);
        assert_eq!(app.info().mode, Mode::Idle);
        assert!(!app.info().paused);
        assert!(app.info().round.is_none());
    }
}

#[test]
fn a_paused_started_break_survives_boundaries_then_finishes_after_manual_resume() {
    let mut settings = two_period_settings();
    settings.schedules[0].periods[0].break_minutes = 60;
    settings.schedules[0].periods[1].start = "12:30".into();
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(11, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3300,
            idle_secs: 0,
        },
        at(11, 55),
    );
    app.dispatch(Action::StartBreak, at(11, 55));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(11, 56),
    );
    app.dispatch(Action::TogglePause, at(11, 56));
    let paused = app.info();
    let boundary = app.dispatch(
        Action::Tick {
            elapsed_secs: 2640,
            idle_secs: 10000,
        },
        at(12, 40),
    );
    assert_eq!(app.info(), paused);
    assert_eq!(app.info().remaining, 3540);
    assert!(boundary.events.is_empty());
    app.dispatch(Action::TogglePause, at(12, 40));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3540,
            idle_secs: 0,
        },
        at(13, 39),
    );
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 3600);
    assert_eq!(app.info().round.unwrap().period_id, "afternoon-period");
    assert_eq!(app.info().round.unwrap().number, 1);
    assert!(!app.info().paused);
}

#[test]
fn ending_a_break_consumes_its_round_and_preserves_pause_in_the_current_period() {
    for (target, paused) in [
        ((9, 55), false),
        ((12, 1), false),
        ((13, 30), false),
        ((9, 55), true),
        ((12, 1), true),
        ((13, 30), true),
    ] {
        let mut settings = settings_with_overrides(serde_json::json!([
            {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"},
            {"round":2,"content":"深蹲"}
        ]));
        settings.schedules[0].periods[0].end = "12:00".into();
        settings.schedules[0].periods.push(serde_json::from_value(serde_json::json!({
            "id":"afternoon-period","start":"13:30","end":"18:00","work_minutes":60,"break_minutes":5,
            "overrides":[{"round":1,"work_minutes":40,"break_minutes":3,"content":"下午伸展"}]
        })).unwrap());
        let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 3000,
                idle_secs: 0,
            },
            at(9, 50),
        );
        app.dispatch(Action::StartBreak, at(9, 50));
        if paused {
            app.dispatch(Action::TogglePause, at(9, 50));
        }
        let ended = app.dispatch(Action::EndBreak, at(target.0, target.1));
        assert_eq!(app.info().paused, paused);
        assert!(ended.events.is_empty());
        assert_eq!(ended.state, Some(app.info()));
        match target {
            (9, 55) => {
                assert_eq!(app.info().remaining, 3300);
                assert_eq!(app.info().round.unwrap().number, 2);
                assert_eq!(app.info().tip.as_deref(), Some("深蹲"));
            }
            (13, 30) => {
                assert_eq!(app.info().remaining, 2400);
                assert_eq!(app.info().round.unwrap().period_id, "afternoon-period");
                assert_eq!(app.info().round.unwrap().number, 1);
                assert_eq!(app.info().tip.as_deref(), Some("下午伸展"));
            }
            _ => {
                assert_eq!(app.info().mode, Mode::Idle);
                assert_eq!(app.info().remaining, 0);
                assert!(app.info().round.is_none());
            }
        }
    }
}

#[test]
fn mode_switch_reset_and_start_break_never_clear_a_manual_pause() {
    let mut app = TimerApp::new_at(
        settings_with_overrides(serde_json::json!([
            {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}
        ])),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    app.dispatch(Action::TogglePause, at(9, 0));
    let mut settings = app.settings().clone();
    settings.use_schedule = false;
    app.dispatch(Action::SaveSettings(settings), at(9, 0));
    assert!(app.info().paused);
    assert_eq!(app.info().remaining, 2700);
    app.dispatch(Action::Reset, at(9, 0));
    assert!(app.info().paused);
    let mut settings = app.settings().clone();
    settings.use_schedule = true;
    app.dispatch(Action::SaveSettings(settings), at(9, 0));
    assert!(app.info().paused);
    assert_eq!(app.info().remaining, 3000);
    app.dispatch(Action::TogglePause, at(9, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(9, 50),
    );
    app.dispatch(Action::TogglePause, at(9, 50));
    app.dispatch(Action::Postpone, at(9, 50));
    assert!(app.info().paused);
    assert_eq!(app.info().round.unwrap().number, 1);
    app.dispatch(Action::Reset, at(9, 50));
    assert!(app.info().paused);
    app.dispatch(Action::TogglePause, at(9, 50));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at(10, 40),
    );
    app.dispatch(Action::TogglePause, at(10, 40));
    app.dispatch(Action::StartBreak, at(10, 40));
    assert!(app.info().paused);
    assert_eq!(app.info().remaining, 600);
    assert_eq!(app.info().round.unwrap().number, 1);
}

#[test]
fn a_paused_break_keeps_its_remaining_time_across_midnight_and_finishes_when_resumed() {
    let mut settings = two_period_settings();
    settings.schedules[0].periods[1].end = "24:00".into();
    settings.schedules[0].periods[1].work_minutes = 30;
    settings.schedules[0].periods[1].break_minutes = 20;
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(23, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1800,
            idle_secs: 0,
        },
        at(23, 30),
    );
    app.dispatch(Action::StartBreak, at(23, 30));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at(23, 35),
    );
    app.dispatch(Action::TogglePause, at(23, 35));
    let paused = app.info();
    let mut tomorrow = at(0, 5);
    tomorrow.monotonic = Duration::from_secs(24 * 3600 + 300);
    tomorrow.local.as_mut().unwrap().date = "2026-10-15".into();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1800,
            idle_secs: 10000,
        },
        tomorrow.clone(),
    );
    assert_eq!(app.info(), paused);
    assert_eq!(app.info().remaining, 900);
    app.dispatch(Action::TogglePause, tomorrow.clone());
    tomorrow.local.as_mut().unwrap().seconds_of_day = 1200;
    tomorrow.monotonic = Duration::from_secs(24 * 3600 + 1200);
    let finished = app.dispatch(
        Action::Tick {
            elapsed_secs: 900,
            idle_secs: 0,
        },
        tomorrow,
    );
    assert_eq!(app.info().mode, Mode::Idle);
    assert!(!app.info().paused);
    assert!(finished.events.is_empty());
}

#[test]
fn one_continuous_away_period_cannot_skip_new_short_work_rounds() {
    let mut settings = settings_with_overrides(serde_json::json!([
        {"round":1,"content":"爬楼"}, {"round":2,"content":"深蹲"}
    ]));
    settings.idle_reset = true;
    settings.schedules[0].periods[0].work_minutes = 1;
    settings.schedules[0].periods[0].break_minutes = 10;
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 1),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 600,
        },
        at(9, 11),
    );
    assert_eq!(app.info().round.unwrap().number, 2);
    assert_eq!(app.info().remaining, 60);
    for minute in 12..=40 {
        let still_away = app.dispatch(
            Action::Tick {
                elapsed_secs: 60,
                idle_secs: (minute - 1) as u64 * 60,
            },
            at(9, minute),
        );
        assert!(still_away.events.is_empty());
        assert_eq!(app.info().mode, Mode::Work);
        assert_eq!(app.info().remaining, 60);
        assert_eq!(app.info().round.unwrap().number, 2);
        assert_eq!(app.info().tip.as_deref(), Some("深蹲"));
    }
}

#[test]
fn continuing_away_after_a_full_break_does_not_count_the_next_short_work_round() {
    let mut settings = settings_with_overrides(serde_json::json!([
        {"round":1,"content":"爬楼"}, {"round":2,"break_minutes":20,"content":"深蹲"}
    ]));
    settings.idle_reset = true;
    settings.schedules[0].periods[0].work_minutes = 1;
    settings.schedules[0].periods[0].break_minutes = 10;
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 1),
    );
    app.dispatch(Action::StartBreak, at(9, 1));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 600,
        },
        at(9, 11),
    );
    assert_eq!(app.info().round.unwrap().number, 2);
    assert_eq!(app.info().break_seconds, 1200);
    for minute in 12..=40 {
        let still_away = app.dispatch(
            Action::Tick {
                elapsed_secs: 60,
                idle_secs: (minute - 1) as u64 * 60,
            },
            at(9, minute),
        );
        assert!(still_away.events.is_empty());
        assert_eq!(app.info().mode, Mode::Work);
        assert_eq!(app.info().remaining, 60);
        assert_eq!(app.info().round.unwrap().number, 2);
    }
}

#[test]
fn away_work_uses_the_actual_override_rest_threshold_and_only_resets_its_round() {
    for (idle_secs, remaining) in [(599, 1800), (600, 3000), (601, 3000)] {
        let mut settings = settings_with_overrides(serde_json::json!([
            {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}
        ]));
        settings.idle_reset = true;
        let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 600,
                idle_secs: 0,
            },
            at(9, 10),
        );
        let away = app.dispatch(
            Action::Tick {
                elapsed_secs: 600,
                idle_secs,
            },
            at(9, 20),
        );
        assert_eq!(app.info().mode, Mode::Work);
        assert_eq!(app.info().remaining, remaining);
        assert_eq!(app.info().round.unwrap().number, 1);
        assert_eq!(app.info().tip.as_deref(), Some("爬楼"));
        assert_eq!(
            (app.info().work_seconds, app.info().break_seconds),
            (3000, 600)
        );
        assert!(away.events.is_empty());
        assert_eq!(away.state, Some(app.info()));
    }
}

#[test]
fn away_reminder_uses_the_actual_rest_threshold_before_finishing_exactly_one_round() {
    for (idle_secs, expected_round, expected_mode) in [
        (599, 1, Mode::Remind),
        (600, 2, Mode::Work),
        (601, 2, Mode::Work),
    ] {
        let mut settings = settings_with_overrides(serde_json::json!([
            {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"},
            {"round":2,"content":"深蹲"}
        ]));
        settings.idle_reset = true;
        let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 3000,
                idle_secs: 0,
            },
            at(9, 50),
        );
        let away = app.dispatch(
            Action::Tick {
                elapsed_secs: 600,
                idle_secs,
            },
            at(10, 0),
        );
        assert_eq!(app.info().mode, expected_mode);
        assert_eq!(app.info().round.unwrap().number, expected_round);
        assert_eq!(
            app.info().remaining,
            if expected_round == 1 { 0 } else { 3300 }
        );
        assert_eq!(
            app.info().tip.as_deref(),
            Some(if expected_round == 1 {
                "爬楼"
            } else {
                "深蹲"
            })
        );
        assert!(away.events.is_empty());
    }
}

#[test]
fn disabled_away_reset_and_manual_pause_do_not_process_idle_work_or_reminders() {
    for (enabled, paused) in [(false, false), (true, true)] {
        for phase in [Mode::Work, Mode::Remind] {
            let mut settings = settings_with_overrides(serde_json::json!([
                {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}
            ]));
            settings.idle_reset = enabled;
            let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
            let elapsed = if phase == Mode::Work { 600 } else { 3000 };
            let minute = if phase == Mode::Work { 10 } else { 50 };
            app.dispatch(
                Action::Tick {
                    elapsed_secs: elapsed,
                    idle_secs: 0,
                },
                at(9, minute),
            );
            if paused {
                app.dispatch(Action::TogglePause, at(9, minute));
            }
            let away = app.dispatch(
                Action::Tick {
                    elapsed_secs: 600,
                    idle_secs: 60000,
                },
                if phase == Mode::Work {
                    at(9, 20)
                } else {
                    at(10, 0)
                },
            );
            assert_eq!(app.info().mode, phase);
            assert_eq!(app.info().round.unwrap().number, 1);
            assert_eq!(
                app.info().remaining,
                match (phase, paused) {
                    (Mode::Work, true) => 2400,
                    (Mode::Work, false) => 1800,
                    _ => 0,
                }
            );
            assert_eq!(app.info().paused, paused);
            assert!(away.events.is_empty());
        }
    }
}

#[test]
fn returning_to_the_computer_allows_work_and_a_fresh_away_period() {
    let mut settings = settings_with_overrides(serde_json::json!([
        {"round":1,"content":"爬楼"}, {"round":2,"content":"深蹲"}
    ]));
    settings.idle_reset = true;
    settings.schedules[0].periods[0].work_minutes = 1;
    settings.schedules[0].periods[0].break_minutes = 10;
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 1),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 600,
        },
        at(9, 11),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 660,
        },
        at(9, 12),
    );
    assert_eq!(app.info().remaining, 60);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 0,
            idle_secs: 0,
        },
        at(9, 12),
    );
    assert_eq!(app.info().remaining, 60);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 13),
    );
    assert_eq!(app.info().mode, Mode::Remind);
    assert_eq!(app.info().round.unwrap().number, 2);
    let away_again = app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 600,
        },
        at(9, 23),
    );
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().round.unwrap().number, 3);
    assert!(away_again.events.is_empty());
}

#[test]
fn period_boundary_precedes_away_reminder_completion_for_idle_or_a_new_period() {
    for next_period in [false, true] {
        let mut settings = settings_with_overrides(serde_json::json!([
            {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}
        ]));
        settings.idle_reset = true;
        settings.schedules[0].periods[0].end = "10:00".into();
        if next_period {
            settings.schedules[0].periods.push(serde_json::from_value(serde_json::json!({
                "id":"next-period", "start":"10:00", "end":"12:00", "work_minutes":40, "break_minutes":3,
                "overrides":[{"round":1,"content":"下午伸展"}]
            })).unwrap());
        }
        let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 3000,
                idle_secs: 0,
            },
            at(9, 50),
        );
        let boundary = app.dispatch(
            Action::Tick {
                elapsed_secs: 600,
                idle_secs: 600,
            },
            at(10, 0),
        );
        assert!(boundary.events.is_empty());
        assert!(boundary.update_windows);
        if next_period {
            assert_eq!(app.info().mode, Mode::Work);
            assert_eq!(app.info().remaining, 2400);
            assert_eq!(app.info().round.unwrap().period_id, "next-period");
            assert_eq!(app.info().round.unwrap().number, 1);
        } else {
            assert_eq!(app.info().mode, Mode::Idle);
            assert!(app.info().round.is_none());
        }
    }
}

#[test]
fn a_disabled_away_switch_stays_disabled_even_at_the_idle_input_limit() {
    let settings = settings_with_overrides(serde_json::json!([
        {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"}
    ]));
    for phase in [Mode::Work, Mode::Remind] {
        let mut app = TimerApp::new_at(settings.clone(), fastrand::Rng::with_seed(42), at(9, 0));
        if phase == Mode::Remind {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 3000,
                    idle_secs: 0,
                },
                at(9, 50),
            );
        }
        let now = if phase == Mode::Work {
            at(9, 1)
        } else {
            at(9, 51)
        };
        app.dispatch(
            Action::Tick {
                elapsed_secs: 1,
                idle_secs: u64::MAX,
            },
            now,
        );
        assert_eq!(app.info().mode, phase);
        assert_eq!(
            app.info().remaining,
            if phase == Mode::Work { 2999 } else { 0 }
        );
        assert_eq!(app.info().round.unwrap().number, 1);
    }
}
