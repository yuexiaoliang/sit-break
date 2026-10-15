//! 保存的日程定义；日期和时间均表示电脑本地日历。
use chrono::Datelike;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub repeat: Repeat,
    #[serde(default)]
    pub periods: Vec<Period>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Repeat {
    Once { date: String },
    Weekly { weekdays: Vec<u32> },
}

impl Repeat {
    pub fn applies_on(&self, local_date: &str) -> bool {
        match self {
            Self::Once { date } => date == local_date,
            Self::Weekly { weekdays } => chrono::NaiveDate::parse_from_str(local_date, "%Y-%m-%d")
                .is_ok_and(|date| weekdays.contains(&date.weekday().number_from_monday())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Period {
    pub id: String,
    pub start: String,
    pub end: String,
    pub work_minutes: u32,
    pub break_minutes: u32,
    #[serde(default)]
    pub overrides: Vec<RoundOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundOverride {
    pub round: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub break_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

pub(crate) struct RoundDefinition {
    pub work_seconds: u64,
    pub break_seconds: u64,
    pub content: Option<String>,
}

impl Period {
    pub(crate) fn round_definition(&self, number: u32) -> RoundDefinition {
        let adjustment = self.overrides.iter().find(|item| item.round == number);
        RoundDefinition {
            work_seconds: u64::from(
                adjustment
                    .and_then(|item| item.work_minutes)
                    .unwrap_or(self.work_minutes),
            ) * 60,
            break_seconds: u64::from(
                adjustment
                    .and_then(|item| item.break_minutes)
                    .unwrap_or(self.break_minutes),
            ) * 60,
            content: adjustment
                .and_then(|item| item.content.as_deref())
                .filter(|content| !content.trim().is_empty())
                .map(str::to_owned),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PeriodConflict {
    pub schedule_id: String,
    pub period_id: String,
    pub start: String,
    pub end: String,
    pub occurrence: Repeat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationError {
    pub schedule_id: Option<String>,
    pub period_id: Option<String>,
    pub field: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<PeriodConflict>,
}

pub fn minute_of_day(value: &str, allow_day_end: bool) -> Option<u32> {
    if allow_day_end && value == "24:00" {
        return Some(1440);
    }
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || !bytes[..2].iter().chain(&bytes[3..]).all(u8::is_ascii_digit)
    {
        return None;
    }
    let hour: u32 = value[..2].parse().ok()?;
    let minute: u32 = value[3..].parse().ok()?;
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

pub fn valid_date(value: &str) -> bool {
    value.len() == 10
        && chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .is_ok_and(|date| date.format("%Y-%m-%d").to_string() == value)
}

fn shared_occurrence(left: &Repeat, right: &Repeat) -> Option<Repeat> {
    match (left, right) {
        (Repeat::Once { date: left }, Repeat::Once { date: right })
            if left == right && valid_date(left) =>
        {
            Some(Repeat::Once { date: left.clone() })
        }
        (Repeat::Weekly { weekdays: left }, Repeat::Weekly { weekdays: right }) => {
            let weekdays = (1..=7)
                .filter(|day| left.contains(day) && right.contains(day))
                .collect::<Vec<_>>();
            (!weekdays.is_empty()).then_some(Repeat::Weekly { weekdays })
        }
        (once @ Repeat::Once { date }, weekly @ Repeat::Weekly { .. })
        | (weekly @ Repeat::Weekly { .. }, once @ Repeat::Once { date }) => {
            (valid_date(date) && weekly.applies_on(date)).then_some(once.clone())
        }
        _ => None,
    }
}

fn period_range(period: &Period) -> Option<(u32, u32)> {
    minute_of_day(&period.start, false)
        .zip(minute_of_day(&period.end, true))
        .filter(|(start, end)| start < end)
}

impl crate::Settings {
    /// 所有设置写入及运行进度变更前共用的校验。
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut report =
            |schedule_id: Option<&str>, period_id: Option<&str>, field: &str, code: &str| {
                errors.push(ValidationError {
                    schedule_id: schedule_id.map(str::to_owned),
                    period_id: period_id.map(str::to_owned),
                    field: field.into(),
                    code: code.into(),
                    conflict: None,
                });
            };
        if !(1..=240).contains(&self.work_minutes) {
            report(None, None, "work_minutes", "work_range");
        }
        if !(1..=60).contains(&self.break_minutes) {
            report(None, None, "break_minutes", "break_range");
        }
        let mut reminder_ids = std::collections::HashSet::new();
        for (index, entry) in self.tips.iter().enumerate() {
            if entry.id.trim().is_empty() || !reminder_ids.insert(&entry.id) {
                report(
                    None,
                    None,
                    &format!("tips.{index}.text"),
                    "reminder_identity",
                );
            }
            if entry.text.trim().is_empty() {
                report(None, None, &format!("tips.{index}.text"), "reminder_empty");
            }
        }
        let mut schedule_ids = std::collections::HashSet::new();
        for schedule in &self.schedules {
            let sid = Some(schedule.id.as_str());
            if schedule.id.is_empty() || !schedule_ids.insert(&schedule.id) {
                report(sid, None, "id", "identity");
            }
            match &schedule.repeat {
                Repeat::Once { date } if !valid_date(date) => {
                    report(sid, None, "date", "invalid_date")
                }
                Repeat::Weekly { weekdays }
                    if weekdays.is_empty()
                        || weekdays.iter().any(|day| !(1..=7).contains(day))
                        || weekdays
                            .iter()
                            .collect::<std::collections::HashSet<_>>()
                            .len()
                            != weekdays.len() =>
                {
                    report(sid, None, "weekdays", "invalid_weekdays")
                }
                _ => {}
            }
            let mut period_ids = std::collections::HashSet::new();
            for period in &schedule.periods {
                let pid = Some(period.id.as_str());
                if period.id.is_empty() || !period_ids.insert(&period.id) {
                    report(sid, pid, "id", "identity");
                }
                let start = minute_of_day(&period.start, false);
                let end = minute_of_day(&period.end, true);
                if start.is_none() {
                    report(sid, pid, "start", "invalid_time");
                }
                if end.is_none() {
                    report(sid, pid, "end", "invalid_time");
                }
                if start.zip(end).is_some_and(|(start, end)| start >= end) {
                    report(sid, pid, "end", "end_before_start");
                }
                if !(1..=240).contains(&period.work_minutes) {
                    report(sid, pid, "work_minutes", "work_range");
                }
                if !(1..=60).contains(&period.break_minutes) {
                    report(sid, pid, "break_minutes", "break_range");
                }
                let mut adjusted_rounds = std::collections::HashSet::new();
                for (index, adjustment) in period.overrides.iter().enumerate() {
                    if adjustment.round == 0 {
                        report(
                            sid,
                            pid,
                            &format!("overrides.{index}.round"),
                            "round_number",
                        );
                    } else if !adjusted_rounds.insert(adjustment.round) {
                        report(
                            sid,
                            pid,
                            &format!("overrides.{index}.round"),
                            "duplicate_round",
                        );
                    }
                    if adjustment
                        .work_minutes
                        .is_some_and(|minutes| !(1..=240).contains(&minutes))
                    {
                        report(
                            sid,
                            pid,
                            &format!("overrides.{index}.work_minutes"),
                            "work_range",
                        );
                    }
                    if adjustment
                        .break_minutes
                        .is_some_and(|minutes| !(1..=60).contains(&minutes))
                    {
                        report(
                            sid,
                            pid,
                            &format!("overrides.{index}.break_minutes"),
                            "break_range",
                        );
                    }
                }
            }
        }
        // 每份日程内部始终要求合法；跨日程只比较同时启用的共同执行日期。
        for schedule in &self.schedules {
            for (index, period) in schedule.periods.iter().enumerate() {
                let Some((start, end)) = minute_of_day(&period.start, false)
                    .zip(minute_of_day(&period.end, true))
                    .filter(|(start, end)| start < end)
                else {
                    continue;
                };
                for other in &schedule.periods[..index] {
                    let Some((other_start, other_end)) = minute_of_day(&other.start, false)
                        .zip(minute_of_day(&other.end, true))
                        .filter(|(start, end)| start < end)
                    else {
                        continue;
                    };
                    if start < other_end && other_start < end {
                        errors.push(ValidationError {
                            schedule_id: Some(schedule.id.clone()),
                            period_id: Some(period.id.clone()),
                            field: "start".into(),
                            code: "overlap".into(),
                            conflict: Some(PeriodConflict {
                                schedule_id: schedule.id.clone(),
                                period_id: other.id.clone(),
                                start: other.start.clone(),
                                end: other.end.clone(),
                                occurrence: schedule.repeat.clone(),
                            }),
                        });
                    }
                }
            }
        }
        for (index, schedule) in self
            .schedules
            .iter()
            .enumerate()
            .filter(|(_, item)| item.enabled)
        {
            for other in self.schedules[..index].iter().filter(|item| item.enabled) {
                let Some(occurrence) = shared_occurrence(&schedule.repeat, &other.repeat) else {
                    continue;
                };
                for period in &schedule.periods {
                    let Some((start, end)) = period_range(period) else {
                        continue;
                    };
                    for other_period in &other.periods {
                        let Some((other_start, other_end)) = period_range(other_period) else {
                            continue;
                        };
                        if start < other_end && other_start < end {
                            errors.push(ValidationError {
                                schedule_id: Some(schedule.id.clone()),
                                period_id: Some(period.id.clone()),
                                field: "start".into(),
                                code: "overlap".into(),
                                conflict: Some(PeriodConflict {
                                    schedule_id: other.id.clone(),
                                    period_id: other_period.id.clone(),
                                    start: other_period.start.clone(),
                                    end: other_period.end.clone(),
                                    occurrence: occurrence.clone(),
                                }),
                            });
                        }
                    }
                }
            }
        }
        errors
    }
}
