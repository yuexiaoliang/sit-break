//! 只读日程预览。草稿和生效安排使用相同的计划时刻。
use serde::{Deserialize, Serialize};

use crate::schedule::{minute_of_day, Period, PlannedRound, Repeat};
use crate::timer::{LocalTime, TimerInfo};
use crate::Settings;

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewMode {
    Plan,
    Progress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchedulePreview {
    pub local_date: String,
    pub current: Option<CurrentPreview>,
    pub schedules: Vec<PreviewSchedule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CurrentPreview {
    pub state: TimerInfo,
    pub break_start: i64,
    pub break_end: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewSchedule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub repeat: Repeat,
    pub date: String,
    pub applies_today: bool,
    pub uses_progress: bool,
    pub periods: Vec<PreviewPeriod>,
    pub gaps: Vec<PreviewGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewPeriod {
    pub id: String,
    pub start: String,
    pub end: String,
    pub rounds: Vec<PreviewRound>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewRound {
    pub number: u32,
    pub work_start: i64,
    pub break_start: i64,
    pub break_end: i64,
    pub work_seconds: u64,
    pub break_seconds: u64,
    pub content: Option<String>,
    pub guidance: Option<crate::Guidance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewGap {
    pub start: i64,
    pub end: i64,
}

impl PreviewRound {
    fn from_plan(period: &Period, round: PlannedRound) -> Self {
        let (content, guidance) = period.round_content(round.number);
        Self {
            number: round.number,
            work_start: round.work_start,
            break_start: round.break_start,
            break_end: round.break_end,
            work_seconds: round.work_seconds(),
            break_seconds: round.break_seconds(),
            content,
            guidance,
        }
    }
}

pub(crate) fn build(
    draft: &Settings,
    saved: &Settings,
    info: &TimerInfo,
    planned_round: Option<PlannedRound>,
    local: Option<&LocalTime>,
    mode: PreviewMode,
) -> SchedulePreview {
    let local_date = local.map(|time| time.date.clone()).unwrap_or_default();
    let seconds = local.map(|time| i64::from(time.seconds_of_day));
    let current = planned_round
        .filter(|_| mode == PreviewMode::Progress)
        .map(|round| CurrentPreview {
            state: info.clone(),
            break_start: round.break_start,
            break_end: round.break_end,
        });
    let settings = match mode {
        PreviewMode::Plan => draft,
        PreviewMode::Progress => saved,
    };
    let schedules = settings
        .schedules
        .iter()
        .map(|schedule| {
            let date = match &schedule.repeat {
                Repeat::Once { date } => date.clone(),
                Repeat::Weekly { .. } => local_date.clone(),
            };
            let uses_progress = mode == PreviewMode::Progress
                && settings.use_schedule
                && schedule.enabled
                && date == local_date
                && schedule.repeat.applies_on(&local_date);
            let mut ordered = schedule.periods.iter().collect::<Vec<_>>();
            ordered.sort_by_key(|period| minute_of_day(&period.start, false).unwrap());
            let gaps = ordered
                .windows(2)
                .filter_map(|pair| {
                    let start = i64::from(minute_of_day(&pair[0].end, true).unwrap()) * 60;
                    let end = i64::from(minute_of_day(&pair[1].start, false).unwrap()) * 60;
                    (start < end).then_some(PreviewGap { start, end })
                })
                .collect();
            let periods = ordered
                .into_iter()
                .map(|period| {
                    let start = i64::from(minute_of_day(&period.start, false).unwrap()) * 60;
                    let end = i64::from(minute_of_day(&period.end, true).unwrap()) * 60;
                    let rounds = period
                        .complete_rounds(start, end)
                        .filter(|round| {
                            !uses_progress || seconds.is_none_or(|now| round.work_start > now)
                        })
                        .map(|round| PreviewRound::from_plan(period, round))
                        .collect();
                    PreviewPeriod {
                        id: period.id.clone(),
                        start: period.start.clone(),
                        end: period.end.clone(),
                        rounds,
                    }
                })
                .collect();
            PreviewSchedule {
                id: schedule.id.clone(),
                name: schedule.name.clone(),
                enabled: schedule.enabled,
                repeat: schedule.repeat.clone(),
                date,
                applies_today: schedule.repeat.applies_on(&local_date),
                uses_progress,
                periods,
                gaps,
            }
        })
        .collect();
    SchedulePreview {
        local_date,
        current,
        schedules,
    }
}
