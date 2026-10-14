//! 计时应用入口。调用方提供时间和用户操作，并执行返回的系统效果。
use std::time::Duration;

use serde::Serialize;

use crate::{choose_tip, idle_reset_limit_secs, Settings, POSTPONE_SECS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Idle,
    Work,
    Remind,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TimerInfo {
    pub mode: Mode,
    pub remaining: u64,
    pub paused: bool,
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
}

impl TimerApp {
    pub fn load_from(path: &std::path::Path, tip_rng: fastrand::Rng) -> Self {
        Self::new(Settings::load_from(path), tip_rng)
    }

    pub fn new(settings: Settings, tip_rng: fastrand::Rng) -> Self {
        Self {
            info: TimerInfo {
                mode: if settings.use_schedule {
                    Mode::Idle
                } else {
                    Mode::Work
                },
                remaining: if settings.use_schedule {
                    0
                } else {
                    u64::from(settings.work_minutes) * 60
                },
                paused: false,
            },
            settings,
            break_ended_at: None,
            tip_rng,
            previous_tip: None,
        }
    }

    pub fn info(&self) -> TimerInfo {
        self.info
    }
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn dispatch(&mut self, action: Action, now: Duration) -> TimerOutput {
        let mut output = TimerOutput::default();
        match action {
            Action::Tick {
                elapsed_secs,
                idle_secs,
            } => {
                if self.info.paused {
                    return output;
                }
                let idle_limit = idle_reset_limit_secs(&self.settings);
                match self.info.mode {
                    Mode::Idle => {}
                    Mode::Work => {
                        let in_grace = self
                            .break_ended_at
                            .map(|ended| {
                                now.saturating_sub(ended) < Duration::from_secs(idle_limit)
                            })
                            .unwrap_or(false);
                        if !in_grace && idle_secs >= idle_limit {
                            self.info.remaining = u64::from(self.settings.work_minutes) * 60;
                        } else {
                            self.info.remaining = self.info.remaining.saturating_sub(elapsed_secs);
                        }
                        if self.info.remaining == 0 {
                            self.info.mode = Mode::Remind;
                            let tip = choose_tip(
                                &self.settings,
                                self.previous_tip.as_deref(),
                                &mut self.tip_rng,
                            );
                            self.previous_tip = Some(tip.clone());
                            output.events.push(TimerEvent::Reminder(ReminderPayload {
                                break_seconds: u64::from(self.settings.break_minutes) * 60,
                                postpone_seconds: POSTPONE_SECS
                                    .min(u64::from(self.settings.work_minutes) * 60)
                                    .max(60),
                                sound: self.settings.sound,
                                tip,
                                sat_minutes: self.settings.work_minutes,
                            }));
                            output.update_windows = true;
                        }
                    }
                    Mode::Break => {
                        self.info.remaining = self.info.remaining.saturating_sub(elapsed_secs);
                        if self.info.remaining == 0 {
                            self.back_to_work(now);
                            output.update_windows = true;
                        }
                    }
                    Mode::Remind => {
                        if idle_secs >= idle_limit {
                            self.back_to_work(now);
                            output.update_windows = true;
                        }
                    }
                }
            }
            Action::StartBreak => {
                if self.info.mode != Mode::Remind {
                    return output;
                }
                self.info.mode = Mode::Break;
                self.info.remaining = u64::from(self.settings.break_minutes) * 60;
                output.events.push(TimerEvent::BreakStarted {
                    seconds: self.info.remaining,
                });
            }
            Action::TogglePause => self.info.paused = !self.info.paused,
            Action::Reset => {
                self.info.mode = if self.settings.use_schedule {
                    Mode::Idle
                } else {
                    Mode::Work
                };
                self.info.remaining = if self.settings.use_schedule {
                    0
                } else {
                    u64::from(self.settings.work_minutes) * 60
                };
                output.update_windows = true;
            }
            Action::Postpone => {
                if self.info.mode != Mode::Remind {
                    return output;
                }
                self.info.mode = Mode::Work;
                self.info.remaining = POSTPONE_SECS
                    .min(u64::from(self.settings.work_minutes) * 60)
                    .max(60);
                output.update_windows = true;
            }
            Action::EndBreak => {
                if self.info.mode != Mode::Break {
                    return output;
                }
                self.back_to_work(now);
                output.update_windows = true;
            }
            Action::ApplyInterval(minutes) => {
                self.settings.work_minutes = minutes;
                if self.info.mode == Mode::Work {
                    self.info.remaining = u64::from(minutes) * 60;
                }
                output.persist_settings = Some(self.settings.clone());
                output.settings_changed = true;
            }
            Action::ApplyBreak(minutes) => {
                self.settings.break_minutes = minutes;
                output.persist_settings = Some(self.settings.clone());
                output.settings_changed = true;
            }
            Action::SaveSettings(settings) => {
                output.update_windows = self.settings.show_widget != settings.show_widget;
                let work_changed = self.settings.work_minutes != settings.work_minutes;
                let mode_changed = self.settings.use_schedule != settings.use_schedule;
                self.settings = settings;
                if mode_changed && self.info.mode != Mode::Break {
                    self.info.mode = if self.settings.use_schedule {
                        Mode::Idle
                    } else {
                        Mode::Work
                    };
                    self.info.remaining = if self.settings.use_schedule {
                        0
                    } else {
                        u64::from(self.settings.work_minutes) * 60
                    };
                    self.break_ended_at = None;
                    output.update_windows = true;
                } else if self.info.mode == Mode::Work {
                    let interval = u64::from(self.settings.work_minutes) * 60;
                    if work_changed || self.info.remaining > interval {
                        self.info.remaining = interval;
                    }
                }
                output.persist_settings = Some(self.settings.clone());
                output.settings_changed = true;
            }
        }
        output.state = Some(self.info);
        output.update_tray = true;
        output
    }

    fn back_to_work(&mut self, now: Duration) {
        self.info.mode = if self.settings.use_schedule {
            Mode::Idle
        } else {
            Mode::Work
        };
        self.info.remaining = if self.settings.use_schedule {
            0
        } else {
            u64::from(self.settings.work_minutes) * 60
        };
        self.break_ended_at = Some(now);
    }
}
