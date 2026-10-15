//! 只读预计安排。当前轮快照与未来草稿共用后端时长继承规则。
use serde::Serialize;

use crate::schedule::{minute_of_day, Period, Repeat};
use crate::timer::{LocalTime, Mode, TimerInfo};
use crate::Settings;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchedulePreview {
    pub local_date: String,
    pub current: Option<CurrentPreview>,
    pub schedules: Vec<PreviewSchedule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CurrentPreview {
    pub state: TimerInfo,
    pub break_start: Option<i64>,
    pub break_end: Option<i64>,
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
    pub break_start: Option<i64>,
    pub break_end: Option<i64>,
    pub work_seconds: u64,
    pub break_seconds: u64,
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewGap {
    pub start: i64,
    pub end: i64,
}

fn rounds(period: &Period, mut cursor: i64, mut number: u32) -> Vec<PreviewRound> {
    let end = i64::from(minute_of_day(&period.end, true).unwrap()) * 60;
    let mut result = Vec::new();
    while cursor < end {
        let definition = period.round_definition(number);
        let break_start = cursor + definition.work_seconds as i64;
        let starts_in_period = break_start < end;
        let break_end = break_start + definition.break_seconds as i64;
        result.push(PreviewRound {
            number,
            work_start: cursor,
            break_start: starts_in_period.then_some(break_start),
            break_end: starts_in_period.then_some(break_end),
            work_seconds: definition.work_seconds,
            break_seconds: definition.break_seconds,
            content: definition.content,
        });
        if !starts_in_period || number == u32::MAX {
            break;
        }
        cursor = break_end;
        number += 1;
    }
    result
}

pub(crate) fn build(
    draft: &Settings,
    saved: &Settings,
    info: &TimerInfo,
    local: Option<&LocalTime>,
    restart_after_break: bool,
) -> SchedulePreview {
    let local_date = local.map(|time| time.date.clone()).unwrap_or_default();
    let actual_end = info
        .round
        .as_ref()
        .and_then(|round| {
            saved
                .schedules
                .iter()
                .find(|schedule| schedule.id == round.schedule_id)
                .and_then(|schedule| {
                    schedule
                        .periods
                        .iter()
                        .find(|period| period.id == round.period_id)
                })
                .and_then(|period| minute_of_day(&period.end, true))
        })
        .map(|minute| i64::from(minute) * 60);
    let current = local
        .filter(|_| info.round.is_some() || info.mode == Mode::Break)
        .filter(|_| info.mode != Mode::Idle)
        .map(|local| {
            let now = i64::from(local.seconds_of_day);
            let break_start = match info.mode {
                Mode::Work => Some(now + info.remaining as i64),
                Mode::Remind => Some(now),
                // 快照不记录实际开始时刻或暂停历程，只预计剩余休息的结束。
                Mode::Break => None,
                Mode::Idle => None,
            }
            .filter(|start| info.mode == Mode::Break || actual_end.is_none_or(|end| *start < end));
            let break_end = if info.mode == Mode::Break {
                Some(now + info.remaining as i64)
            } else {
                break_start.map(|start| start + info.break_seconds as i64)
            };
            CurrentPreview {
                state: info.clone(),
                break_start,
                break_end,
            }
        });
    let schedules = draft
        .schedules
        .iter()
        .map(|schedule| {
            let date = match &schedule.repeat {
                Repeat::Once { date } => date.clone(),
                Repeat::Weekly { .. } => local_date.clone(),
            };
            let uses_progress = current.is_some()
                && draft.use_schedule
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
                    let mut cursor = i64::from(minute_of_day(&period.start, false).unwrap()) * 60;
                    let mut number = 1;
                    if uses_progress {
                        cursor = cursor.max(
                            current
                                .as_ref()
                                .and_then(|active| active.break_end)
                                .or(actual_end)
                                .unwrap_or_else(|| i64::from(local.unwrap().seconds_of_day)),
                        );
                    }
                    if let Some(active) =
                        current.as_ref().filter(|_| uses_progress).filter(|active| {
                            active.state.round.as_ref().is_some_and(|round| {
                                round.schedule_id == schedule.id
                                    && round.period_id == period.id
                                    && round.date == date
                            })
                        })
                    {
                        number = if restart_after_break {
                            1
                        } else {
                            active
                                .state
                                .round
                                .as_ref()
                                .unwrap()
                                .number
                                .saturating_add(1)
                        };
                    }
                    PreviewPeriod {
                        id: period.id.clone(),
                        start: period.start.clone(),
                        end: period.end.clone(),
                        rounds: rounds(period, cursor, number),
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
