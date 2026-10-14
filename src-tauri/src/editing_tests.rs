use std::time::Duration;

use crate::timer::{Action, LocalTime, Mode, TimerApp, TimerTime};
use crate::Settings;

fn at(hour: u32, minute: u32) -> TimerTime {
    TimerTime {
        monotonic: Duration::from_secs(u64::from(hour * 3600 + minute * 60)),
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
        tips: vec!["默认提示".into()],
        schedules: serde_json::from_value(serde_json::json!([{
            "id":"schedule", "name":"工作日", "enabled":true,
            "repeat":{"kind":"weekly","weekdays":[1,2,3,4,5]},
            "periods":[{"id":"morning", "start":"09:00","end":"12:00","work_minutes":55,"break_minutes":5,"overrides":[
                {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"},
                {"round":2,"work_minutes":20,"break_minutes":3,"content":"深蹲"}
            ]}]
        }])).unwrap(),
        ..Settings::default()
    }
}

fn started_break() -> TimerApp {
    let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), at(9, 0));
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
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 56),
    );
    app
}

#[test]
fn switching_modes_away_and_back_during_a_paused_break_restarts_the_first_round_after_break() {
    let mut app = started_break();
    app.dispatch(Action::TogglePause, at(9, 56));
    let before = app.info();
    assert_eq!(before.mode, Mode::Break);
    for use_schedule in [false, true] {
        let mut changed = app.settings().clone();
        changed.use_schedule = use_schedule;
        let saved = app.dispatch(Action::SaveSettings(changed), at(9, 56));
        assert!(saved.validation_errors.is_empty());
        assert_eq!(saved.state, Some(before.clone()));
        assert_eq!(app.info(), before);
    }
    app.dispatch(Action::EndBreak, at(9, 56));
    let info = app.info();
    assert_eq!(info.round.unwrap().number, 1);
    assert_eq!(
        (info.work_seconds, info.break_seconds, info.tip.as_deref()),
        (3000, 600, Some("爬楼"))
    );
    assert_eq!(info.remaining, 3000);
    assert!(info.paused);
}

#[test]
fn closing_then_restoring_the_active_plan_or_period_during_a_break_restarts_round_one() {
    for disable_plan in [true, false] {
        let mut app = started_break();
        let before = app.info();
        let mut changed = app.settings().clone();
        if disable_plan {
            changed.schedules[0].enabled = false;
        } else {
            changed.schedules[0].periods[0].end = "09:55".into();
        }
        app.dispatch(Action::SaveSettings(changed), at(9, 56));
        assert_eq!(app.info(), before);
        let mut restored = app.settings().clone();
        restored.schedules[0].enabled = true;
        restored.schedules[0].periods[0].end = "12:00".into();
        app.dispatch(Action::SaveSettings(restored), at(9, 56));
        assert_eq!(app.info(), before);
        app.dispatch(
            Action::Tick {
                elapsed_secs: 540,
                idle_secs: 0,
            },
            at(10, 5),
        );
        assert_eq!(app.info().round.unwrap().number, 1);
        assert_eq!(
            (app.info().work_seconds, app.info().break_seconds),
            (3000, 600)
        );
    }
}

#[test]
fn resetting_a_break_with_a_pending_restart_uses_the_latest_first_round_once() {
    let mut app = started_break();
    app.dispatch(Action::TogglePause, at(9, 56));
    let mut off = app.settings().clone();
    off.use_schedule = false;
    app.dispatch(Action::SaveSettings(off), at(9, 56));
    let mut on = app.settings().clone();
    on.use_schedule = true;
    let first = &mut on.schedules[0].periods[0].overrides[0];
    first.work_minutes = Some(40);
    first.break_minutes = Some(4);
    first.content = Some("新首轮".into());
    app.dispatch(Action::SaveSettings(on), at(9, 56));
    app.dispatch(Action::Reset, at(9, 56));
    let info = app.info();
    assert_eq!(
        (info.work_seconds, info.break_seconds, info.tip.as_deref()),
        (2400, 240, Some("新首轮"))
    );
    assert_eq!(info.remaining, 2400);
    assert_eq!(info.round.unwrap().number, 1);
    assert!(info.paused);
    app.dispatch(Action::TogglePause, at(9, 56));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 2400,
            idle_secs: 0,
        },
        at(10, 36),
    );
    app.dispatch(Action::StartBreak, at(10, 36));
    app.dispatch(Action::EndBreak, at(10, 36));
    assert_eq!(app.info().round.unwrap().number, 2);
}

#[test]
fn editing_defaults_overrides_content_and_common_preferences_freezes_each_current_phase() {
    for phase in [Mode::Work, Mode::Remind, Mode::Break] {
        let mut app = if phase == Mode::Break {
            started_break()
        } else {
            TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), at(9, 0))
        };
        if phase == Mode::Work {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 60,
                    idle_secs: 0,
                },
                at(9, 1),
            );
        } else if phase == Mode::Remind {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 3000,
                    idle_secs: 0,
                },
                at(9, 50),
            );
        }
        app.dispatch(Action::TogglePause, at(9, 56));
        let before = app.info();
        let mut changed = app.settings().clone();
        changed.sound = false;
        changed.language = "en".into();
        changed.tips = vec!["新默认提示".into()];
        let period = &mut changed.schedules[0].periods[0];
        period.work_minutes = 60;
        period.break_minutes = 7;
        period.overrides[0].work_minutes = Some(40);
        period.overrides[0].break_minutes = Some(4);
        period.overrides[0].content = Some("修改后的爬楼".into());
        period.overrides[1].work_minutes = Some(30);
        period.overrides[1].break_minutes = Some(8);
        period.overrides[1].content = Some("修改后的深蹲".into());
        let output = app.dispatch(Action::SaveSettings(changed), at(9, 56));
        assert_eq!(output.state, Some(before.clone()));
        assert_eq!(app.info(), before);
        let mut common = app.settings().clone();
        common.widget_size = 100;
        common.autostart = true;
        app.dispatch(Action::SaveSettings(common), at(9, 56));
        assert_eq!(app.info(), before);
        app.dispatch(Action::TogglePause, at(9, 56));
        if phase == Mode::Work {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 2940,
                    idle_secs: 0,
                },
                at(9, 57),
            );
        }
        if phase != Mode::Break {
            app.dispatch(Action::StartBreak, at(9, 57));
        }
        let remaining = app.info().remaining;
        app.dispatch(
            Action::Tick {
                elapsed_secs: remaining,
                idle_secs: 0,
            },
            at(10, 7),
        );
        assert_eq!(app.info().round.unwrap().number, 2);
        assert_eq!(
            (
                app.info().work_seconds,
                app.info().break_seconds,
                app.info().tip.as_deref()
            ),
            (1800, 480, Some("修改后的深蹲"))
        );
        app.dispatch(
            Action::Tick {
                elapsed_secs: 1800,
                idle_secs: 0,
            },
            at(10, 37),
        );
        app.dispatch(Action::StartBreak, at(10, 37));
        app.dispatch(Action::EndBreak, at(10, 37));
        assert_eq!(app.info().round.unwrap().number, 3);
        assert_eq!(
            (
                app.info().work_seconds,
                app.info().break_seconds,
                app.info().tip.as_deref()
            ),
            (3600, 420, Some("新默认提示"))
        );
        let path =
            std::env::temp_dir().join(format!("sit-break-edited-{}.json", fastrand::u64(..)));
        app.settings().save_to(&path).unwrap();
        let mut next_day = at(9, 0);
        next_day.local.as_mut().unwrap().date = "2026-10-15".into();
        let next = TimerApp::new_at(
            Settings::load_from(&path),
            fastrand::Rng::with_seed(42),
            next_day,
        );
        assert_eq!(next.info().round.unwrap().number, 1);
        assert_eq!(
            (
                next.info().work_seconds,
                next.info().break_seconds,
                next.info().tip.as_deref()
            ),
            (2400, 240, Some("修改后的爬楼"))
        );
        assert_eq!(next.settings().widget_size, 100);
        assert!(next.settings().autostart);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn disabling_or_expiring_work_and_reminder_closes_them_without_unpausing() {
    for phase in [Mode::Work, Mode::Remind] {
        for disable_plan in [true, false] {
            let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), at(9, 0));
            if phase == Mode::Remind {
                app.dispatch(
                    Action::Tick {
                        elapsed_secs: 3000,
                        idle_secs: 0,
                    },
                    at(9, 50),
                );
            }
            app.dispatch(Action::TogglePause, at(9, 55));
            let mut changed = app.settings().clone();
            if disable_plan {
                changed.schedules[0].enabled = false;
            } else {
                changed.schedules[0].periods[0].end = "09:55".into();
            }
            let output = app.dispatch(Action::SaveSettings(changed), at(9, 55));
            assert_eq!(app.info().mode, Mode::Idle);
            assert!(app.info().paused);
            assert!(output.update_windows);
            assert!(output.events.is_empty());
            let mut restored = app.settings().clone();
            restored.schedules[0].enabled = true;
            restored.schedules[0].periods[0].end = "12:00".into();
            app.dispatch(Action::SaveSettings(restored), at(9, 55));
            assert_eq!(app.info().round.unwrap().number, 1);
            assert_eq!(app.info().remaining, 3000);
            assert!(app.info().paused);
        }
    }
}

#[test]
fn finishing_after_repeated_mode_switches_uses_the_latest_mode_and_preserved_simple_lengths() {
    let mut app = started_break();
    let before = app.info();
    for use_schedule in [false, true, false] {
        let mut changed = app.settings().clone();
        changed.use_schedule = use_schedule;
        changed.work_minutes = 23;
        changed.break_minutes = 4;
        app.dispatch(Action::SaveSettings(changed), at(9, 56));
        assert_eq!(app.info(), before);
    }
    app.dispatch(Action::EndBreak, at(10, 0));
    assert_eq!(app.info().mode, Mode::Work);
    assert!(app.info().round.is_none());
    assert_eq!(
        (app.info().work_seconds, app.info().break_seconds),
        (1380, 240)
    );
    let mut on = app.settings().clone();
    on.use_schedule = true;
    app.dispatch(Action::SaveSettings(on), at(10, 0));
    assert_eq!(app.info().round.unwrap().number, 1);
    assert_eq!(
        (app.info().work_seconds, app.info().break_seconds),
        (3000, 600)
    );
    assert_eq!(
        (app.settings().work_minutes, app.settings().break_minutes),
        (23, 4)
    );
}

#[test]
fn switching_a_paused_simple_break_to_schedules_then_back_and_forward_keeps_its_snapshot() {
    let mut stored = settings();
    stored.use_schedule = false;
    stored.work_minutes = 1;
    stored.break_minutes = 1;
    let mut app = TimerApp::new_at(stored, fastrand::Rng::with_seed(42), at(9, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        at(9, 1),
    );
    app.dispatch(Action::StartBreak, at(9, 1));
    app.dispatch(Action::TogglePause, at(9, 1));
    let before = app.info();
    for use_schedule in [true, false, true] {
        let mut changed = app.settings().clone();
        changed.use_schedule = use_schedule;
        app.dispatch(Action::SaveSettings(changed), at(9, 1));
        assert_eq!(app.info(), before);
    }
    app.dispatch(Action::EndBreak, at(9, 1));
    assert_eq!(app.info().round.unwrap().number, 1);
    assert_eq!(app.info().remaining, 3000);
    assert!(app.info().paused);
}

#[test]
fn conflicting_save_during_break_cannot_latch_a_mode_restart_or_change_any_configuration() {
    let mut app = started_break();
    let before = app.info();
    let before_settings = serde_json::to_value(app.settings()).unwrap();
    let mut invalid = app.settings().clone();
    invalid.use_schedule = false;
    invalid.sound = false;
    let mut duplicate = invalid.schedules[0].clone();
    duplicate.id = "conflicting-plan".into();
    invalid.schedules.push(duplicate);
    let output = app.dispatch(Action::SaveSettings(invalid), at(14, 0));
    assert_eq!(output.validation_errors[0].code, "overlap");
    assert!(output.state.is_none());
    assert!(output.events.is_empty());
    assert!(output.persist_settings.is_none());
    assert_eq!(app.info(), before);
    assert_eq!(
        serde_json::to_value(app.settings()).unwrap(),
        before_settings
    );
    app.dispatch(Action::EndBreak, at(10, 0));
    assert_eq!(app.info().round.unwrap().number, 2);
}

#[test]
fn editing_only_default_lengths_keeps_unadjusted_current_rounds_and_updates_the_next() {
    for phase in [Mode::Work, Mode::Remind, Mode::Break] {
        let mut original = settings();
        original.schedules[0].periods[0].overrides.clear();
        let mut app = TimerApp::new_at(original, fastrand::Rng::with_seed(42), at(9, 0));
        if phase != Mode::Work {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 3300,
                    idle_secs: 0,
                },
                at(9, 55),
            );
        }
        if phase == Mode::Break {
            app.dispatch(Action::StartBreak, at(9, 55));
        }
        let before = app.info();
        let mut changed = app.settings().clone();
        changed.schedules[0].periods[0].work_minutes = 60;
        changed.schedules[0].periods[0].break_minutes = 7;
        app.dispatch(Action::SaveSettings(changed), at(9, 55));
        assert_eq!(app.info(), before);
        if phase == Mode::Work {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: 3300,
                    idle_secs: 0,
                },
                at(10, 0),
            );
        }
        if phase != Mode::Break {
            app.dispatch(Action::StartBreak, at(10, 0));
        }
        app.dispatch(Action::EndBreak, at(10, 0));
        assert_eq!(app.info().round.unwrap().number, 2);
        assert_eq!(
            (app.info().work_seconds, app.info().break_seconds),
            (3600, 420)
        );
    }
}

#[test]
fn finishing_a_break_after_its_plan_is_disabled_or_shortened_stays_paused_idle() {
    for disable_plan in [true, false] {
        let mut app = started_break();
        app.dispatch(Action::TogglePause, at(9, 56));
        let before = app.info();
        let mut changed = app.settings().clone();
        if disable_plan {
            changed.schedules[0].enabled = false;
        } else {
            changed.schedules[0].periods[0].end = "09:55".into();
        }
        app.dispatch(Action::SaveSettings(changed), at(9, 56));
        assert_eq!(app.info(), before);
        let ended = app.dispatch(Action::EndBreak, at(10, 0));
        assert_eq!(app.info().mode, Mode::Idle);
        assert!(app.info().paused);
        assert!(ended.update_windows);
        assert!(app.info().round.is_none());
    }
}
