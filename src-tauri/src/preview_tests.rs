use std::time::Duration;

use crate::preview::PreviewMode::{Plan, Progress};
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

fn arrangement() -> Settings {
    let mut settings = Settings {
        use_schedule: true,
        idle_reset: false,
        tips: vec!["喝水".into(), "伸展".into()],
        ..Settings::default()
    };
    settings.schedules = serde_json::from_value(serde_json::json!([{
        "id":"day", "name":"工作日", "enabled":true,
        "repeat":{"kind":"weekly","weekdays":[1,2,3,4,5]},
        "periods":[
            {"id":"morning","start":"09:00","end":"12:00","work_minutes":55,"break_minutes":5,
             "overrides":[{"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼"},{"round":2,"content":"深蹲"}]},
            {"id":"afternoon","start":"13:30","end":"18:00","work_minutes":60,"break_minutes":5}
        ]
    }])).unwrap();
    settings
}

#[test]
fn preview_reads_unsaved_draft_periods_gaps_and_rounds_without_applying_it() {
    let app = TimerApp::new_at(arrangement(), fastrand::Rng::with_seed(42), at(8, 0));
    let before = app.info();
    let saved = serde_json::to_value(app.settings()).unwrap();
    let mut draft = app.settings().clone();
    draft.schedules[0].name = "还没保存 <工作日>".into();
    let preview = app.preview_with_mode(&draft, at(8, 0), Plan).unwrap();
    assert_eq!(preview.schedules[0].name, "还没保存 <工作日>");
    let morning = &preview.schedules[0].periods[0];
    assert_eq!(morning.rounds[0].number, 1);
    assert_eq!(
        (
            morning.rounds[0].work_start,
            morning.rounds[0].break_start,
            morning.rounds[0].break_end
        ),
        (32400, 35400, 36000)
    );
    assert_eq!(morning.rounds[0].content.as_deref(), Some("爬楼"));
    assert_eq!(morning.rounds[1].content.as_deref(), Some("深蹲"));
    assert_eq!(preview.schedules[0].gaps[0].start, 43200);
    assert_eq!(preview.schedules[0].gaps[0].end, 48600);
    assert_eq!(preview.schedules[0].periods[1].rounds[0].break_start, 52200);
    assert_eq!(app.info(), before);
    assert_eq!(serde_json::to_value(app.settings()).unwrap(), saved);
}

#[test]
fn preview_lists_all_draft_rounds_and_only_upcoming_live_rounds() {
    let mut settings = arrangement();
    let morning = &mut settings.schedules[0].periods[0];
    morning.overrides = serde_json::from_value(serde_json::json!([
        {"round":1,"content":"提肛"}, {"round":2,"content":"俯卧撑100个"},
        {"round":3,"content":"提肛"}
    ]))
    .unwrap();
    let afternoon = &mut settings.schedules[0].periods[1];
    afternoon.end = "18:30".into();
    afternoon.work_minutes = 55;
    afternoon.overrides = serde_json::from_value(serde_json::json!([
        {"round":1,"content":"提肛"}, {"round":2,"content":"俯卧撑100个"},
        {"round":3,"content":"提肛"}, {"round":4,"content":"俯卧撑100个"},
        {"round":5,"content":"核心"}
    ]))
    .unwrap();
    let app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(17, 18));
    let preview = app.preview(app.settings(), at(17, 18)).unwrap();
    assert_eq!(
        preview
            .current
            .as_ref()
            .unwrap()
            .state
            .round
            .as_ref()
            .unwrap()
            .number,
        4
    );
    assert_eq!(preview.current.as_ref().unwrap().break_start, 62700);
    let periods = &preview.schedules[0].periods;
    assert!(periods[0].rounds.is_empty());
    assert_eq!(periods[1].rounds.len(), 1);
    assert_eq!(periods[1].rounds[0].number, 5);
    assert_eq!(periods[1].rounds[0].break_start, 66300);

    let preview = app
        .preview_with_mode(app.settings(), at(17, 18), Plan)
        .unwrap();
    assert!(preview.current.is_none());
    let periods = &preview.schedules[0].periods;
    assert_eq!(periods[0].rounds.len(), 3);
    assert_eq!(periods[0].rounds[2].break_end, 43200);
    assert_eq!(periods[1].rounds.len(), 5);
    assert_eq!(periods[1].rounds[4].break_end, 66600);
    assert_eq!(periods[1].rounds[4].content.as_deref(), Some("核心"));

    let capacities = crate::schedule_period_capacities(app.settings().schedules[0].periods.clone());
    for (capacity, expected) in capacities.iter().zip([3, 5]) {
        let capacity = capacity.as_ref().unwrap();
        assert_eq!(capacity.complete_rounds, expected);
        assert_eq!(capacity.last_adjustable_round, expected);
        assert_eq!(capacity.remaining_minutes, 0);
    }
}

#[test]
fn save_rejects_an_adjustment_whose_round_exceeds_the_execution_period() {
    let mut app = TimerApp::new_at(arrangement(), fastrand::Rng::with_seed(42), at(9, 0));
    let mut draft = app.settings().clone();
    let morning = &mut draft.schedules[0].periods[0];
    morning
        .overrides
        .push(serde_json::from_value(serde_json::json!({"round":4,"content":"喝水"})).unwrap());
    let before = app.info();
    let output = app.dispatch(Action::SaveSettings(draft), at(9, 0));
    assert!(
        output.validation_errors.iter().any(|issue| {
            issue.period_id.as_deref() == Some("morning")
                && issue.field == "overrides.2.round"
                && issue.code == "round_outside_period"
        }),
        "{:?}",
        output.validation_errors
    );
    assert!(output.persist_settings.is_none());
    assert_eq!(app.info(), before);
    assert_eq!(app.settings().schedules[0].periods[0].overrides.len(), 2);
}

#[test]
fn a_shorter_adjusted_round_can_finish_at_day_end_but_not_after() {
    let mut settings = arrangement();
    let afternoon = &mut settings.schedules[0].periods[1];
    afternoon.start = "23:00".into();
    afternoon.end = "24:00".into();
    afternoon.work_minutes = 40;
    let mut app = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(20, 0));
    let preview = app
        .preview_with_mode(app.settings(), at(20, 0), Plan)
        .unwrap();
    assert_eq!(preview.schedules[0].periods[1].rounds.len(), 1);
    let capacity = app.settings().schedules[0].periods[1]
        .round_capacity()
        .unwrap();
    assert_eq!(capacity.complete_rounds, 1);
    assert_eq!(capacity.last_adjustable_round, 2);
    assert_eq!(capacity.remaining_minutes, 15);

    let mut nearly_full = app.settings().schedules[0].periods[1].clone();
    nearly_full.end = "23:46".into();
    let capacity = nearly_full.round_capacity().unwrap();
    assert_eq!(capacity.last_adjustable_round, 1);
    assert_eq!(capacity.remaining_minutes, 1);

    let mut draft = app.settings().clone();
    draft.schedules[0].periods[1].overrides =
        serde_json::from_value(serde_json::json!([{ "round": 2, "work_minutes": 10 }])).unwrap();
    let preview = app.preview_with_mode(&draft, at(20, 0), Plan).unwrap();
    let rounds = &preview.schedules[0].periods[1].rounds;
    assert_eq!(rounds.len(), 2);
    assert_eq!(rounds[1].break_end, 86400);
    assert!(app
        .dispatch(Action::SaveSettings(draft.clone()), at(20, 0))
        .persist_settings
        .is_some());

    draft.schedules[0].periods[1].overrides[0].break_minutes = Some(6);
    let preview = app.preview_with_mode(&draft, at(20, 0), Plan).unwrap();
    assert_eq!(preview.schedules[0].periods[1].rounds.len(), 1);
    let errors = app
        .dispatch(Action::SaveSettings(draft), at(20, 0))
        .validation_errors;
    assert_eq!(errors[0].field, "overrides.0.round");
    assert_eq!(errors[0].code, "round_outside_period");
}

#[test]
fn save_requires_at_least_one_complete_round_in_an_execution_period() {
    let mut app = TimerApp::new_at(arrangement(), fastrand::Rng::with_seed(42), at(8, 0));
    let mut draft = app.settings().clone();
    let period = &mut draft.schedules[0].periods[0];
    period.end = "09:59".into();
    period.overrides.clear();
    let output = app.dispatch(Action::SaveSettings(draft), at(8, 0));
    assert_eq!(output.validation_errors[0].field, "work_minutes");
    assert_eq!(
        output.validation_errors[0].code,
        "period_round_does_not_fit"
    );
    assert!(output.persist_settings.is_none());
}

#[test]
fn a_legacy_period_with_an_out_of_range_adjustment_can_be_loaded_and_corrected() {
    let mut legacy = arrangement();
    legacy.schedules[0].periods[0].overrides[1].round = 4;
    let path = std::env::temp_dir().join(format!(
        "sit-break-legacy-rounds-{}.json",
        fastrand::u64(..)
    ));
    legacy.write_to(&path).unwrap();
    let mut app = TimerApp::load_from(&path, fastrand::Rng::with_seed(42));
    assert_eq!(app.settings().schedules, legacy.schedules);

    let output = app.dispatch(Action::ApplyInterval(30), at(8, 0));
    output.persist_settings.unwrap().write_to(&path).unwrap();
    let mut corrected = Settings::load_from(&path);
    assert_eq!(corrected.work_minutes, 30);
    assert_eq!(corrected.schedules, legacy.schedules);

    corrected.schedules[0].periods[0].overrides[1].round = 3;
    let output = app.dispatch(Action::SaveSettings(corrected), at(8, 0));
    output.persist_settings.unwrap().save_to(&path).unwrap();
    assert_eq!(
        Settings::load_from(&path).schedules[0].periods[0].overrides[1].round,
        3
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn preview_distinguishes_guidance_only_rounds_from_random_reminders_without_selecting_them() {
    let app = TimerApp::new_at(arrangement(), fastrand::Rng::with_seed(42), at(8, 0));
    let mut draft = serde_json::to_value(app.settings()).unwrap();
    let guidance = serde_json::json!({"kind":"video","source":{"type":"local","id":"local-00000000000000000000000000000001"}});
    draft["schedules"][0]["periods"][0]["overrides"] = serde_json::json!([
        {"round":1,"guidance":guidance},
        {"round":2}
    ]);
    let preview = app
        .preview_with_mode(&serde_json::from_value(draft).unwrap(), at(8, 0), Plan)
        .unwrap();
    let rounds = &preview.schedules[0].periods[0].rounds;
    let state = serde_json::to_value(rounds).unwrap();
    assert_eq!(state[0]["guidance"], guidance);
    assert!(rounds[0].content.is_none());
    assert!(state[1]["guidance"].is_null());
    assert!(rounds[1].content.is_none());
    assert_eq!(app.info().mode, Mode::Idle);
}

#[test]
fn progress_uses_saved_times_even_with_unsaved_edits_and_snoozing() {
    let mut app = TimerApp::new_at(arrangement(), fastrand::Rng::with_seed(42), at(9, 50));
    let mut draft = app.settings().clone();
    draft.schedules[0].periods[0].overrides[0].break_minutes = Some(5);
    let plan = app.preview_with_mode(&draft, at(9, 50), Plan).unwrap();
    assert_eq!(plan.schedules[0].periods[0].rounds[1].work_start, 35700);
    let preview = app.preview(&draft, at(9, 50)).unwrap();
    assert_eq!(preview.current.as_ref().unwrap().break_end, 36000);
    assert_eq!(preview.schedules[0].periods[0].rounds[0].work_start, 36000);

    app.dispatch(Action::Postpone, at(9, 50));
    app.dispatch(Action::TogglePause, at(9, 53));
    draft.schedules.clear();
    let paused = app.preview(&draft, at(9, 53)).unwrap();
    assert_eq!(paused.schedules, preview.schedules);
    let current = paused.current.unwrap();
    assert!(current.state.paused);
    assert_eq!(current.break_start, 35400);
    assert_eq!(current.break_end, 36000);
}

#[test]
fn progress_in_a_quiet_gap_lists_only_later_rounds() {
    let app = TimerApp::new_at(arrangement(), fastrand::Rng::with_seed(42), at(12, 30));
    assert_eq!(app.info().mode, Mode::Idle);
    let preview = app.preview(app.settings(), at(12, 30)).unwrap();
    assert!(preview.current.is_none());
    let periods = &preview.schedules[0].periods;
    assert!(periods[0].rounds.is_empty());
    assert_eq!(periods[1].rounds[0].work_start, 48600);
}

#[test]
fn repeated_preview_does_not_change_future_random_tips_or_the_saved_configuration() {
    let settings = arrangement();
    let mut inspected = TimerApp::new_at(settings.clone(), fastrand::Rng::with_seed(42), at(9, 0));
    let mut untouched = TimerApp::new_at(settings, fastrand::Rng::with_seed(42), at(9, 0));
    for _ in 0..3 {
        for mode in [Plan, Progress] {
            inspected
                .preview_with_mode(inspected.settings(), at(9, 0), mode)
                .unwrap();
        }
    }
    for (work, time) in [(3000, at(9, 50)), (3300, at(10, 50))] {
        for app in [&mut inspected, &mut untouched] {
            app.dispatch(
                Action::Tick {
                    elapsed_secs: work,
                    idle_secs: 0,
                },
                time.clone(),
            );
            app.dispatch(Action::StartBreak, time.clone());
            app.dispatch(Action::EndBreak, time.clone());
        }
    }
    assert_eq!(inspected.info(), untouched.info());
    assert_eq!(
        serde_json::to_value(inspected.settings()).unwrap(),
        serde_json::to_value(untouched.settings()).unwrap()
    );
}

#[test]
fn invalid_draft_is_rejected_without_affecting_progress_preview_or_live_state() {
    let app = TimerApp::new_at(arrangement(), fastrand::Rng::with_seed(42), at(9, 55));
    let before = app.info();
    let mut invalid = app.settings().clone();
    invalid.schedules[0].periods[0].end = "08:00".into();
    assert!(app.preview_with_mode(&invalid, at(9, 55), Plan).is_err());
    let preview = app.preview(&invalid, at(9, 55)).unwrap();
    assert_eq!(preview.current.unwrap().state, before);
    assert_eq!(app.info(), before);
}
