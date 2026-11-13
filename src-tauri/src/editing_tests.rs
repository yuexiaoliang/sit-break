use crate::timer::{Action, LocalTime, Mode, TimerApp, TimerTime};
use crate::Settings;

fn at(hour: u32, minute: u32) -> TimerTime {
    TimerTime::at_local(LocalTime {
        date: "2026-10-14".into(),
        seconds_of_day: hour * 3600 + minute * 60,
    })
    .unwrap()
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
                {"round":1,"work_minutes":50,"break_minutes":10,"content":"爬楼","guidance":{"kind":"video","source":{"type":"local","id":"local-00000000000000000000000000000001"}}},
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
fn invalid_round_guidance_save_leaves_the_current_round_and_settings_intact() {
    let mut app = TimerApp::new_at(settings(), fastrand::Rng::with_seed(42), at(9, 0));
    let before = app.info();
    let original = serde_json::to_value(app.settings()).unwrap();
    let mut changed = original.clone();
    changed["schedules"][0]["periods"][0]["overrides"][0]["guidance"] =
        serde_json::json!({"kind":"video","source":{"type":"local","id":"missing"}});
    let saved = app.dispatch(
        Action::SaveSettings(serde_json::from_value(changed).unwrap()),
        at(9, 1),
    );
    assert!(saved
        .validation_errors
        .iter()
        .any(|error| error.field == "overrides.0.guidance" && error.code == "guidance_invalid"));
    assert!(saved.persist_settings.is_none());
    assert_eq!(app.info(), before);
    assert_eq!(serde_json::to_value(app.settings()).unwrap(), original);
}

#[test]
fn conflicting_save_during_break_leaves_the_current_state_and_settings_intact() {
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
fn switching_modes_uses_the_selected_clock_and_preserves_manual_pause() {
    let mut app = started_break();
    app.dispatch(Action::TogglePause, at(9, 56));
    let mut draft = app.settings().clone();
    draft.use_schedule = false;
    draft.work_minutes = 23;
    draft.break_minutes = 4;
    app.dispatch(Action::SaveSettings(draft), at(9, 56));
    assert_eq!(app.info().mode, Mode::Work);
    assert!(app.info().round.is_none());
    assert_eq!(app.info().remaining, 23 * 60);
    assert!(app.info().paused);

    let mut draft = app.settings().clone();
    draft.use_schedule = true;
    app.dispatch(Action::SaveSettings(draft), at(10, 7));
    assert_eq!(app.info().round.unwrap().number, 2);
    assert_eq!(app.info().remaining, 13 * 60);
    assert_eq!(app.info().tip.as_deref(), Some("深蹲"));
    assert!(app.info().paused);
}

#[test]
fn disabling_a_schedule_closes_its_break_and_reenabling_aligns_to_the_clock() {
    let mut app = started_break();
    let mut draft = app.settings().clone();
    draft.schedules[0].enabled = false;
    let disabled = app.dispatch(Action::SaveSettings(draft), at(9, 56));
    assert_eq!(app.info().mode, Mode::Idle);
    assert!(!app.reminder_visible());
    assert!(disabled.update_windows);

    let mut draft = app.settings().clone();
    draft.schedules[0].enabled = true;
    app.dispatch(Action::SaveSettings(draft), at(10, 11));
    assert_eq!(app.info().round.unwrap().number, 2);
    assert_eq!(app.info().remaining, 9 * 60);
}

#[test]
fn saving_new_times_preserves_the_active_break_then_rejoins_the_updated_plan() {
    let mut app = started_break();
    let before = app.info();
    let mut draft = app.settings().clone();
    let round = &mut draft.schedules[0].periods[0].overrides[0];
    round.work_minutes = Some(45);
    round.break_minutes = Some(10);
    round.content = Some("下次显示".into());
    round.guidance = None;

    app.dispatch(Action::SaveSettings(draft), at(9, 57));
    assert_eq!(app.info().round, before.round);
    assert_eq!(app.info().mode, Mode::Break);
    assert_eq!(app.info().remaining, 8 * 60);
    assert_eq!(app.info().tip, before.tip);
    assert_eq!(app.info().guidance, before.guidance);

    let boundary = app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        at(10, 5),
    );
    assert!(boundary.update_windows);
    assert_eq!(app.info().mode, Mode::Work);
    assert_eq!(app.info().remaining, 10 * 60);
    assert_eq!(app.info().round.unwrap().number, 2);
    assert_eq!(app.info().tip.as_deref(), Some("深蹲"));

    let next_day = TimerTime::at_local(LocalTime {
        date: "2026-10-15".into(),
        seconds_of_day: 9 * 3600,
    })
    .unwrap();
    app.dispatch(
        Action::Tick {
            elapsed_secs: 1,
            idle_secs: 0,
        },
        next_day,
    );
    assert_eq!(app.info().tip.as_deref(), Some("下次显示"));
    assert!(app.info().guidance.is_none());
}
