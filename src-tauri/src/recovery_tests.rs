use std::time::Duration;

use crate::timer::{Action, LocalTime, Mode, TimerApp, TimerTime};
use crate::Settings;

fn time(hour: u32, minute: u32, monotonic: u64) -> TimerTime {
    TimerTime {
        monotonic: Duration::from_secs(monotonic),
        local: Some(LocalTime {
            date: "2026-10-14".into(),
            seconds_of_day: hour * 3600 + minute * 60,
        }),
    }
}

fn settings() -> Settings {
    Settings {
        use_schedule: true,
        idle_reset: false,
        schedules: serde_json::from_value(serde_json::json!([{
            "id":"workdays", "name":"工作日", "enabled":true,
            "repeat":{"kind":"weekly","weekdays":[1,2,3,4,5]},
            "periods":[{"id":"morning","start":"09:00","end":"12:00","work_minutes":55,"break_minutes":5,"overrides":[{"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"},{"round":2,"content":"深蹲"}]},
            {"id":"afternoon","start":"13:30","end":"18:00","work_minutes":60,"break_minutes":5}]
        }])).unwrap(),
        ..Settings::default()
    }
}

fn paths() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("sit-break-recovery-{}", fastrand::u64(..)));
    (dir.join("settings.json"), dir.join("progress.json"), dir)
}

#[test]
fn reopening_skips_old_rounds_and_uses_the_current_work_time() {
    let (settings_path, progress_path, dir) = paths();
    let app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 35, 0));
    app.settings().save_to(&settings_path).unwrap();
    app.save_progress_to(&progress_path, time(9, 35, 0))
        .unwrap();
    assert_eq!(app.info().round.unwrap().number, 1);

    let mut reopened = TimerApp::restore_from(
        Settings::load_from(&settings_path),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(11, 10, 0),
    );
    assert_eq!(reopened.info().round.unwrap().number, 3);
    assert_eq!(reopened.info().remaining, 45 * 60);
    let output = reopened.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        time(11, 11, 1),
    );
    assert_eq!(reopened.info().remaining, 44 * 60);
    assert!(output.events.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn upgrading_old_progress_keeps_pause_but_cannot_restore_the_old_round() {
    let (_, progress_path, dir) = paths();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&progress_path, r#"{
        "version": 1,
        "saved_local": {"date": "2026-10-14", "seconds_of_day": 40320},
        "use_schedule": true,
        "info": {"mode": "break", "remaining": 300, "paused": true, "work_seconds": 3000, "break_seconds": 600,
          "round": {"schedule_id": "workdays", "period_id": "morning", "date": "2026-10-14", "number": 1},
          "tip": "旧轮次"},
        "restart_after_break": false,
        "last_idle_secs": 0
    }"#).unwrap();
    let mut reopened = TimerApp::restore_from(
        settings(),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(11, 13, 0),
    );
    assert_eq!(reopened.info().round.unwrap().number, 3);
    assert_eq!(reopened.info().mode, Mode::Work);
    assert_eq!(reopened.info().remaining, 42 * 60);
    assert!(reopened.info().paused);

    let expired = reopened.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        time(12, 0, 1),
    );
    assert_eq!(reopened.info().mode, Mode::Idle);
    assert!(reopened.info().paused);
    assert!(expired.events.is_empty());
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        time(13, 40, 2),
    );
    assert_eq!(reopened.info().round.unwrap().period_id, "afternoon");
    assert_eq!(reopened.info().remaining, 50 * 60);
    assert!(reopened.info().paused);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reopening_an_active_break_preserves_its_content_and_uses_the_current_deadline() {
    let (settings_path, progress_path, dir) = paths();
    let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 50, 0));
    app.dispatch(Action::StartBreak, time(9, 50, 0));
    app.save_progress_to(&progress_path, time(9, 52, 120))
        .unwrap();
    let mut edited = app.settings().clone();
    edited.schedules[0].periods[0].overrides[0].content = Some("新的提示".into());
    edited.save_to(&settings_path).unwrap();

    let reopened = TimerApp::restore_from(
        Settings::load_from(&settings_path),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(9, 56, 0),
    );
    assert_eq!(reopened.info().mode, Mode::Break);
    assert_eq!(reopened.info().remaining, 240);
    assert_eq!(reopened.info().tip.as_deref(), Some("爬楼"));
    assert!(reopened.reminder_visible());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn snooze_and_dismissal_only_survive_reopening_in_the_same_break_window() {
    let (_, progress_path, dir) = paths();
    let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 50, 0));
    app.dispatch(Action::Postpone, time(9, 50, 0));
    app.save_progress_to(&progress_path, time(9, 50, 0))
        .unwrap();
    let reopen = |h, m| {
        TimerApp::restore_from(
            settings(),
            &progress_path,
            fastrand::Rng::with_seed(42),
            time(h, m, 0),
        )
    };
    assert!(!reopen(9, 54).reminder_visible());
    assert!(reopen(9, 55).reminder_visible());

    let mut app = reopen(9, 55);
    app.dispatch(Action::StartBreak, time(9, 55, 0));
    app.dispatch(Action::EndBreak, time(9, 56, 60));
    app.save_progress_to(&progress_path, time(9, 56, 60))
        .unwrap();
    assert!(!reopen(9, 59).reminder_visible());
    let next = reopen(10, 56);
    assert!(next.reminder_visible());
    assert_eq!(next.info().round.unwrap().number, 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn corrupted_progress_does_not_inject_invalid_remaining_or_manual_pause() {
    let (_, progress_path, dir) = paths();
    let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 0, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        time(9, 1, 60),
    );
    app.save_progress_to(&progress_path, time(9, 1, 60))
        .unwrap();
    let original: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&progress_path).unwrap()).unwrap();
    for (field, value) in [
        ("remaining", serde_json::json!(999999)),
        ("work_seconds", serde_json::json!(0)),
        ("break_seconds", serde_json::json!(0)),
    ] {
        let mut invalid = original.clone();
        invalid["info"][field] = value;
        invalid["info"]["paused"] = serde_json::json!(true);
        std::fs::write(&progress_path, serde_json::to_string(&invalid).unwrap()).unwrap();
        let reopened = TimerApp::restore_from(
            settings(),
            &progress_path,
            fastrand::Rng::with_seed(42),
            time(9, 2, 0),
        );
        assert_eq!(reopened.info().remaining, 48 * 60);
        assert!(!reopened.info().paused);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn restored_guidance_without_an_optional_text_field_keeps_its_pair_when_work_finishes() {
    let (settings_path, progress_path, dir) = paths();
    let mut configured = settings();
    let round = &mut configured.schedules[0].periods[0].overrides[0];
    round.content = None;
    round.work_minutes = Some(1);
    round.guidance = Some(
        serde_json::from_value(serde_json::json!({
            "kind":"video", "source":{"type":"local","id":"local-00000000000000000000000000000001"}
        }))
        .unwrap(),
    );
    let app = TimerApp::new_at(configured, fastrand::Rng::with_seed(42), time(9, 0, 0));
    app.settings().save_to(&settings_path).unwrap();
    app.save_progress_to(&progress_path, time(9, 0, 0)).unwrap();
    let mut saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&progress_path).unwrap()).unwrap();
    saved["info"].as_object_mut().unwrap().remove("tip");
    std::fs::write(&progress_path, serde_json::to_string(&saved).unwrap()).unwrap();
    let mut reopened = TimerApp::restore_from(
        Settings::load_from(&settings_path),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(9, 0, 0),
    );
    assert!(reopened.info().tip.is_none());
    assert_eq!(reopened.info().guidance, app.info().guidance);
    let output = reopened.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        time(9, 1, 60),
    );
    assert_eq!(reopened.info().guidance, app.info().guidance);
    assert!(
        matches!(&output.events[..], [crate::timer::TimerEvent::Reminder(payload)]
        if payload.tip.is_empty() && payload.guidance == app.info().guidance)
    );
    reopened.dispatch(Action::StartBreak, time(9, 1, 60));
    assert_eq!(reopened.info().guidance, app.info().guidance);
    assert!(reopened.info().tip.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_progress_and_legacy_simple_settings_keep_the_original_simple_startup() {
    let (settings_path, progress_path, dir) = paths();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"work_minutes":27,"break_minutes":3,"sound":false,"tips":["旧自定义提示"]}"#,
    )
    .unwrap();
    let configured = Settings::load_from(&settings_path);
    let mut app = TimerApp::restore_from(
        configured,
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(9, 0, 0),
    );
    assert_eq!(app.info().remaining, 1620);
    assert!(!app.settings().use_schedule);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 0,
        },
        time(9, 10, 600),
    );
    app.save_progress_to(&progress_path, time(9, 10, 600))
        .unwrap();
    let reopened = TimerApp::restore_from(
        Settings::load_from(&settings_path),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(10, 30, 0),
    );
    assert_eq!(reopened.info().remaining, 1620);
    assert_eq!(reopened.info().break_seconds, 180);
    assert!(!reopened.settings().sound);
    assert_eq!(reopened.settings().tips[0].text, "旧自定义提示");
    std::fs::write(&progress_path, "{截断的进度").unwrap();
    let malformed = TimerApp::restore_from(
        Settings::load_from(&settings_path),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(10, 30, 0),
    );
    assert_eq!(malformed.info().remaining, 1620);
    std::fs::remove_dir_all(dir).unwrap();
}
