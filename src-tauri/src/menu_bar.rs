use crate::timer::{Mode, TimerApp};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{set_pressed, update};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
    Ring,
    Paused,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fill {
    Work,
    Reminder,
    Break,
    Paused,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Status {
    pub label: String,
    pub icon: Icon,
    pub progress: Option<(Fill, f64)>,
}

impl Status {
    pub(crate) fn from_timer(timer: &TimerApp, zh: bool) -> Self {
        let info = timer.info();
        let local =
            |chinese: &'static str, english: &'static str| if zh { chinese } else { english };
        let label = match info.mode {
            Mode::Work => match &info.round {
                Some(round) if zh => format!("第 {} 轮", round.number),
                Some(round) => format!("R{}", round.number),
                None => local("工作", "Work").into(),
            },
            Mode::Remind if timer.scheduled_break_finished() => local("已休息", "Done").into(),
            Mode::Remind => local("起身", "Move").into(),
            Mode::Break => local("休息", "Break").into(),
            Mode::Idle => local("待机", "Idle").into(),
        };
        let icon = match (info.paused, timer.settings().use_schedule) {
            (true, true) => Icon::Muted,
            (true, false) => Icon::Paused,
            (false, _) => Icon::Ring,
        };
        let total = timer.countdown_total_seconds();
        let progress = if info.remaining == 0 || total == 0 {
            None
        } else {
            let fill = match (info.mode, icon) {
                (_, Icon::Paused) => Fill::Paused,
                (Mode::Work, _) => Fill::Work,
                (Mode::Remind, _) => Fill::Reminder,
                (Mode::Break, _) => Fill::Break,
                (Mode::Idle, _) => unreachable!(),
            };
            Some((fill, info.remaining.min(total) as f64 / total as f64))
        };
        Self {
            label,
            icon,
            progress,
        }
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn description(&self, zh: bool) -> String {
        let suffix = match (self.icon, zh) {
            (Icon::Ring, _) => "",
            (Icon::Paused, true) => " · 已暂停",
            (Icon::Paused, false) => " · Paused",
            (Icon::Muted, true) => " · 提醒已暂停",
            (Icon::Muted, false) => " · Reminders paused",
        };
        format!("Sit Break · {}{suffix}", self.label)
    }
}
