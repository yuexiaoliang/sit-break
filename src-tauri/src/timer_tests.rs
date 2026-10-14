use std::time::Duration;

use crate::timer::{Action, Mode, TimerApp, TimerEvent, TimerInfo};
use crate::Settings;

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
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Idle,
            remaining: 0,
            paused: true
        }
    );
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
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Work,
            remaining: 180,
            paused: true
        }
    );
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
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Break,
            remaining: 40,
            paused: true
        }
    );
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
    settings.schedules = vec![schedule.clone()];
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
    assert_eq!(reloaded.settings().schedules, vec![schedule.clone()]);
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
    assert_eq!(reloaded.settings().schedules, vec![schedule]);
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
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Work,
            remaining: 50,
            paused: true
        }
    );
    let reset = app.dispatch(Action::Reset, Duration::from_secs(100));
    assert_eq!(
        reset.state,
        Some(TimerInfo {
            mode: Mode::Work,
            remaining: 60,
            paused: true
        })
    );
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
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Break,
            remaining: 60,
            paused: true
        }
    );
    app.dispatch(Action::Reset, Duration::from_secs(190));
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Work,
            remaining: 60,
            paused: true
        }
    );
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
    assert_eq!(
        output.state,
        Some(TimerInfo {
            mode: Mode::Work,
            remaining: 60,
            paused: false
        })
    );
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
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Work,
            remaining: 60,
            paused: false
        }
    );

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
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Work,
            remaining: 120,
            paused: false
        }
    );
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
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Break,
            remaining: 100,
            paused: false
        }
    );
    app.dispatch(Action::EndBreak, Duration::from_secs(210));
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Work,
            remaining: 300,
            paused: false
        }
    );
}

#[test]
fn simple_cycle_waits_for_user_then_returns_to_work_after_full_break() {
    let mut app = simple_timer();
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Work,
            remaining: 60,
            paused: false
        }
    );
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
    assert_eq!(
        output.state,
        Some(TimerInfo {
            mode: Mode::Break,
            remaining: 60,
            paused: false
        })
    );
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
    assert_eq!(
        app.info(),
        TimerInfo {
            mode: Mode::Work,
            remaining: 60,
            paused: false
        }
    );
}
