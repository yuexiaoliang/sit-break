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
                 "overrides":[{"round":1,"content":"爬楼"}]},
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
fn sleeping_across_many_round_lengths_finishes_only_the_current_break() {
    let mut app = scheduled_timer(false, 9, 0);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at("2026-10-14", 9, 50),
    );
    app.dispatch(Action::StartBreak, at("2026-10-14", 9, 50));
    app.dispatch(Action::Suspend { idle_secs: 0 }, at("2026-10-14", 9, 50));
    let wake = app.dispatch(
        Action::Resume {
            elapsed_secs: 4200,
            idle_secs: 0,
        },
        at("2026-10-14", 11, 0),
    );
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 3000);
    assert_eq!(app.info().round.unwrap().number, 2);
    assert!(wake.events.is_empty());
    app.dispatch(
        Action::Resume {
            elapsed_secs: 4200,
            idle_secs: 0,
        },
        at("2026-10-14", 11, 0),
    );
    assert_eq!(app.info().round.unwrap().number, 2);
}

#[test]
fn waking_input_does_not_hide_the_sleep_away_time_for_work_or_reminders() {
    for reminding in [false, true] {
        let mut app = scheduled_timer(true, 9, 0);
        app.dispatch(
            Action::Tick {
                elapsed_secs: if reminding { 3000 } else { 120 },
                idle_secs: 0,
            },
            at("2026-10-14", 9, 50),
        );
        app.dispatch(Action::Suspend { idle_secs: 50 }, at("2026-10-14", 9, 50));
        let wake = app.dispatch(
            Action::Resume {
                elapsed_secs: 550,
                idle_secs: 0,
            },
            at("2026-10-14", 10, 0),
        );
        assert_eq!(app.info().mode, Mode::Work);
        assert_eq!(app.info().remaining, 3000);
        assert_eq!(
            app.info().round.unwrap().number,
            if reminding { 2 } else { 1 }
        );
        assert!(wake.events.is_empty());
    }
    let mut app = scheduled_timer(false, 9, 0);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at("2026-10-14", 9, 50),
    );
    app.dispatch(Action::Suspend { idle_secs: 50 }, at("2026-10-14", 9, 50));
    let wake = app.dispatch(
        Action::Resume {
            elapsed_secs: 2000,
            idle_secs: 0,
        },
        at("2026-10-14", 10, 30),
    );
    assert_eq!(app.info().mode, Mode::Remind);
    assert_eq!(app.info().round.unwrap().number, 1);
    assert!(wake.events.is_empty());
}

#[test]
fn waking_work_or_reminders_uses_only_the_current_date_and_period() {
    for reminding in [false, true] {
        for (date, hour, minute, expected_period) in [
            ("2026-10-14", 12, 30, None),
            ("2026-10-14", 13, 40, Some("pm")),
            ("2026-10-15", 9, 10, Some("am")),
        ] {
            let mut app = scheduled_timer(true, 9, 0);
            app.dispatch(
                Action::Tick {
                    elapsed_secs: if reminding { 3000 } else { 120 },
                    idle_secs: 0,
                },
                at("2026-10-14", 9, 50),
            );
            app.dispatch(Action::Suspend { idle_secs: 0 }, at("2026-10-14", 9, 50));
            let wake = app.dispatch(
                Action::Resume {
                    elapsed_secs: 86400,
                    idle_secs: 0,
                },
                at(date, hour, minute),
            );
            assert!(wake.events.is_empty());
            if let Some(period) = expected_period {
                let info = app.info();
                assert_eq!(info.mode, Mode::Work);
                assert_eq!(info.remaining, if period == "pm" { 3600 } else { 3000 });
                let round = info.round.unwrap();
                assert_eq!(round.number, 1);
                assert_eq!(round.period_id, period);
                assert_eq!(round.date, date);
            } else {
                assert_eq!(app.info().mode, Mode::Idle);
                assert!(app.info().round.is_none());
            }
        }
    }
}

#[test]
fn sleep_away_reset_is_not_hidden_by_an_awake_only_post_break_grace_clock() {
    let mut app = simple_timer();
    let mut settings = app.settings().clone();
    settings.idle_reset = true;
    app.dispatch(Action::SaveSettings(settings), Duration::ZERO);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        Duration::from_secs(3000),
    );
    app.dispatch(Action::StartBreak, Duration::from_secs(3000));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 600,
            idle_secs: 0,
        },
        Duration::from_secs(3600),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 120,
            idle_secs: 0,
        },
        Duration::from_secs(3720),
    );
    assert_eq!(app.info().remaining, 2880);
    app.dispatch(Action::Suspend { idle_secs: 0 }, Duration::from_secs(3720));
    // macOS 的普通经过时间不包含这 600 秒睡眠，原生连续钟单独报告它。
    app.dispatch(
        Action::Resume {
            elapsed_secs: 600,
            idle_secs: 0,
        },
        Duration::from_secs(3720),
    );
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 3000);
}

#[test]
fn manual_pause_freezes_sleep_counting_but_still_discards_expired_work_and_reminders() {
    for phase in [Mode::Work, Mode::Remind, Mode::Break] {
        let mut app = scheduled_timer(true, 9, 0);
        if phase != Mode::Work {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 3000,
                    idle_secs: 0,
                },
                at("2026-10-14", 9, 50),
            );
        }
        if phase == Mode::Break {
            app.dispatch(Action::StartBreak, at("2026-10-14", 9, 50));
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 60,
                    idle_secs: 0,
                },
                at("2026-10-14", 9, 51),
            );
        }
        app.dispatch(Action::TogglePause, at("2026-10-14", 9, 51));
        app.dispatch(Action::Suspend { idle_secs: 5000 }, at("2026-10-14", 9, 51));
        let wake = app.dispatch(
            Action::Resume {
                elapsed_secs: 14400,
                idle_secs: 5000,
            },
            at("2026-10-14", 13, 51),
        );
        assert!(app.info().paused);
        assert!(wake.events.is_empty());
        if phase == Mode::Break {
            assert_eq!(app.info().mode, Mode::Break);
            assert_eq!(app.info().remaining, 540);
            assert_eq!(app.info().round.unwrap().period_id, "am");
        } else {
            assert_eq!(app.info().mode, Mode::Work);
            assert_eq!(app.info().remaining, 3600);
            assert_eq!(app.info().round.unwrap().period_id, "pm");
        }
        app.dispatch(Action::TogglePause, at("2026-10-14", 13, 51));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 1,
                idle_secs: 0,
            },
            at("2026-10-14", 13, 51),
        );
        assert_eq!(
            app.info().remaining,
            if phase == Mode::Break { 539 } else { 3599 }
        );
    }
}

#[test]
fn a_sleeping_break_can_continue_into_lunch_and_finishes_at_the_wake_period() {
    for (date, hour, minute, elapsed, expected_mode, expected_period, remaining) in [
        ("2026-10-14", 12, 2, 180, Mode::Break, Some("am"), 420),
        ("2026-10-14", 12, 11, 720, Mode::Idle, None, 0),
        ("2026-10-14", 13, 40, 6060, Mode::Work, Some("pm"), 3600),
        ("2026-10-15", 9, 10, 76260, Mode::Work, Some("am"), 3000),
    ] {
        let mut app = scheduled_timer(false, 11, 9);
        app.dispatch(
            Action::Tick {
                elapsed_secs: 3000,
                idle_secs: 0,
            },
            at("2026-10-14", 11, 59),
        );
        app.dispatch(Action::StartBreak, at("2026-10-14", 11, 59));
        app.dispatch(Action::Suspend { idle_secs: 0 }, at("2026-10-14", 11, 59));
        let wake = app.dispatch(
            Action::Resume {
                elapsed_secs: elapsed,
                idle_secs: 0,
            },
            at(date, hour, minute),
        );
        assert_eq!(app.info().mode, expected_mode);
        assert_eq!(app.info().remaining, remaining);
        assert!(wake.events.is_empty());
        if let Some(period) = expected_period {
            let round = app.info().round.unwrap();
            assert_eq!(round.period_id, period);
            assert_eq!(round.number, 1);
            assert_eq!(
                round.date,
                if expected_mode == Mode::Break {
                    "2026-10-14"
                } else {
                    date
                }
            );
        } else {
            assert!(app.info().round.is_none());
        }
    }
}

#[test]
fn closing_a_break_preserves_remaining_time_while_sleeping_the_live_app_counts_it() {
    let mut app = scheduled_timer(false, 9, 0);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        at("2026-10-14", 9, 50),
    );
    app.dispatch(Action::StartBreak, at("2026-10-14", 9, 50));
    let path = std::env::temp_dir().join(format!(
        "sit-break-close-versus-sleep-{}.json",
        fastrand::u64(..)
    ));
    app.save_progress_to(&path, at("2026-10-14", 9, 50))
        .unwrap();
    let mut reopened = TimerApp::restore_from(
        app.settings().clone(),
        &path,
        fastrand::Rng::with_seed(42),
        at("2026-10-14", 9, 55),
    );
    assert_eq!(reopened.info().mode, Mode::Break);
    assert_eq!(reopened.info().remaining, 600);
    app.dispatch(Action::Suspend { idle_secs: 0 }, at("2026-10-14", 9, 50));
    app.dispatch(
        Action::Resume {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at("2026-10-14", 9, 55),
    );
    assert_eq!(app.info().remaining, 300);
    reopened.dispatch(Action::Suspend { idle_secs: 0 }, at("2026-10-14", 9, 55));
    reopened.dispatch(
        Action::Resume {
            elapsed_secs: 300,
            idle_secs: 0,
        },
        at("2026-10-14", 10, 0),
    );
    assert_eq!(reopened.info().remaining, 300);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn sleeping_after_reopen_counts_away_time_without_counting_the_closed_gap() {
    let mut app = scheduled_timer(true, 9, 0);
    app.dispatch(
        Action::Tick {
            elapsed_secs: 120,
            idle_secs: 100,
        },
        at("2026-10-14", 9, 2),
    );
    let path = std::env::temp_dir().join(format!(
        "sit-break-reopened-sleep-{}.json",
        fastrand::u64(..)
    ));
    app.save_progress_to(&path, at("2026-10-14", 9, 2)).unwrap();
    let mut app = TimerApp::restore_from(
        app.settings().clone(),
        &path,
        fastrand::Rng::with_seed(42),
        at("2026-10-14", 9, 32),
    );
    // 原始 OS 离开包含关闭的 30 分钟；有效离开仍只有退出前的 100 秒。
    app.dispatch(Action::Suspend { idle_secs: 1900 }, at("2026-10-14", 9, 32));
    app.dispatch(
        Action::Resume {
            elapsed_secs: 300,
            idle_secs: 2200,
        },
        at("2026-10-14", 9, 37),
    );
    assert_eq!(app.info().remaining, 2880);
    assert_eq!(app.info().round.unwrap().number, 1);
    // 再睡 200 秒才达到本轮 600 秒休息阈值，重置当前轮而不跳轮。
    app.dispatch(Action::Suspend { idle_secs: 2200 }, at("2026-10-14", 9, 37));
    app.dispatch(
        Action::Resume {
            elapsed_secs: 200,
            idle_secs: 2400,
        },
        at("2026-10-14", 9, 40),
    );
    assert_eq!(app.info().remaining, 3000);
    assert_eq!(app.info().round.unwrap().number, 1);
    std::fs::remove_file(path).unwrap();
}
