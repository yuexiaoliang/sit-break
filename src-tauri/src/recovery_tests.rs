use std::time::Duration;

use crate::timer::{Action, LocalTime, Mode, TimerApp, TimerTime};
use crate::Settings;

fn time(hour: u32, minute: u32, monotonic: u64) -> TimerTime {
    TimerTime {
        monotonic: Duration::from_secs(monotonic),
        local: Some(LocalTime { date: "2026-10-14".into(), seconds_of_day: hour * 3600 + minute * 60 }),
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
fn reopening_in_the_same_period_restores_round_two_without_counting_the_closed_time() {
    let (settings_path, progress_path, dir) = paths();
    let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 0, 0));
    app.dispatch(Action::Tick { elapsed_secs: 3000, idle_secs: 0 }, time(9, 50, 3000));
    app.dispatch(Action::StartBreak, time(9, 55, 3300));
    app.dispatch(Action::EndBreak, time(9, 55, 3300));
    app.dispatch(Action::Tick { elapsed_secs: 125, idle_secs: 0 }, time(9, 57, 3425));
    let before = app.info();
    assert_eq!(before.round.as_ref().unwrap().number, 2);
    assert_eq!(before.remaining, 3175);
    app.settings().save_to(&settings_path).unwrap();
    app.save_progress_to(&progress_path, time(9, 57, 3425)).unwrap();
    let mut reopened = TimerApp::restore_from(Settings::load_from(&settings_path), &progress_path, fastrand::Rng::with_seed(42), time(10, 50, 0));
    assert_eq!(reopened.info(), before);
    let output = reopened.dispatch(Action::Tick { elapsed_secs: 1, idle_secs: 0 }, time(10, 50, 1));
    assert_eq!(output.state, Some(reopened.info()));
    assert_eq!(reopened.info().mode, Mode::Work);
    assert_eq!(reopened.info().remaining, 3174);
    assert!(output.events.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn expired_paused_progress_is_discarded_but_manual_pause_survives_lunch_afternoon_and_next_day() {
    let (settings_path, progress_path, dir) = paths();
    let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 0, 0));
    app.dispatch(Action::Tick { elapsed_secs: 3000, idle_secs: 0 }, time(9, 50, 3000));
    app.dispatch(Action::TogglePause, time(9, 50, 3000));
    app.settings().save_to(&settings_path).unwrap();
    app.save_progress_to(&progress_path, time(9, 50, 3000)).unwrap();
    for (now, mode, work_seconds) in [
        (time(12, 30, 0), Mode::Idle, 0),
        (time(13, 30, 0), Mode::Work, 3600),
        ({ let mut next = time(9, 0, 0); next.local.as_mut().unwrap().date = "2026-10-15".into(); next }, Mode::Work, 3000),
    ] {
        let mut reopened = TimerApp::restore_from(Settings::load_from(&settings_path), &progress_path, fastrand::Rng::with_seed(42), now.clone());
        assert!(reopened.info().paused);
        assert_eq!(reopened.info().mode, mode);
        assert_eq!(reopened.info().work_seconds, work_seconds);
        let output = reopened.dispatch(Action::Tick { elapsed_secs: 9000, idle_secs: 9000 }, now);
        assert!(output.events.is_empty());
        assert!(reopened.info().paused);
        if mode == Mode::Work { assert_eq!(reopened.info().round.unwrap().number, 1); }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn corrupted_progress_does_not_inject_invalid_remaining_or_manual_pause() {
    let (_, progress_path, dir) = paths();
    let app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 0, 0));
    app.save_progress_to(&progress_path, time(9, 0, 0)).unwrap();
    let original: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&progress_path).unwrap()).unwrap();
    for (field, value) in [
        ("remaining", serde_json::json!(999999)),
        ("work_seconds", serde_json::json!(0)),
        ("break_seconds", serde_json::json!(0)),
    ] {
        let mut invalid = original.clone();
        invalid["info"][field] = value;
        invalid["info"]["paused"] = serde_json::json!(true);
        std::fs::write(&progress_path, serde_json::to_string(&invalid).unwrap()).unwrap();
        let reopened = TimerApp::restore_from(settings(), &progress_path, fastrand::Rng::with_seed(42), time(10, 0, 0));
        assert_eq!(reopened.info().remaining, 3000);
        assert!(!reopened.info().paused);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
