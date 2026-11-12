//! 计时应用入口。调用方提供本地日历、经过时间和用户操作，并执行返回的系统效果。
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::schedule::{PlannedRound, ValidationError};
use crate::{choose_tip, Guidance, Settings, POSTPONE_SECS};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Idle,
    Work,
    Remind,
    Break,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundInfo {
    pub schedule_id: String,
    pub period_id: String,
    pub date: String,
    pub number: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerInfo {
    pub mode: Mode,
    pub remaining: u64,
    pub paused: bool,
    pub work_seconds: u64,
    pub break_seconds: u64,
    pub round: Option<RoundInfo>,
    pub tip: Option<String>,
    #[serde(default)]
    pub guidance: Option<Guidance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderPayload {
    pub break_seconds: u64,
    pub postpone_seconds: u64,
    pub sound: bool,
    pub tip: String,
    pub guidance: Option<Guidance>,
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
    Suspend { idle_secs: u64 },
    Resume { elapsed_secs: u64, idle_secs: u64 },
    StartBreak,
    TogglePause,
    Reset,
    Postpone,
    EndBreak,
    ApplyInterval(u32),
    ApplyBreak(u32),
    SaveSettings(Settings),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScheduledRest {
    #[default]
    Waiting,
    SnoozedUntil(i64),
    Started,
    Dismissed,
}

#[derive(Serialize, Deserialize)]
struct SavedProgress {
    version: u32,
    saved_local: Option<LocalTime>,
    use_schedule: bool,
    info: TimerInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_guidance: Option<Guidance>,
    #[serde(default)]
    planned_round: Option<PlannedRound>,
    #[serde(default)]
    scheduled_rest: ScheduledRest,
}

impl SavedProgress {
    fn is_valid(&self) -> bool {
        if !matches!(self.version, 1 | 2)
            || self
                .info
                .guidance
                .as_ref()
                .is_some_and(|guidance| !guidance.is_valid())
            || self
                .previous_guidance
                .as_ref()
                .is_some_and(|guidance| !guidance.is_valid())
            || self.saved_local.as_ref().is_some_and(|local| {
                !crate::schedule::valid_date(&local.date) || local.seconds_of_day >= 86_400
            })
        {
            return false;
        }
        if self.info.mode == Mode::Idle {
            return self.info.remaining == 0
                && self.info.work_seconds == 0
                && self.info.break_seconds == 0
                && self.info.round.is_none();
        }
        let valid_lengths = (60..=14_400).contains(&self.info.work_seconds)
            && self.info.work_seconds.is_multiple_of(60)
            && (60..=3600).contains(&self.info.break_seconds)
            && self.info.break_seconds.is_multiple_of(60);
        let valid_remaining = match self.info.mode {
            Mode::Work => (1..=self.info.work_seconds).contains(&self.info.remaining),
            Mode::Remind => {
                self.info.remaining == 0
                    || (self.version == 2
                        && self.use_schedule
                        && self.info.remaining <= self.info.break_seconds)
            }
            Mode::Break => (1..=self.info.break_seconds).contains(&self.info.remaining),
            Mode::Idle => unreachable!(),
        };
        let valid_round = self.info.round.as_ref().is_none_or(|round| {
            !round.schedule_id.is_empty()
                && !round.period_id.is_empty()
                && round.number > 0
                && crate::schedule::valid_date(&round.date)
                && self.saved_local.is_some()
                && (self
                    .info
                    .tip
                    .as_ref()
                    .is_some_and(|tip| !tip.trim().is_empty())
                    || self.info.guidance.as_ref().is_some_and(Guidance::is_valid))
        });
        valid_lengths && valid_remaining && valid_round
    }
}

pub struct TimerApp {
    info: TimerInfo,
    settings: Settings,
    break_ended_at: Option<Duration>,
    tip_rng: fastrand::Rng,
    previous_tip: Option<String>,
    previous_guidance: Option<Guidance>,
    planned_round: Option<PlannedRound>,
    scheduled_rest: ScheduledRest,
    suspended_idle: Option<u64>,
}

impl TimerApp {
    pub fn save_progress_to(
        &self,
        path: &std::path::Path,
        now: impl Into<TimerTime>,
    ) -> Result<(), String> {
        let now = now.into();
        let progress = SavedProgress {
            version: 2,
            saved_local: now.local,
            use_schedule: self.settings.use_schedule,
            info: self.info(),
            previous_guidance: None,
            planned_round: self.planned_round,
            scheduled_rest: self.scheduled_rest,
        };
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let text = serde_json::to_string_pretty(&progress).map_err(|error| error.to_string())?;
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, text).map_err(|error| error.to_string())?;
        std::fs::rename(&temporary, path).map_err(|error| error.to_string())
    }

    pub fn restore_from(
        settings: Settings,
        path: &std::path::Path,
        tip_rng: fastrand::Rng,
        now: impl Into<TimerTime>,
    ) -> Self {
        let now = now.into();
        let mut app = Self::new_at(settings, tip_rng, now.clone());
        let Some(saved) = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<SavedProgress>(&text).ok())
        else {
            return app;
        };
        if !saved.is_valid() {
            return app;
        }
        app.info.paused = saved.info.paused;
        // 旧版剩余时间按实际进度累计。升级后只沿用暂停状态，按钟点选轮。
        if saved.version == 2
            && saved.use_schedule
            && app.settings.use_schedule
            && app.info.round.is_some()
            && app.info.round == saved.info.round
            && app.planned_round == saved.planned_round
        {
            app.info.tip = saved.info.tip;
            app.info.guidance = saved.info.guidance;
            app.previous_tip = app.info.tip.clone();
            app.previous_guidance = app.info.guidance.clone();
            app.scheduled_rest = saved.scheduled_rest;
            app.refresh_schedule(now.local.as_ref());
        }
        app
    }

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
                guidance: None,
            },
            settings,
            break_ended_at: None,
            tip_rng,
            previous_tip: None,
            previous_guidance: None,
            planned_round: None,
            scheduled_rest: ScheduledRest::Waiting,
            suspended_idle: None,
        };
        let now = now.into();
        if app.settings.use_schedule {
            app.refresh_schedule(now.local.as_ref());
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
        self.preview_with_mode(draft, now, crate::preview::PreviewMode::Progress)
    }

    pub fn preview_with_mode(
        &self,
        draft: &Settings,
        now: impl Into<TimerTime>,
        mode: crate::preview::PreviewMode,
    ) -> Result<crate::preview::SchedulePreview, Vec<ValidationError>> {
        if mode == crate::preview::PreviewMode::Plan {
            let errors = draft.validate_for_preview();
            if !errors.is_empty() {
                return Err(errors);
            }
        }
        Ok(crate::preview::build(
            draft,
            &self.settings,
            &self.info,
            self.planned_round,
            now.into().local.as_ref(),
            mode,
        ))
    }

    pub(crate) fn reminder_visible(&self) -> bool {
        if !self.settings.use_schedule {
            return matches!(self.info.mode, Mode::Remind | Mode::Break);
        }
        !self.info.paused
            && self.suspended_idle.is_none()
            && match self.info.mode {
                Mode::Break => true,
                Mode::Remind => self.scheduled_rest == ScheduledRest::Waiting,
                Mode::Work | Mode::Idle => false,
            }
    }

    pub fn dispatch(&mut self, action: Action, now: impl Into<TimerTime>) -> TimerOutput {
        let now = now.into();
        let mut output = TimerOutput::default();
        let prior_round = self.info.round.clone();
        let prior_mode = self.info.mode;
        let was_visible = self.reminder_visible();

        match action {
            Action::SaveSettings(settings) => {
                output.validation_errors = settings.validate();
                if !output.validation_errors.is_empty() {
                    return output;
                }
                output.update_windows = self.settings.show_widget != settings.show_widget;
                self.save_settings(settings, &now);
                output.persist_settings = Some(self.settings.clone());
                output.settings_changed = true;
            }
            Action::ApplyInterval(minutes) if (1..=240).contains(&minutes) => {
                self.settings.work_minutes = minutes;
                if self.settings.use_schedule {
                    self.refresh_schedule(now.local.as_ref());
                } else if self.info.mode != Mode::Break {
                    self.info.work_seconds = u64::from(minutes) * 60;
                    if self.info.mode == Mode::Work {
                        self.info.remaining = self.info.work_seconds;
                    }
                }
                output.persist_settings = Some(self.settings.clone());
                output.settings_changed = true;
            }
            Action::ApplyBreak(minutes) if (1..=60).contains(&minutes) => {
                self.settings.break_minutes = minutes;
                if self.settings.use_schedule {
                    self.refresh_schedule(now.local.as_ref());
                } else if self.info.mode != Mode::Break {
                    self.info.break_seconds = u64::from(minutes) * 60;
                }
                output.persist_settings = Some(self.settings.clone());
                output.settings_changed = true;
            }
            action if self.settings.use_schedule => {
                self.dispatch_scheduled(action, &now, &mut output)
            }
            action => self.dispatch_simple(action, &now, &mut output),
        }

        let visible = self.reminder_visible();
        if prior_round != self.info.round || prior_mode != self.info.mode || was_visible != visible
        {
            output.update_windows = true;
            if self.settings.use_schedule && self.info.mode == Mode::Remind && visible {
                output.events.push(self.reminder_event());
            }
        }
        output.state = Some(self.info());
        output.update_tray = true;
        output
    }

    fn save_settings(&mut self, settings: Settings, now: &TimerTime) {
        let work_changed = self.settings.work_minutes != settings.work_minutes;
        let mode_changed = self.settings.use_schedule != settings.use_schedule;
        self.settings = settings;
        if mode_changed {
            self.break_ended_at = None;
        }
        if self.settings.use_schedule {
            self.refresh_schedule(now.local.as_ref());
        } else if mode_changed {
            self.start_simple_work();
        } else if self.info.mode != Mode::Break {
            self.info.work_seconds = u64::from(self.settings.work_minutes) * 60;
            self.info.break_seconds = u64::from(self.settings.break_minutes) * 60;
            if self.info.mode == Mode::Work
                && (work_changed || self.info.remaining > self.info.work_seconds)
            {
                self.info.remaining = self.info.work_seconds;
            }
        }
    }

    fn dispatch_simple(&mut self, action: Action, now: &TimerTime, output: &mut TimerOutput) {
        let resuming = matches!(action, Action::Resume { .. });
        match action {
            Action::Tick {
                elapsed_secs,
                idle_secs,
            }
            | Action::Resume {
                elapsed_secs,
                idle_secs,
            } => {
                let sleeping_idle = if resuming {
                    self.suspended_idle.take()
                } else {
                    None
                };
                if self.info.paused
                    || self.suspended_idle.is_some()
                    || (resuming && sleeping_idle.is_none())
                {
                    return;
                }
                let idle_secs = sleeping_idle
                    .map(|before| before.saturating_add(elapsed_secs).max(idle_secs))
                    .unwrap_or(idle_secs);
                let away = self.settings.idle_reset && idle_secs >= self.info.break_seconds;
                match self.info.mode {
                    Mode::Idle => {}
                    Mode::Work => {
                        if resuming && away {
                            self.break_ended_at = None;
                        }
                        let in_grace = self.break_ended_at.is_some_and(|ended| {
                            now.monotonic.saturating_sub(ended)
                                < Duration::from_secs(self.info.break_seconds)
                        });
                        if away && !in_grace {
                            self.info.remaining = self.info.work_seconds;
                        } else if !resuming {
                            self.info.remaining = self.info.remaining.saturating_sub(elapsed_secs);
                        }
                        if self.info.remaining == 0 {
                            self.info.mode = Mode::Remind;
                            self.select_reminder_entry();
                            output.events.push(self.reminder_event());
                        }
                    }
                    Mode::Break => {
                        self.info.remaining = self.info.remaining.saturating_sub(elapsed_secs);
                        if self.info.remaining == 0 {
                            self.finish_simple_break(now);
                        }
                    }
                    Mode::Remind => {
                        if away {
                            self.finish_simple_break(now);
                        }
                    }
                }
            }
            Action::Suspend { idle_secs } => {
                self.suspended_idle.get_or_insert(idle_secs);
            }
            Action::StartBreak if self.info.mode == Mode::Remind => {
                self.info.mode = Mode::Break;
                self.info.remaining = self.info.break_seconds;
                output.events.push(TimerEvent::BreakStarted {
                    seconds: self.info.remaining,
                });
            }
            Action::TogglePause => self.info.paused = !self.info.paused,
            Action::Reset => self.start_simple_work(),
            Action::Postpone if self.info.mode == Mode::Remind => {
                self.info.mode = Mode::Work;
                self.info.remaining = self.postpone_seconds();
            }
            Action::EndBreak if self.info.mode == Mode::Break => self.finish_simple_break(now),
            _ => {}
        }
    }

    fn dispatch_scheduled(&mut self, action: Action, now: &TimerTime, output: &mut TimerOutput) {
        let previous_round = self.info.round.clone();
        self.refresh_schedule(now.local.as_ref());
        // 旧浮窗上的点击不能改变刚切换到的新一轮。
        let can_act_on_rest = previous_round == self.info.round && self.reminder_visible();
        match action {
            Action::TogglePause => self.info.paused = !self.info.paused,
            Action::Suspend { idle_secs } => {
                self.suspended_idle.get_or_insert(idle_secs);
            }
            Action::Resume { .. } => self.suspended_idle = None,
            Action::StartBreak if can_act_on_rest && self.info.mode == Mode::Remind => {
                self.scheduled_rest = ScheduledRest::Started;
                self.info.mode = Mode::Break;
                output.events.push(TimerEvent::BreakStarted {
                    seconds: self.info.remaining,
                });
            }
            Action::Postpone if can_act_on_rest && self.info.mode == Mode::Remind => {
                let end = self.planned_round.unwrap().break_end;
                let seconds = i64::from(now.local.as_ref().unwrap().seconds_of_day);
                self.scheduled_rest =
                    ScheduledRest::SnoozedUntil((seconds + POSTPONE_SECS as i64).min(end));
            }
            Action::EndBreak if can_act_on_rest && self.info.mode == Mode::Break => {
                self.scheduled_rest = ScheduledRest::Dismissed;
                self.info.mode = Mode::Remind;
            }
            _ => {}
        }
    }

    fn postpone_seconds(&self) -> u64 {
        POSTPONE_SECS.min(self.info.work_seconds).max(60)
    }

    fn reminder_event(&self) -> TimerEvent {
        TimerEvent::Reminder(ReminderPayload {
            break_seconds: self.info.break_seconds,
            postpone_seconds: if self.settings.use_schedule {
                POSTPONE_SECS.min(self.info.remaining)
            } else {
                self.postpone_seconds()
            },
            sound: self.settings.sound,
            tip: self.info.tip.clone().unwrap_or_default(),
            guidance: self.info.guidance.clone(),
            sat_minutes: (self.info.work_seconds / 60) as u32,
        })
    }

    fn refresh_schedule(&mut self, local: Option<&LocalTime>) {
        let Some(local) = local else {
            self.set_idle();
            return;
        };
        let seconds = i64::from(local.seconds_of_day);
        let active = self
            .settings
            .schedules
            .iter()
            .filter(|schedule| schedule.enabled && schedule.repeat.applies_on(&local.date))
            .find_map(|schedule| {
                schedule.periods.iter().find_map(|period| {
                    period
                        .round_at(seconds)
                        .map(|round| (schedule, period, round))
                })
            });
        let Some((schedule, period, planned)) = active else {
            self.set_idle();
            return;
        };
        let same_round = self.info.round.as_ref().is_some_and(|round| {
            round.date == local.date
                && round.schedule_id == schedule.id
                && round.period_id == period.id
                && round.number == planned.number
        });
        if !same_round {
            self.info.round = Some(RoundInfo {
                schedule_id: schedule.id.clone(),
                period_id: period.id.clone(),
                date: local.date.clone(),
                number: planned.number,
            });
            let (content, guidance) = period.round_content(planned.number);
            if content.is_some() || guidance.is_some() {
                self.info.tip = Some(content.unwrap_or_default());
                self.info.guidance = guidance;
                self.previous_tip = self.info.tip.clone();
                self.previous_guidance = self.info.guidance.clone();
            } else {
                self.select_reminder_entry();
            }
        }
        if !same_round || self.planned_round != Some(planned) {
            self.scheduled_rest = ScheduledRest::Waiting;
        }
        self.planned_round = Some(planned);
        self.info.work_seconds = planned.work_seconds();
        self.info.break_seconds = planned.break_seconds();
        if seconds < planned.break_start {
            self.scheduled_rest = ScheduledRest::Waiting;
            self.info.mode = Mode::Work;
            self.info.remaining = (planned.break_start - seconds) as u64;
        } else {
            if matches!(self.scheduled_rest, ScheduledRest::SnoozedUntil(until) if seconds >= until)
            {
                self.scheduled_rest = ScheduledRest::Waiting;
            }
            self.info.mode = if self.scheduled_rest == ScheduledRest::Started {
                Mode::Break
            } else {
                Mode::Remind
            };
            self.info.remaining = (planned.break_end - seconds) as u64;
        }
    }

    fn select_reminder_entry(&mut self) {
        let entries: Vec<_> = self
            .settings
            .tips
            .iter()
            .filter(|entry| !entry.text.trim().is_empty() || entry.guidance.is_some())
            .collect();
        if entries.is_empty() {
            self.info.tip = Some(choose_tip(
                &self.settings,
                self.previous_tip.as_deref(),
                &mut self.tip_rng,
            ));
            self.info.guidance = None;
        } else {
            let alternatives: Vec<_> = entries
                .iter()
                .copied()
                .filter(|entry| {
                    Some(entry.text.as_str()) != self.previous_tip.as_deref()
                        || entry.guidance != self.previous_guidance
                })
                .collect();
            let candidates = if alternatives.is_empty() {
                &entries
            } else {
                &alternatives
            };
            let entry = candidates[self.tip_rng.usize(..candidates.len())];
            self.info.tip = Some(entry.text.clone());
            self.info.guidance = entry.guidance.clone();
        }
        self.previous_tip = self.info.tip.clone();
        self.previous_guidance = self.info.guidance.clone();
    }

    fn set_idle(&mut self) {
        self.planned_round = None;
        self.scheduled_rest = ScheduledRest::Waiting;
        self.info.mode = Mode::Idle;
        self.info.remaining = 0;
        self.info.work_seconds = 0;
        self.info.break_seconds = 0;
        self.info.round = None;
        self.info.tip = None;
        self.info.guidance = None;
    }

    fn start_simple_work(&mut self) {
        self.planned_round = None;
        self.scheduled_rest = ScheduledRest::Waiting;
        self.info.mode = Mode::Work;
        self.info.work_seconds = u64::from(self.settings.work_minutes) * 60;
        self.info.break_seconds = u64::from(self.settings.break_minutes) * 60;
        self.info.remaining = self.info.work_seconds;
        self.info.round = None;
        self.info.tip = None;
        self.info.guidance = None;
    }

    fn finish_simple_break(&mut self, now: &TimerTime) {
        self.start_simple_work();
        self.break_ended_at = Some(now.monotonic);
    }
}
