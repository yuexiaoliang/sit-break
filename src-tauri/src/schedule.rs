//! 保存的日程定义；日期和时间均表示电脑本地日历。
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Period {
    pub id: String,
    pub start: String,
    pub end: String,
    pub work_minutes: u32,
    pub break_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationError {
    pub schedule_id: Option<String>,
    pub period_id: Option<String>,
    pub field: String,
    pub code: String,
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
                });
            };
        if !(1..=240).contains(&self.work_minutes) {
            report(None, None, "work_minutes", "work_range");
        }
        if !(1..=60).contains(&self.break_minutes) {
            report(None, None, "break_minutes", "break_range");
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
            }
        }
        errors
    }
}
