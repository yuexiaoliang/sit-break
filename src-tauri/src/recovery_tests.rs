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
fn reopening_in_the_same_period_restores_round_two_without_counting_the_closed_time() {
    let (settings_path, progress_path, dir) = paths();
    let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 0, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        time(9, 50, 3000),
    );
    app.dispatch(Action::StartBreak, time(9, 55, 3300));
    app.dispatch(Action::EndBreak, time(9, 55, 3300));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 125,
            idle_secs: 0,
        },
        time(9, 57, 3425),
    );
    let before = app.info();
    assert_eq!(before.round.as_ref().unwrap().number, 2);
    assert_eq!(before.remaining, 3175);
    app.settings().save_to(&settings_path).unwrap();
    app.save_progress_to(&progress_path, time(9, 57, 3425))
        .unwrap();
    let mut reopened = TimerApp::restore_from(
        Settings::load_from(&settings_path),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(10, 50, 0),
    );
    assert_eq!(reopened.info(), before);
    let output = reopened.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        time(10, 50, 1),
    );
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
    app.dispatch(
        Action::Tick {
            elapsed_secs: 3000,
            idle_secs: 0,
        },
        time(9, 50, 3000),
    );
    app.dispatch(Action::TogglePause, time(9, 50, 3000));
    app.settings().save_to(&settings_path).unwrap();
    app.save_progress_to(&progress_path, time(9, 50, 3000))
        .unwrap();
    for (now, mode, work_seconds) in [
        (time(12, 30, 0), Mode::Idle, 0),
        (time(13, 30, 0), Mode::Work, 3600),
        (
            {
                let mut next = time(9, 0, 0);
                next.local.as_mut().unwrap().date = "2026-10-15".into();
                next
            },
            Mode::Work,
            3000,
        ),
    ] {
        let mut reopened = TimerApp::restore_from(
            Settings::load_from(&settings_path),
            &progress_path,
            fastrand::Rng::with_seed(42),
            now.clone(),
        );
        assert!(reopened.info().paused);
        assert_eq!(reopened.info().mode, mode);
        assert_eq!(reopened.info().work_seconds, work_seconds);
        let output = reopened.dispatch(
            Action::Tick {
                elapsed_secs: 9000,
                idle_secs: 9000,
            },
            now,
        );
        assert!(output.events.is_empty());
        assert!(reopened.info().paused);
        if mode == Mode::Work {
            assert_eq!(reopened.info().round.unwrap().number, 1);
        }
    }
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
            time(10, 0, 0),
        );
        assert_eq!(reopened.info().remaining, 3000);
        assert!(!reopened.info().paused);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reopening_does_not_count_closed_time_toward_the_idle_threshold() {
    let (_, progress_path, dir) = paths();
    let mut configured = settings();
    configured.idle_reset = true;
    configured.schedules[0].periods[0].overrides[0].work_minutes = Some(1);
    let mut app = TimerApp::new_at(configured, fastrand::Rng::with_seed(42), time(9, 0, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        time(9, 1, 60),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 120,
            idle_secs: 120,
        },
        time(9, 3, 180),
    );
    assert_eq!(app.info().mode, Mode::Remind);
    app.save_progress_to(&progress_path, time(9, 3, 180))
        .unwrap();
    let mut reopened = TimerApp::restore_from(
        app.settings().clone(),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(10, 0, 0),
    );
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 3541,
        },
        time(10, 0, 1),
    );
    assert_eq!(reopened.info().mode, Mode::Remind);
    assert_eq!(reopened.info().round.unwrap().number, 1);
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 478,
            idle_secs: 4019,
        },
        time(10, 8, 479),
    );
    assert_eq!(reopened.info().mode, Mode::Remind);
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 4020,
        },
        time(10, 8, 480),
    );
    assert_eq!(reopened.info().mode, Mode::Work);
    assert_eq!(reopened.info().round.unwrap().number, 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn restoring_rebuilds_remaining_grace_on_the_new_process_clock() {
    let (_, progress_path, dir) = paths();
    let mut configured = settings();
    configured.idle_reset = true;
    let period = &mut configured.schedules[0].periods[0];
    period.overrides[0].work_minutes = Some(1);
    period.overrides[0].break_minutes = Some(1);
    period.overrides[1].work_minutes = Some(1);
    period.overrides[1].break_minutes = Some(10);
    let mut app = TimerApp::new_at(configured, fastrand::Rng::with_seed(42), time(9, 0, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        time(9, 1, 60),
    );
    app.dispatch(Action::StartBreak, time(9, 1, 60));
    app.dispatch(Action::EndBreak, time(9, 1, 60));
    let mut saved_time = time(9, 1, 80);
    saved_time.local.as_mut().unwrap().seconds_of_day += 20;
    app.dispatch(
        Action::Tick {
            elapsed_secs: 20,
            idle_secs: 700,
        },
        saved_time.clone(),
    );
    assert_eq!(app.info().remaining, 40);
    app.save_progress_to(&progress_path, saved_time).unwrap();
    let mut reopened = TimerApp::restore_from(
        app.settings().clone(),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(9, 2, 0),
    );
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 741,
        },
        time(9, 2, 1),
    );
    assert_eq!(reopened.info().remaining, 39);
    let mut later = time(9, 11, 580);
    later.local.as_mut().unwrap().seconds_of_day += 40;
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 1320,
        },
        later,
    );
    assert_eq!(reopened.info().remaining, 60);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn restoring_an_already_started_break_keeps_latest_mode_selection_and_pending_restart() {
    for use_schedule in [false, true] {
        let (settings_path, progress_path, dir) = paths();
        let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 0, 0));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 3000,
                idle_secs: 0,
            },
            time(9, 50, 3000),
        );
        app.dispatch(Action::StartBreak, time(9, 55, 3300));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 60,
                idle_secs: 0,
            },
            time(9, 56, 3360),
        );
        let before = app.info();
        let mut changed = app.settings().clone();
        changed.use_schedule = false;
        changed.work_minutes = 23;
        changed.break_minutes = 4;
        changed.schedules[0].periods[0].overrides[0].work_minutes = Some(40);
        changed.schedules[0].periods[0].overrides[0].break_minutes = Some(4);
        changed.schedules[0].periods[0].overrides[0].content = Some("新的首轮".into());
        app.dispatch(Action::SaveSettings(changed), time(9, 56, 3360));
        if use_schedule {
            let mut changed = app.settings().clone();
            changed.use_schedule = true;
            app.dispatch(Action::SaveSettings(changed), time(9, 56, 3360));
        }
        app.settings().save_to(&settings_path).unwrap();
        app.save_progress_to(&progress_path, time(9, 56, 3360))
            .unwrap();
        let mut reopened = TimerApp::restore_from(
            Settings::load_from(&settings_path),
            &progress_path,
            fastrand::Rng::with_seed(42),
            time(10, 30, 0),
        );
        assert_eq!(reopened.info(), before);
        reopened.dispatch(Action::EndBreak, time(10, 30, 0));
        if use_schedule {
            assert_eq!(reopened.info().round.unwrap().number, 1);
            assert_eq!(
                (
                    reopened.info().work_seconds,
                    reopened.info().break_seconds,
                    reopened.info().tip.as_deref()
                ),
                (2400, 240, Some("新的首轮"))
            );
        } else {
            assert!(reopened.info().round.is_none());
            assert_eq!(
                (reopened.info().work_seconds, reopened.info().break_seconds),
                (1380, 240)
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn all_current_phases_restore_their_old_snapshot_and_future_rounds_use_saved_edits() {
    for phase in [Mode::Work, Mode::Remind, Mode::Break] {
        for paused in [false, true] {
            let (settings_path, progress_path, dir) = paths();
            let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 0, 0));
            app.dispatch(
                Action::Tick {
                    elapsed_secs: if phase == Mode::Work { 60 } else { 3000 },
                    idle_secs: 0,
                },
                time(9, 50, 3000),
            );
            if phase == Mode::Break {
                app.dispatch(Action::StartBreak, time(9, 55, 3300));
                app.dispatch(
                    Action::Tick {
                        elapsed_secs: 60,
                        idle_secs: 0,
                    },
                    time(9, 56, 3360),
                );
            }
            if paused {
                app.dispatch(Action::TogglePause, time(9, 56, 3360));
            }
            let before = app.info();
            let mut edited = app.settings().clone();
            let period = &mut edited.schedules[0].periods[0];
            period.work_minutes = 60;
            period.break_minutes = 7;
            period.overrides[0].work_minutes = Some(40);
            period.overrides[0].break_minutes = Some(4);
            period.overrides[0].content = Some("下一次的爬楼".into());
            period.overrides[1].work_minutes = Some(30);
            period.overrides[1].break_minutes = Some(8);
            period.overrides[1].content = Some("新深蹲\n再喝水".into());
            edited.sound = false;
            app.dispatch(Action::SaveSettings(edited), time(9, 56, 3360));
            app.settings().save_to(&settings_path).unwrap();
            app.save_progress_to(&progress_path, time(9, 56, 3360))
                .unwrap();
            let mut reopened = TimerApp::restore_from(
                Settings::load_from(&settings_path),
                &progress_path,
                fastrand::Rng::with_seed(42),
                time(10, 30, 0),
            );
            assert_eq!(reopened.info(), before);
            assert_eq!(reopened.info().tip.as_deref(), Some("爬楼"));
            if paused {
                reopened.dispatch(Action::TogglePause, time(10, 30, 0));
            }
            if phase == Mode::Work {
                reopened.dispatch(
                    Action::Tick {
                        elapsed_secs: before.remaining,
                        idle_secs: 0,
                    },
                    time(10, 35, 300),
                );
            }
            if phase != Mode::Break {
                reopened.dispatch(Action::StartBreak, time(10, 35, 300));
            }
            let remaining = reopened.info().remaining;
            reopened.dispatch(
                Action::Tick {
                    elapsed_secs: remaining,
                    idle_secs: 0,
                },
                time(10, 45, 900),
            );
            assert_eq!(reopened.info().round.unwrap().number, 2);
            assert_eq!(
                (
                    reopened.info().work_seconds,
                    reopened.info().break_seconds,
                    reopened.info().tip.as_deref()
                ),
                (1800, 480, Some("新深蹲\n再喝水"))
            );
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn disabled_deleted_shortened_or_changed_period_identity_discards_old_progress_without_clearing_pause(
) {
    for change in [
        "disabled",
        "deleted",
        "shortened",
        "new-identity",
        "simple-mode",
    ] {
        let (settings_path, progress_path, dir) = paths();
        let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), time(9, 0, 0));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 3000,
                idle_secs: 0,
            },
            time(9, 50, 3000),
        );
        app.dispatch(Action::TogglePause, time(9, 50, 3000));
        app.save_progress_to(&progress_path, time(9, 50, 3000))
            .unwrap();
        let mut edited = app.settings().clone();
        match change {
            "disabled" => edited.schedules[0].enabled = false,
            "deleted" => {
                edited.schedules[0].periods.remove(0);
            }
            "shortened" => edited.schedules[0].periods[0].end = "10:00".into(),
            "new-identity" => edited.schedules[0].periods[0].id = "new-period".into(),
            "simple-mode" => edited.use_schedule = false,
            _ => unreachable!(),
        }
        edited.save_to(&settings_path).unwrap();
        let mut reopened = TimerApp::restore_from(
            Settings::load_from(&settings_path),
            &progress_path,
            fastrand::Rng::with_seed(42),
            time(10, 30, 0),
        );
        assert!(reopened.info().paused);
        assert_ne!(reopened.info().mode, Mode::Remind);
        if change == "new-identity" {
            assert_eq!(reopened.info().round.unwrap().period_id, "new-period");
            assert_eq!(reopened.info().remaining, 3000);
        } else if change == "simple-mode" {
            assert!(reopened.info().round.is_none());
            assert_eq!(reopened.info().remaining, 2700);
        } else {
            assert_eq!(reopened.info().mode, Mode::Idle);
        }
        let output = reopened.dispatch(
            Action::Tick {
                elapsed_secs: 9000,
                idle_secs: 9000,
            },
            time(10, 30, 1),
        );
        assert!(output.events.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn a_consumed_away_episode_survives_reopening_and_only_new_input_resumes_short_work() {
    let (_, progress_path, dir) = paths();
    let mut configured = settings();
    configured.idle_reset = true;
    let period = &mut configured.schedules[0].periods[0];
    period.overrides[0].work_minutes = Some(1);
    period.overrides[0].break_minutes = Some(1);
    period.overrides[1].work_minutes = Some(1);
    period.overrides[1].break_minutes = Some(10);
    let mut app = TimerApp::new_at(configured, fastrand::Rng::with_seed(42), time(9, 0, 0));
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        time(9, 1, 60),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 60,
        },
        time(9, 2, 120),
    );
    assert_eq!(app.info().round.unwrap().number, 2);
    app.save_progress_to(&progress_path, time(9, 2, 120))
        .unwrap();
    let mut reopened = TimerApp::restore_from(
        app.settings().clone(),
        &progress_path,
        fastrand::Rng::with_seed(42),
        time(9, 30, 0),
    );
    let output = reopened.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 1800,
        },
        time(9, 31, 60),
    );
    assert_eq!(reopened.info().mode, Mode::Work);
    assert_eq!(reopened.info().remaining, 60);
    assert_eq!(reopened.info().round.unwrap().number, 2);
    assert!(output.events.is_empty());
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        time(9, 31, 61),
    );
    assert_eq!(reopened.info().remaining, 59);
    reopened.dispatch(
        Action::Tick {
            elapsed_secs: 59,
            idle_secs: 59,
        },
        time(9, 32, 120),
    );
    assert_eq!(reopened.info().mode, Mode::Remind);
    assert_eq!(reopened.info().round.unwrap().number, 2);
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

#[test]
fn input_while_closed_starts_a_new_idle_episode_even_if_the_raw_counter_exceeds_the_old_value() {
    for paused in [false, true] {
        let (_, progress_path, dir) = paths();
        let mut configured = settings();
        configured.idle_reset = true;
        let period = &mut configured.schedules[0].periods[0];
        period.overrides[0].work_minutes = Some(1);
        period.overrides[0].break_minutes = Some(1);
        period.overrides[1].work_minutes = Some(1);
        period.overrides[1].break_minutes = Some(10);
        let mut app = TimerApp::new_at(configured, fastrand::Rng::with_seed(42), time(9, 0, 0));
        app.dispatch(
            Action::Tick {
                elapsed_secs: 60,
                idle_secs: 0,
            },
            time(9, 1, 60),
        );
        app.dispatch(
            Action::Tick {
                elapsed_secs: 60,
                idle_secs: 60,
            },
            time(9, 2, 120),
        );
        if paused {
            app.dispatch(Action::TogglePause, time(9, 2, 120));
        }
        app.save_progress_to(&progress_path, time(9, 2, 120))
            .unwrap();
        let mut reopened = TimerApp::restore_from(
            app.settings().clone(),
            &progress_path,
            fastrand::Rng::with_seed(42),
            time(9, 30, 0),
        );
        reopened.dispatch(
            Action::Tick {
                elapsed_secs: 1,
                idle_secs: 200,
            },
            time(9, 30, 1),
        );
        if paused {
            assert_eq!(reopened.info().remaining, 60);
            reopened
                .save_progress_to(&progress_path, time(9, 30, 1))
                .unwrap();
            reopened = TimerApp::restore_from(
                reopened.settings().clone(),
                &progress_path,
                fastrand::Rng::with_seed(42),
                time(9, 31, 0),
            );
            reopened.dispatch(
                Action::Tick {
                    elapsed_secs: 0,
                    idle_secs: 261,
                },
                time(9, 31, 0),
            );
            reopened.dispatch(Action::TogglePause, time(9, 31, 0));
            reopened.dispatch(
                Action::Tick {
                    elapsed_secs: 1,
                    idle_secs: 262,
                },
                time(9, 31, 1),
            );
        }
        assert_eq!(reopened.info().mode, Mode::Work);
        assert_eq!(reopened.info().remaining, 59);
        assert_eq!(reopened.info().round.unwrap().number, 2);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
