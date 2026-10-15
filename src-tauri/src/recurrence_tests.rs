use std::time::Duration;

use crate::timer::{Action, LocalTime, Mode, TimerApp, TimerTime};
use crate::Settings;

fn time(date: &str, hour: u32, minute: u32) -> TimerTime {
    let elapsed_days = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .unwrap()
        .signed_duration_since(chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap())
        .num_days() as u64;
    TimerTime {
        monotonic: Duration::from_secs(
            elapsed_days * 86_400 + u64::from(hour * 3600 + minute * 60),
        ),
        local: Some(LocalTime {
            date: date.into(),
            seconds_of_day: hour * 3600 + minute * 60,
        }),
    }
}

fn scheduled(value: serde_json::Value) -> Settings {
    Settings {
        use_schedule: true,
        idle_reset: false,
        schedules: serde_json::from_value(value).unwrap(),
        ..Settings::default()
    }
}

#[test]
fn weekday_and_weekend_schedules_only_run_on_their_selected_local_days() {
    let settings = scheduled(serde_json::json!([
        {"id":"workdays", "name":"工作日", "enabled":true, "repeat":{"kind":"weekly","weekdays":[1,2,3,4,5]}, "periods":[{"id":"morning","start":"09:00","end":"12:00","work_minutes":50,"break_minutes":10}]},
        {"id":"weekend", "name":"周末", "enabled":true, "repeat":{"kind":"weekly","weekdays":[6]}, "periods":[{"id":"weekend-morning","start":"09:00","end":"12:00","work_minutes":30,"break_minutes":5}]}
    ]));
    let path = std::env::temp_dir().join(format!("sit-break-weekly-{}.json", fastrand::u64(..)));
    let mut app = TimerApp::new(Settings::default(), fastrand::Rng::with_seed(42));
    let saved = app.dispatch(Action::SaveSettings(settings), time("2026-10-14", 9, 0));
    assert!(saved.validation_errors.is_empty());
    assert_eq!(app.info().round.unwrap().schedule_id, "workdays");
    saved.persist_settings.unwrap().save_to(&path).unwrap();
    let mut app = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        time("2026-10-17", 9, 0),
    );
    assert_eq!(app.info().work_seconds, 1800);
    assert_eq!(app.info().round.unwrap().schedule_id, "weekend");
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        time("2026-10-18", 9, 0),
    );
    assert_eq!(app.info().mode, Mode::Idle);
    assert_eq!(app.settings().schedules.len(), 2);
    std::fs::remove_file(path).unwrap();
}

fn plan(
    id: &str,
    repeat: serde_json::Value,
    start: &str,
    end: &str,
    enabled: bool,
) -> serde_json::Value {
    serde_json::json!({"id":id, "name":id, "enabled":enabled, "repeat":repeat, "periods":[{"id":format!("{id}-period"),"start":start,"end":end,"work_minutes":50,"break_minutes":10}]})
}

#[test]
fn same_date_conflicts_reject_the_entire_save_and_locate_both_plans() {
    let date = serde_json::json!({"kind":"once","date":"2026-10-14"});
    let original = scheduled(serde_json::json!([plan(
        "left",
        date.clone(),
        "09:00",
        "12:00",
        true
    )]));
    let path = std::env::temp_dir().join(format!("sit-break-conflict-{}.json", fastrand::u64(..)));
    original.save_to(&path).unwrap();
    let mut app = TimerApp::new_at(
        original,
        fastrand::Rng::with_seed(42),
        time("2026-10-14", 9, 0),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 12,
            idle_secs: 0,
        },
        time("2026-10-14", 9, 1),
    );
    let before = app.info();
    let before_settings = serde_json::to_value(app.settings()).unwrap();
    let mut invalid = scheduled(serde_json::json!([
        plan("left", date.clone(), "09:00", "12:00", true),
        plan("right", date, "11:30", "13:00", true)
    ]));
    invalid.sound = false;
    invalid.autostart = true;
    invalid.work_minutes = 55;
    let output = app.dispatch(
        Action::SaveSettings(invalid.clone()),
        time("2026-10-14", 14, 0),
    );
    let error = output
        .validation_errors
        .iter()
        .find(|error| error.code == "overlap")
        .expect("同一天启用日程的重叠必须拒绝");
    assert_eq!(error.schedule_id.as_deref(), Some("right"));
    assert_eq!(error.period_id.as_deref(), Some("right-period"));
    let conflict = error.conflict.as_ref().unwrap();
    assert_eq!(conflict.schedule_id, "left");
    assert_eq!(
        (conflict.start.as_str(), conflict.end.as_str()),
        ("09:00", "12:00")
    );
    assert!(output.persist_settings.is_none());
    assert!(!output.settings_changed);
    assert_eq!(app.info(), before);
    assert_eq!(
        serde_json::to_value(app.settings()).unwrap(),
        before_settings
    );
    assert!(invalid.save_to(&path).is_err());
    assert_eq!(
        serde_json::to_value(Settings::load_from(&path)).unwrap(),
        before_settings
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn weekly_conflicts_report_only_the_weekdays_the_two_plans_share() {
    let settings = scheduled(serde_json::json!([
        plan(
            "monday-wednesday",
            serde_json::json!({"kind":"weekly","weekdays":[1,3]}),
            "09:00",
            "12:00",
            true
        ),
        plan(
            "wednesday-thursday",
            serde_json::json!({"kind":"weekly","weekdays":[3,4]}),
            "10:00",
            "13:00",
            true
        )
    ]));
    let mut app = TimerApp::new(Settings::default(), fastrand::Rng::with_seed(42));
    let output = app.dispatch(Action::SaveSettings(settings), time("2026-10-14", 9, 0));
    let conflict = output
        .validation_errors
        .iter()
        .find(|error| error.code == "overlap")
        .expect("共同星期三的重叠必须拒绝")
        .conflict
        .as_ref()
        .unwrap();
    assert_eq!(conflict.schedule_id, "monday-wednesday");
    assert_eq!(
        serde_json::to_value(&conflict.occurrence).unwrap(),
        serde_json::json!({"kind":"weekly","weekdays":[3]})
    );
    assert!(output.persist_settings.is_none());
}

#[test]
fn one_time_and_weekly_overlap_is_resolved_against_the_actual_date_in_either_order() {
    for reversed in [false, true] {
        let mut plans = vec![
            plan(
                "one-time",
                serde_json::json!({"kind":"once","date":"2026-10-14"}),
                "09:00",
                "12:00",
                true,
            ),
            plan(
                "weekly",
                serde_json::json!({"kind":"weekly","weekdays":[3]}),
                "10:00",
                "13:00",
                true,
            ),
        ];
        if reversed {
            plans.reverse();
        }
        let mut app = TimerApp::new(Settings::default(), fastrand::Rng::with_seed(42));
        let output = app.dispatch(
            Action::SaveSettings(scheduled(serde_json::json!(plans))),
            time("2026-10-14", 9, 0),
        );
        let conflict = output
            .validation_errors
            .iter()
            .find(|error| error.code == "overlap")
            .expect("一次性周三与每周三的重叠必须拒绝")
            .conflict
            .as_ref()
            .unwrap();
        assert_eq!(
            serde_json::to_value(&conflict.occurrence).unwrap(),
            serde_json::json!({"kind":"once","date":"2026-10-14"})
        );
    }
}

#[test]
fn invalid_or_empty_weekday_selections_are_located_without_applying_any_settings() {
    for weekdays in [vec![], vec![0], vec![8], vec![1, 1]] {
        let mut app = TimerApp::new(Settings::default(), fastrand::Rng::with_seed(42));
        let before = app.info();
        let invalid = scheduled(serde_json::json!([plan(
            "invalid-weekdays",
            serde_json::json!({"kind":"weekly","weekdays":weekdays}),
            "09:00",
            "12:00",
            true
        )]));
        let output = app.dispatch(Action::SaveSettings(invalid), time("2026-10-14", 9, 0));
        let error = output
            .validation_errors
            .iter()
            .find(|error| error.code == "invalid_weekdays")
            .expect("每周日程至少选择一个有效且不重复的星期");
        assert_eq!(error.schedule_id.as_deref(), Some("invalid-weekdays"));
        assert_eq!(error.field, "weekdays");
        assert!(output.persist_settings.is_none());
        assert_eq!(app.info(), before);
        assert!(!app.settings().use_schedule);
    }
}

#[test]
fn different_dates_or_weekdays_adjacent_periods_and_disabled_plans_are_allowed() {
    let wed = serde_json::json!({"kind":"once","date":"2026-10-14"});
    let thu = serde_json::json!({"kind":"once","date":"2026-10-15"});
    let weekly_wed = serde_json::json!({"kind":"weekly","weekdays":[3]});
    for plans in [
        vec![
            plan("left", wed.clone(), "09:00", "12:00", true),
            plan("right", thu, "09:00", "12:00", true),
        ],
        vec![
            plan(
                "left",
                serde_json::json!({"kind":"weekly","weekdays":[1]}),
                "09:00",
                "12:00",
                true,
            ),
            plan(
                "right",
                serde_json::json!({"kind":"weekly","weekdays":[2]}),
                "09:00",
                "12:00",
                true,
            ),
        ],
        vec![
            plan(
                "left",
                serde_json::json!({"kind":"once","date":"2026-10-15"}),
                "09:00",
                "12:00",
                true,
            ),
            plan("right", weekly_wed.clone(), "09:00", "12:00", true),
        ],
        vec![
            plan("left", wed.clone(), "09:00", "12:00", true),
            plan("right", wed.clone(), "12:00", "18:00", true),
        ],
        vec![
            plan("left", weekly_wed.clone(), "09:00", "12:00", true),
            plan("right", weekly_wed, "12:00", "18:00", true),
        ],
        vec![
            plan("left", wed.clone(), "09:00", "12:00", true),
            plan("right", wed.clone(), "09:00", "12:00", false),
        ],
        vec![
            plan("left", wed.clone(), "09:00", "12:00", false),
            plan("right", wed, "09:00", "12:00", false),
        ],
    ] {
        let mut app = TimerApp::new(Settings::default(), fastrand::Rng::with_seed(42));
        let output = app.dispatch(
            Action::SaveSettings(scheduled(serde_json::json!(plans))),
            time("2026-10-14", 9, 0),
        );
        assert!(
            output.validation_errors.is_empty(),
            "{:?}",
            output.validation_errors
        );
        assert!(output.persist_settings.is_some());
        assert_eq!(app.settings().schedules.len(), 2);
    }
}

#[test]
fn disabling_keeps_a_plan_and_conflicting_reenable_keeps_the_current_valid_plan() {
    let date = serde_json::json!({"kind":"once","date":"2026-10-14"});
    let settings = scheduled(serde_json::json!([
        plan("left", date.clone(), "09:00", "12:00", true),
        plan("right", date, "09:00", "12:00", false)
    ]));
    let mut app = TimerApp::new_at(
        settings,
        fastrand::Rng::with_seed(42),
        time("2026-10-14", 9, 0),
    );
    let mut disabled = app.settings().clone();
    disabled.schedules[0].enabled = false;
    app.dispatch(Action::SaveSettings(disabled), time("2026-10-14", 9, 0));
    assert_eq!(app.info().mode, Mode::Idle);
    assert_eq!(app.settings().schedules[0].periods[0].id, "left-period");
    let mut switched = app.settings().clone();
    switched.schedules[1].enabled = true;
    app.dispatch(Action::SaveSettings(switched), time("2026-10-14", 9, 0));
    assert_eq!(app.info().round.unwrap().schedule_id, "right");
    let before = app.info();
    let mut conflicting = app.settings().clone();
    conflicting.schedules[0].enabled = true;
    let output = app.dispatch(Action::SaveSettings(conflicting), time("2026-10-14", 9, 0));
    assert_eq!(output.validation_errors[0].code, "overlap");
    assert_eq!(app.info(), before);
    assert!(!app.settings().schedules[0].enabled);
    assert!(app.settings().schedules[1].enabled);
}

#[test]
fn every_day_repeats_on_sunday_and_weekly_internal_overlap_remains_invalid() {
    let daily = scheduled(serde_json::json!([plan(
        "every-day",
        serde_json::json!({"kind":"weekly","weekdays":[1,2,3,4,5,6,7]}),
        "09:00",
        "12:00",
        true
    )]));
    let app = TimerApp::new_at(
        daily.clone(),
        fastrand::Rng::with_seed(42),
        time("2026-10-18", 9, 0),
    );
    assert_eq!(app.info().round.unwrap().schedule_id, "every-day");
    let mut invalid = daily;
    let mut overlap = invalid.schedules[0].periods[0].clone();
    overlap.id = "overlap".into();
    overlap.start = "11:00".into();
    overlap.end = "13:00".into();
    invalid.schedules[0].periods.push(overlap);
    let mut app = TimerApp::new(Settings::default(), fastrand::Rng::with_seed(42));
    let output = app.dispatch(Action::SaveSettings(invalid), time("2026-10-18", 9, 0));
    assert_eq!(output.validation_errors[0].code, "overlap");
}

#[test]
fn overnight_split_executes_the_correct_local_day_and_finishes_started_break_first() {
    for (monday, tuesday) in [
        (
            serde_json::json!({"kind":"once","date":"2026-10-12"}),
            serde_json::json!({"kind":"once","date":"2026-10-13"}),
        ),
        (
            serde_json::json!({"kind":"weekly","weekdays":[1]}),
            serde_json::json!({"kind":"weekly","weekdays":[2]}),
        ),
    ] {
        let settings = scheduled(serde_json::json!([
            plan("before-midnight", monday, "22:00", "24:00", true),
            plan("after-midnight", tuesday, "00:00", "01:00", true)
        ]));
        let mut app = TimerApp::new(Settings::default(), fastrand::Rng::with_seed(42));
        let saved = app.dispatch(
            Action::SaveSettings(settings.clone()),
            time("2026-10-12", 23, 5),
        );
        assert!(saved.validation_errors.is_empty());
        assert_eq!(app.info().round.unwrap().schedule_id, "before-midnight");
        app.dispatch(
            Action::Tick {
                elapsed_secs: 3000,
                idle_secs: 0,
            },
            time("2026-10-12", 23, 55),
        );
        app.dispatch(Action::StartBreak, time("2026-10-12", 23, 55));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 300,
                idle_secs: 0,
            },
            time("2026-10-13", 0, 0),
        );
        assert_eq!(app.info().mode, Mode::Break);
        assert_eq!(app.info().round.unwrap().schedule_id, "before-midnight");
        app.dispatch(
            Action::Tick {
                elapsed_secs: 300,
                idle_secs: 0,
            },
            time("2026-10-13", 0, 5),
        );
        let round = app.info().round.unwrap();
        assert_eq!(
            (
                round.schedule_id.as_str(),
                round.date.as_str(),
                round.number
            ),
            ("after-midnight", "2026-10-13", 1)
        );
        assert_eq!(app.info().remaining, 3000);
        app.dispatch(
            Action::Tick {
                elapsed_secs: 1,
                idle_secs: 0,
            },
            time("2026-10-13", 1, 0),
        );
        assert_eq!(app.info().mode, Mode::Idle);
    }
}
