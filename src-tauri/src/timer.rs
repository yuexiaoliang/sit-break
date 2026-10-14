//! 计时应用入口。调用方提供本地日历、经过时间和用户操作，并执行返回的系统效果。
use std::time::Duration;

use serde::Serialize;

use crate::schedule::{minute_of_day, Period, Schedule, ValidationError};
use crate::{choose_tip, Settings, POSTPONE_SECS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTime {
    pub date: String,
    pub seconds_of_day: u32,
}

#[derive(Debug, Clone)]
pub struct TimerTime {
    pub monotonic: Duration,
    pub local: Option<LocalTime>,
}

impl From<Duration> for TimerTime {
    fn from(monotonic: Duration) -> Self {
        Self {
            monotonic,
            local: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Idle,
    Work,
    Remind,
    Break,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoundInfo {
    pub schedule_id: String,
    pub period_id: String,
    pub date: String,
    pub number: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimerInfo {
    pub mode: Mode,
    pub remaining: u64,
    pub paused: bool,
    pub work_seconds: u64,
    pub break_seconds: u64,
    pub round: Option<RoundInfo>,
    pub tip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderPayload {
    pub break_seconds: u64,
    pub postpone_seconds: u64,
    pub sound: bool,
    pub tip: String,
    pub sat_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerEvent {
    Reminder(ReminderPayload),
    BreakStarted { seconds: u64 },
}

#[derive(Debug, Default)]
pub struct TimerOutput {
    pub validation_errors: Vec<ValidationError>,
    pub state: Option<TimerInfo>,
    pub events: Vec<TimerEvent>,
    pub update_windows: bool,
    pub update_tray: bool,
    pub persist_settings: Option<Settings>,
    pub settings_changed: bool,
}

pub enum Action {
    Tick { elapsed_secs: u64, idle_secs: u64 },
    StartBreak,
    TogglePause,
    Reset,
    Postpone,
    EndBreak,
    ApplyInterval(u32),
    ApplyBreak(u32),
    SaveSettings(Settings),
}

pub struct TimerApp {
    info: TimerInfo,
    settings: Settings,
    break_ended_at: Option<Duration>,
    tip_rng: fastrand::Rng,
    previous_tip: Option<String>,
    // 日程已在本次持续离开中结束一轮，下一轮等待重新操作电脑。
    schedule_idle_consumed: bool,
    last_idle_secs: Option<u64>,
    restart_after_break: bool,
}

impl TimerApp {
    pub fn load_from(path: &std::path::Path, tip_rng: fastrand::Rng) -> Self {
        Self::new(Settings::load_from(path), tip_rng)
    }

    pub fn new(settings: Settings, tip_rng: fastrand::Rng) -> Self {
        Self::new_at(settings, tip_rng, Duration::ZERO)
    }

    pub fn new_at(settings: Settings, tip_rng: fastrand::Rng, now: impl Into<TimerTime>) -> Self {
        let mut app = Self {
            info: TimerInfo {
                mode: Mode::Idle,
                remaining: 0,
                paused: false,
                work_seconds: 0,
                break_seconds: 0,
                round: None,
                tip: None,
            },
            settings,
            break_ended_at: None,
            tip_rng,
            previous_tip: None,
            schedule_idle_consumed: false,
            last_idle_secs: None,
            restart_after_break: false,
        };
        let now = now.into();
        if app.settings.use_schedule {
            app.reconcile_period(now.local.as_ref());
        } else {
            app.start_simple_work();
        }
        app
    }

    pub fn info(&self) -> TimerInfo {
        self.info.clone()
    }
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn preview(
        &self,
        draft: &Settings,
        now: impl Into<TimerTime>,
    ) -> Result<crate::preview::SchedulePreview, Vec<ValidationError>> {
        let errors = draft.validate();
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(crate::preview::build(
            draft,
            &self.settings,
            &self.info,
            now.into().local.as_ref(),
            self.restart_after_break,
        ))
    }

    pub fn dispatch(&mut self, action: Action, now: impl Into<TimerTime>) -> TimerOutput {
        let now = now.into();
        let mut output = TimerOutput::default();
        // 无效保存优先返回，不让同一调用里的时间输入改变运行进度。
        if let Action::SaveSettings(settings) = &action {
            output.validation_errors = settings.validate();
            if !output.validation_errors.is_empty() {
                return output;
            }
        }
        let started = if matches!(action, Action::SaveSettings(_)) {
            false
        } else {
            let before = self.info.mode;
            let started = self.reconcile_period(now.local.as_ref());
            output.update_windows = before != self.info.mode;
            started
        };
        match action {
            Action::Tick {
                elapsed_secs,
                idle_secs,
            } => {
                if !self.info.paused && !started {
                    if self.settings.use_schedule {
                        if idle_secs == 0
                            || self
                                .last_idle_secs
                                .is_some_and(|previous| idle_secs < previous)
                        {
                            self.schedule_idle_consumed = false;
                        }
                        self.last_idle_secs = Some(idle_secs);
                    }
                    let idle_limit = if self.settings.idle_reset {
                        self.info.break_seconds
                    } else {
                        u64::MAX
                    };
                    let away_long_enough =
                        self.settings.idle_reset && idle_secs >= self.info.break_seconds;
                    match self.info.mode {
                        Mode::Idle => {}
                        Mode::Work => {
                            let in_grace = self.break_ended_at.is_some_and(|ended| {
                                now.monotonic.saturating_sub(ended)
                                    < Duration::from_secs(idle_limit)
                            });
                            let consumed_away = self.settings.use_schedule
                                && self.settings.idle_reset
                                && self.schedule_idle_consumed;
                            if consumed_away || (!in_grace && away_long_enough) {
                                self.info.remaining = self.info.work_seconds;
                            } else {
                                self.info.remaining =
                                    self.info.remaining.saturating_sub(elapsed_secs);
                            }
                            if self.info.remaining == 0 {
                                self.info.mode = Mode::Remind;
                                if !self.settings.use_schedule || self.info.tip.is_none() {
                                    let tip = choose_tip(
                                        &self.settings,
                                        self.previous_tip.as_deref(),
                                        &mut self.tip_rng,
                                    );
                                    self.previous_tip = Some(tip.clone());
                                    self.info.tip = Some(tip);
                                }
                                output.events.push(TimerEvent::Reminder(ReminderPayload {
                                    break_seconds: self.info.break_seconds,
                                    postpone_seconds: self.postpone_seconds(),
                                    sound: self.settings.sound,
                                    tip: self.info.tip.clone().unwrap_or_default(),
                                    sat_minutes: (self.info.work_seconds / 60) as u32,
                                }));
                                output.update_windows = true;
                            }
                        }
                        Mode::Break => {
                            self.info.remaining = self.info.remaining.saturating_sub(elapsed_secs);
                            if self.info.remaining == 0 {
                                let consumed_away = self.settings.use_schedule && away_long_enough;
                                self.finish_round(&now);
                                self.schedule_idle_consumed |= consumed_away;
                                output.update_windows = true;
                            }
                        }
                        Mode::Remind => {
                            if away_long_enough
                                && (!self.settings.use_schedule || !self.schedule_idle_consumed)
                            {
                                self.schedule_idle_consumed = self.settings.use_schedule;
                                self.finish_round(&now);
                                output.update_windows = true;
                            }
                        }
                    }
                }
            }
            Action::StartBreak => {
                if self.info.mode == Mode::Remind {
                    self.info.mode = Mode::Break;
                    self.info.remaining = self.info.break_seconds;
                    output.events.push(TimerEvent::BreakStarted {
                        seconds: self.info.remaining,
                    });
                    output.update_windows = true;
                }
            }
            Action::TogglePause => self.info.paused = !self.info.paused,
            Action::Reset => {
                if self.settings.use_schedule {
                    if let Some((schedule, period)) = self.current_period(now.local.as_ref()) {
                        let date = &now.local.as_ref().unwrap().date;
                        let same = self.info.round.as_ref().is_some_and(|round| {
                            round.schedule_id == schedule.id
                                && round.period_id == period.id
                                && round.date == *date
                        });
                        if same && !self.restart_after_break {
                            self.info.mode = Mode::Work;
                            self.info.remaining = self.info.work_seconds;
                        } else {
                            self.start_scheduled_round(&schedule, &period, date, 1);
                        }
                    } else {
                        self.set_idle();
                    }
                } else {
                    self.start_simple_work();
                }
                self.restart_after_break = false;
                output.update_windows = true;
            }
            Action::Postpone => {
                if self.info.mode == Mode::Remind {
                    self.info.mode = Mode::Work;
                    self.info.remaining = self.postpone_seconds();
                    output.update_windows = true;
                }
            }
            Action::EndBreak => {
                if self.info.mode == Mode::Break {
                    self.finish_round(&now);
                    output.update_windows = true;
                }
            }
            Action::ApplyInterval(minutes) => {
                if (1..=240).contains(&minutes) {
                    self.settings.work_minutes = minutes;
                    if !self.settings.use_schedule && self.info.mode != Mode::Break {
                        self.info.work_seconds = u64::from(minutes) * 60;
                        if self.info.mode == Mode::Work {
                            self.info.remaining = self.info.work_seconds;
                        }
                    }
                    output.persist_settings = Some(self.settings.clone());
                    output.settings_changed = true;
                }
            }
            Action::ApplyBreak(minutes) => {
                if (1..=60).contains(&minutes) {
                    self.settings.break_minutes = minutes;
                    if !self.settings.use_schedule && self.info.mode != Mode::Break {
                        self.info.break_seconds = u64::from(minutes) * 60;
                    }
                    output.persist_settings = Some(self.settings.clone());
                    output.settings_changed = true;
                }
            }
            Action::SaveSettings(settings) => {
                output.update_windows = self.settings.show_widget != settings.show_widget;
                let work_changed = self.settings.work_minutes != settings.work_minutes;
                let mode_changed = self.settings.use_schedule != settings.use_schedule;
                self.settings = settings;
                if self.info.mode == Mode::Break {
                    let current_invalidated = self.info.round.as_ref().is_some_and(|round| {
                        !self.current_period(now.local.as_ref()).is_some_and(
                            |(schedule, period)| {
                                round.schedule_id == schedule.id
                                    && round.period_id == period.id
                                    && now
                                        .local
                                        .as_ref()
                                        .is_some_and(|local| round.date == local.date)
                            },
                        )
                    });
                    if mode_changed || current_invalidated {
                        // 随后切回原方式或重新启用，也不恢复已关闭的安排。
                        self.restart_after_break = true;
                    }
                }
                if mode_changed && self.info.mode != Mode::Break {
                    self.set_idle();
                    if self.settings.use_schedule {
                        self.reconcile_period(now.local.as_ref());
                    } else {
                        self.start_simple_work();
                    }
                    self.break_ended_at = None;
                    output.update_windows = true;
                } else if self.settings.use_schedule {
                    let before = self.info.mode;
                    self.reconcile_period(now.local.as_ref());
                    output.update_windows |= before != self.info.mode;
                } else if self.info.mode != Mode::Break {
                    self.info.work_seconds = u64::from(self.settings.work_minutes) * 60;
                    self.info.break_seconds = u64::from(self.settings.break_minutes) * 60;
                    if self.info.mode == Mode::Work
                        && (work_changed || self.info.remaining > self.info.work_seconds)
                    {
                        self.info.remaining = self.info.work_seconds;
                    }
                }
                output.persist_settings = Some(self.settings.clone());
                output.settings_changed = true;
            }
        }
        output.state = Some(self.info());
        output.update_tray = true;
        output
    }

    fn postpone_seconds(&self) -> u64 {
        POSTPONE_SECS.min(self.info.work_seconds).max(60)
    }

    fn current_period(&self, local: Option<&LocalTime>) -> Option<(Schedule, Period)> {
        if !self.settings.use_schedule {
            return None;
        }
        let local = local?;
        self.settings
            .schedules
            .iter()
            .filter(|schedule| schedule.enabled)
            .find_map(|schedule| {
                if !schedule.repeat.applies_on(&local.date) {
                    return None;
                }
                schedule
                    .periods
                    .iter()
                    .find(|period| {
                        minute_of_day(&period.start, false)
                            .zip(minute_of_day(&period.end, true))
                            .is_some_and(|(start, end)| {
                                start * 60 <= local.seconds_of_day
                                    && local.seconds_of_day < end * 60
                            })
                    })
                    .map(|period| (schedule.clone(), period.clone()))
            })
    }

    /// 边界先于工作归零和提醒；已开始的休息保留完整快照。
    fn reconcile_period(&mut self, local: Option<&LocalTime>) -> bool {
        if !self.settings.use_schedule || self.info.mode == Mode::Break {
            return false;
        }
        match self.current_period(local) {
            Some((schedule, period)) => {
                let local = local.unwrap();
                let same = self.info.round.as_ref().is_some_and(|round| {
                    round.schedule_id == schedule.id
                        && round.period_id == period.id
                        && round.date == local.date
                });
                if same {
                    false
                } else {
                    self.start_scheduled_round(&schedule, &period, &local.date, 1);
                    true
                }
            }
            None => {
                self.set_idle();
                false
            }
        }
    }

    fn start_scheduled_round(
        &mut self,
        schedule: &Schedule,
        period: &Period,
        date: &str,
        number: u32,
    ) {
        self.info.mode = Mode::Work;
        let definition = period.round_definition(number);
        self.info.work_seconds = definition.work_seconds;
        self.info.break_seconds = definition.break_seconds;
        self.info.remaining = self.info.work_seconds;
        let tip = definition.content.unwrap_or_else(|| {
            choose_tip(
                &self.settings,
                self.previous_tip.as_deref(),
                &mut self.tip_rng,
            )
        });
        self.previous_tip = Some(tip.clone());
        self.info.tip = Some(tip);
        self.info.round = Some(RoundInfo {
            schedule_id: schedule.id.clone(),
            period_id: period.id.clone(),
            date: date.into(),
            number,
        });
    }

    fn set_idle(&mut self) {
        self.info.mode = Mode::Idle;
        self.info.remaining = 0;
        self.info.work_seconds = 0;
        self.info.break_seconds = 0;
        self.info.round = None;
        self.info.tip = None;
    }

    fn start_simple_work(&mut self) {
        self.info.mode = Mode::Work;
        self.info.work_seconds = u64::from(self.settings.work_minutes) * 60;
        self.info.break_seconds = u64::from(self.settings.break_minutes) * 60;
        self.info.remaining = self.info.work_seconds;
        self.info.round = None;
        self.info.tip = None;
    }

    fn finish_round(&mut self, now: &TimerTime) {
        let finished = self
            .info
            .round
            .clone()
            .filter(|_| !self.restart_after_break);
        self.restart_after_break = false;
        if self.settings.use_schedule {
            if let Some((schedule, period)) = self.current_period(now.local.as_ref()) {
                let date = &now.local.as_ref().unwrap().date;
                let next = finished
                    .filter(|round| {
                        round.schedule_id == schedule.id
                            && round.period_id == period.id
                            && round.date == *date
                    })
                    .map_or(1, |round| round.number.saturating_add(1));
                self.start_scheduled_round(&schedule, &period, date, next);
            } else {
                self.set_idle();
            }
        } else {
            self.start_simple_work();
        }
        self.break_ended_at = Some(now.monotonic);
    }
}
