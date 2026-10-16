export type Appearance = "system" | "light" | "dark";
export type NumericDraft = number | string;
export interface Guidance {
  kind: "video";
  source: { type: "local"; id: string };
}
export interface RoundDraft {
  round: NumericDraft;
  work_minutes?: NumericDraft;
  break_minutes?: NumericDraft;
  content?: string;
  guidance?: Guidance | null;
}
export interface PeriodDraft {
  id: string;
  start: string;
  end: string;
  work_minutes: NumericDraft;
  break_minutes: NumericDraft;
  overrides?: RoundDraft[];
}
export interface RepeatDraft {
  kind: "once" | "weekly";
  date?: string;
  weekdays?: number[];
}
export interface ScheduleDraft {
  id: string;
  name: string;
  enabled: boolean;
  repeat: RepeatDraft;
  periods: PeriodDraft[];
}
export interface ReminderEntry {
  id: string;
  text: string;
  guidance?: Guidance | null;
}
export interface SettingsDraft {
  work_minutes: NumericDraft;
  break_minutes: NumericDraft;
  widget_size: NumericDraft;
  use_schedule: boolean;
  schedules: ScheduleDraft[];
  tips: ReminderEntry[];
  sound: boolean;
  show_widget: boolean;
  autostart: boolean;
  idle_reset: boolean;
  language: string;
  appearance: Appearance;
}
export interface Settings extends Omit<
  SettingsDraft,
  "work_minutes" | "break_minutes" | "widget_size"
> {
  work_minutes: number;
  break_minutes: number;
  widget_size: number;
}
export interface ValidationError {
  schedule_id: string | null;
  period_id: string | null;
  field: string;
  code: string;
  conflict?: {
    schedule_id: string;
    period_id: string;
    start: string;
    end: string;
    occurrence: RepeatDraft;
  };
}
export function appearance(value: unknown): Appearance {
  return value === "light" || value === "dark" ? value : "system";
}
export function normalizeSettings(value: Settings): Settings {
  return { ...value, appearance: appearance(value.appearance) };
}
export function localDate() {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}
export function clockMinutes(value: string, end = true): number | undefined {
  if (end && value === "24:00") return 1440;
  if (!/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(value)) return;
  const [hour, minute] = value.split(":").map(Number);
  return hour * 60 + minute;
}
export function clockText(minutes: number) {
  return `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;
}
export function quietGaps(periods: PeriodDraft[]) {
  const ranges = periods
    .map((p) => ({
      start: clockMinutes(p.start, false),
      end: clockMinutes(p.end),
    }))
    .filter(
      (r): r is { start: number; end: number } =>
        r.start !== undefined && r.end !== undefined && r.start < r.end,
    )
    .sort((a, b) => a.start - b.start);
  const gaps: { start: string; end: string }[] = [];
  let end = ranges[0]?.end;
  for (const range of ranges.slice(1)) {
    if (end !== undefined && range.start > end)
      gaps.push({ start: clockText(end), end: clockText(range.start) });
    end = Math.max(end ?? range.end, range.end);
  }
  return gaps;
}
export function newPeriod(
  schedule: ScheduleDraft,
  draft: SettingsDraft,
): PeriodDraft {
  const last = [...schedule.periods]
    .sort((a, b) => (clockMinutes(a.end) ?? 0) - (clockMinutes(b.end) ?? 0))
    .at(-1);
  const start = last ? clockMinutes(last.end) : 540;
  return {
    id: crypto.randomUUID(),
    start: start !== undefined && start < 1440 ? clockText(start) : "",
    end:
      start !== undefined && start < 1440
        ? clockText(Math.min(start + (last ? 120 : 180), 1440))
        : "",
    work_minutes: last?.work_minutes ?? draft.work_minutes,
    break_minutes: last?.break_minutes ?? draft.break_minutes,
    overrides: [],
  };
}
export function errorKey(
  issue: Pick<ValidationError, "schedule_id" | "period_id" | "field">,
) {
  return [issue.schedule_id ?? "", issue.period_id ?? "", issue.field].join(
    "/",
  );
}
/** 只转换可序列化的表单值；冲突与运行规则仍由 Rust 统一校验。 */
export function prepareSettings(draft: SettingsDraft): {
  settings?: Settings;
  errors: ValidationError[];
} {
  const value = structuredClone(draft);
  const errors: ValidationError[] = [];
  function report(
    field: string,
    code: string,
    sid: string | null = null,
    pid: string | null = null,
  ) {
    errors.push({ schedule_id: sid, period_id: pid, field, code });
  }
  function integer(
    raw: NumericDraft | undefined,
    min: number,
    max: number,
    field: string,
    code: string,
    sid: string | null = null,
    pid: string | null = null,
  ) {
    if (
      raw === "" ||
      raw === undefined ||
      !Number.isInteger(Number(raw)) ||
      Number(raw) < min ||
      Number(raw) > max
    )
      report(field, code, sid, pid);
    return Number(raw);
  }
  value.work_minutes = integer(
    value.work_minutes,
    1,
    240,
    "work_minutes",
    "work_range",
  );
  value.break_minutes = integer(
    value.break_minutes,
    1,
    60,
    "break_minutes",
    "break_range",
  );
  value.widget_size = integer(
    value.widget_size,
    40,
    140,
    "widget_size",
    "widget_range",
  );
  value.tips.forEach((tip, index) => {
    if (!tip.text.trim() && !tip.guidance)
      report(`tips.${index}.text`, "reminder_empty");
  });
  for (const schedule of value.schedules) {
    if (
      schedule.repeat.kind === "once" &&
      !/^\d{4}-\d{2}-\d{2}$/.test(schedule.repeat.date ?? "")
    )
      report("date", "invalid_date", schedule.id);
    if (schedule.repeat.kind === "weekly" && !schedule.repeat.weekdays?.length)
      report("weekdays", "invalid_weekdays", schedule.id);
    for (const period of schedule.periods) {
      const sid = schedule.id,
        pid = period.id;
      const start = clockMinutes(period.start, false),
        end = clockMinutes(period.end);
      if (start === undefined) report("start", "invalid_time", sid, pid);
      if (end === undefined) report("end", "invalid_time", sid, pid);
      if (start !== undefined && end !== undefined && end <= start)
        report("end", "end_before_start", sid, pid);
      period.work_minutes = integer(
        period.work_minutes,
        1,
        240,
        "work_minutes",
        "work_range",
        sid,
        pid,
      );
      period.break_minutes = integer(
        period.break_minutes,
        1,
        60,
        "break_minutes",
        "break_range",
        sid,
        pid,
      );
      (period.overrides ?? []).forEach((round, index) => {
        round.round = integer(
          round.round,
          1,
          4294967295,
          `overrides.${index}.round`,
          "round_number",
          sid,
          pid,
        );
        for (const [key, max, code] of [
          ["work_minutes", 240, "work_range"],
          ["break_minutes", 60, "break_range"],
        ] as const) {
          if (round[key] === "" || round[key] === undefined) delete round[key];
          else
            round[key] = integer(
              round[key],
              1,
              max,
              `overrides.${index}.${key}`,
              code,
              sid,
              pid,
            );
        }
      });
    }
  }
  return errors.length ? { errors } : { settings: value as Settings, errors };
}
