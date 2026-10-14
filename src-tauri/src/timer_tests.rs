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
    assert_eq!(settings["tips"], serde_json::json!(["Walk"]));
    std::fs::remove_file(path).unwrap();
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
    let mut app = TimerApp::new_at(
        scheduled_settings("12:00"),
        fastrand::Rng::with_seed(42),
        at(9, 0),
    );
    let tip = app.info().tip.unwrap();
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
        assert!(output.events.iter().any(|event| matches!(event, TimerEvent::Reminder(payload) if payload.tip == tip && payload.break_seconds == 600)));
    }
    app.dispatch(Action::StartBreak, at(10, 0));
    assert_eq!(app.info().tip.as_deref(), Some(tip.as_str()));
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

fn settings_with_overrides(overrides: serde_json::Value) -> Settings {
    let mut json = serde_json::to_value(scheduled_settings("15:00")).unwrap();
    let period = &mut json["schedules"][0]["periods"][0];
    period["work_minutes"] = serde_json::json!(55);
    period["break_minutes"] = serde_json::json!(5);
    period["overrides"] = overrides;
    serde_json::from_value(json).unwrap()
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
