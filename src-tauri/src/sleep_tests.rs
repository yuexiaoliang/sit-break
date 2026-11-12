use std::time::Duration;

use crate::timer::{Action, Mode, TimerApp};
use crate::Settings;

fn simple_timer() -> TimerApp {
    TimerApp::new(
        Settings {
            work_minutes: 50,
            break_minutes: 10,
            idle_reset: false,
            ..Settings::default()
        },
        fastrand::Rng::with_seed(42),
    )
}

#[test]
fn sleeping_work_does_not_count_time_or_emit_a_reminder() {
    let mut app = simple_timer();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 120,
            idle_secs: 0,
        },
        Duration::from_secs(120),
    );
    app.dispatch(Action::Suspend { idle_secs: 0 }, Duration::from_secs(120));
    let sleep_tick = app.dispatch(
        Action::Tick {
            elapsed_secs: 3600,
            idle_secs: 3600,
        },
        Duration::from_secs(3720),
    );
    let wake = app.dispatch(
        Action::Resume {
            elapsed_secs: 3600,
            idle_secs: 0,
        },
        Duration::from_secs(3720),
    );
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 2880);
    assert!(sleep_tick.events.is_empty());
    assert!(wake.events.is_empty());
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        Duration::from_secs(3721),
    );
    assert_eq!(app.info().remaining, 2879);
}

#[test]
fn a_started_break_counts_sleep_once_and_then_continues_its_remaining_time() {
    let mut app = simple_timer();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        Duration::from_secs(3000),
    );
    app.dispatch(Action::StartBreak, Duration::from_secs(3000));
    app.dispatch(Action::Suspend { idle_secs: 20 }, Duration::from_secs(3000));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 200,
            idle_secs: 200,
        },
        Duration::from_secs(3200),
    );
    let wake = app.dispatch(
        Action::Resume {
            elapsed_secs: 200,
            idle_secs: 0,
        },
        Duration::from_secs(3200),
    );
    assert_eq!(app.info().mode, Mode::Break);
    assert_eq!(app.info().remaining, 400);
    assert!(wake.events.is_empty());
    app.dispatch(
        Action::Resume {
            elapsed_secs: 200,
            idle_secs: 0,
        },
        Duration::from_secs(3200),
    );
    assert_eq!(app.info().remaining, 400);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        Duration::from_secs(3201),
    );
    assert_eq!(app.info().remaining, 399);
}

fn scheduled_timer(idle_reset: bool, hour: u32, minute: u32) -> TimerApp {
    let settings = Settings {
        use_schedule: true,
        idle_reset,
        schedules: serde_json::from_value(serde_json::json!([{
            "id":"day", "name":"一天", "enabled":true,
            "repeat":{"kind":"weekly","weekdays":[1,2,3,4,5,6,7]},
            "periods":[
                {"id":"am","start":"09:00","end":"12:00","work_minutes":50,"break_minutes":10,
                 "overrides":[{"round":1,"content":"爬楼","guidance":{"kind":"video","source":{"type":"local","id":"local-00000000000000000000000000000001"}}}]},
                {"id":"pm","start":"13:30","end":"18:00","work_minutes":60,"break_minutes":5}
            ]
        }]))
        .unwrap(),
        ..Settings::default()
    };
    assert!(
        settings.validate().is_empty(),
        "正常睡眠样例应能通过设置校验"
    );
    TimerApp::new_at(
        settings,
        fastrand::Rng::with_seed(42),
        at("2026-10-14", hour, minute),
    )
}

fn at(date: &str, hour: u32, minute: u32) -> crate::timer::TimerTime {
    crate::timer::TimerTime {
        monotonic: Duration::from_secs(u64::from(hour * 3600 + minute * 60)),
        local: Some(crate::timer::LocalTime {
            date: date.into(),
            seconds_of_day: hour * 3600 + minute * 60,
        }),
    }
}

#[test]
fn waking_from_a_break_skips_elapsed_rounds_and_lunch_without_replaying_reminders() {
    let mut app = scheduled_timer(true, 9, 55);
    app.dispatch(Action::StartBreak, at("2026-10-14", 9, 55));
    app.dispatch(Action::Suspend { idle_secs: 0 }, at("2026-10-14", 9, 56));
    assert!(!app.reminder_visible());

    let woke = app.dispatch(
        Action::Resume {
            elapsed_secs: 9000,
            idle_secs: 0,
        },
        at("2026-10-14", 13, 50),
    );
    let info = app.info();
    assert_eq!(info.mode, Mode::Work);
    assert_eq!(info.remaining, 40 * 60);
    assert_eq!(info.round.unwrap().period_id, "pm");
    assert!(woke.events.is_empty());
}

#[test]
fn waking_in_the_same_break_uses_its_remaining_clock_time() {
    let mut app = scheduled_timer(true, 9, 50);
    app.dispatch(Action::StartBreak, at("2026-10-14", 9, 50));
    app.dispatch(Action::Suspend { idle_secs: 0 }, at("2026-10-14", 9, 51));
    let woke = app.dispatch(
        Action::Resume {
            elapsed_secs: 300,
            idle_secs: 300,
        },
        at("2026-10-14", 9, 56),
    );
    assert_eq!(app.info().mode, Mode::Break);
    assert_eq!(app.info().remaining, 240);
    assert!(app.reminder_visible());
    assert!(woke.events.is_empty());
}

#[test]
fn waking_during_a_later_rest_prompts_only_for_the_current_round() {
    let mut app = scheduled_timer(true, 9, 10);
    app.dispatch(Action::Suspend { idle_secs: 0 }, at("2026-10-14", 9, 10));
    let woke = app.dispatch(
        Action::Resume {
            elapsed_secs: 10000,
            idle_secs: 10000,
        },
        at("2026-10-14", 11, 53),
    );
    assert_eq!(app.info().round.unwrap().number, 3);
    assert_eq!(app.info().remaining, 7 * 60);
    assert!(app.reminder_visible());
    assert!(matches!(
        woke.events.as_slice(),
        [crate::timer::TimerEvent::Reminder(_)]
    ));
}
