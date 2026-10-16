import { Button, Group, SegmentedControl, Stack, Text } from "@mantine/core";
import { CalendarBlankIcon, ClockIcon, PlusIcon } from "@phosphor-icons/react";
import { DurationControl, plainLabel, type Strings } from "./components";
import { fmt } from "./i18n";
import {
  clockMinutes,
  errorKey,
  type ScheduleDraft,
  type SettingsDraft,
  type ValidationError,
} from "./model";

type TimerField = "work_minutes" | "break_minutes" | "use_schedule";

function repeatLabel(schedule: ScheduleDraft, S: Strings) {
  if (schedule.repeat.kind === "once")
    return `${S.repeatOnce} · ${schedule.repeat.date || S.onceDate}`;
  const days = [...new Set(schedule.repeat.weekdays ?? [])].sort();
  if (days.length === 7) return S.everyDay;
  if (days.join() === "1,2,3,4,5") return S.workdays;
  return (
    days.map((day) => S[`weekday${day}`]).join(S.weekdaySeparator) ||
    S.repeatWeekly
  );
}

export function TimerSettings({
  draft,
  onChange,
  onEditSchedules,
  onAddSchedule,
  errors,
  S,
}: {
  draft: SettingsDraft;
  onChange: <K extends TimerField>(key: K, value: SettingsDraft[K]) => void;
  onEditSchedules: () => void;
  onAddSchedule: () => void;
  errors: ValidationError[];
  S: Strings;
}) {
  const enabled = draft.schedules.filter((schedule) => schedule.enabled);
  const durationErrors = errors.filter(
    (error) =>
      !error.schedule_id &&
      (error.field === "work_minutes" || error.field === "break_minutes"),
  );
  function correctSimple() {
    onChange("use_schedule", false);
    requestAnimationFrame(() =>
      document
        .getElementById(
          durationErrors[0]?.field === "break_minutes" ? "breakMin" : "workMin",
        )
        ?.focus(),
    );
  }
  return (
    <div className="timer-configuration">
      <Text size="sm" fw={600} id="timer-mode-label" mb={10}>
        {S.timingMode}
      </Text>
      <SegmentedControl
        className="timer-mode-picker"
        name="timingMode"
        aria-labelledby="timer-mode-label"
        value={draft.use_schedule ? "scheduled" : "simple"}
        onChange={(value) => onChange("use_schedule", value === "scheduled")}
        fullWidth
        withItemsBorders={false}
        transitionDuration={160}
        data={[
          {
            value: "simple",
            label: (
              <span className="timer-mode-label">
                <ClockIcon size={18} aria-hidden="true" />
                {S.simpleTitle}
              </span>
            ),
          },
          {
            value: "scheduled",
            label: (
              <span className="timer-mode-label">
                <CalendarBlankIcon size={18} aria-hidden="true" />
                {S.scheduledTitle}
              </span>
            ),
          },
        ]}
      />
      <div
        className="timer-mode-panel"
        hidden={draft.use_schedule}
        role="region"
        aria-label={S.simpleTitle}
      >
        <div className="durations">
          {(["work_minutes", "break_minutes"] as const).map((field) => {
            const isWork = field === "work_minutes";
            const error = durationErrors.find((issue) => issue.field === field);
            return (
              <div
                key={field}
                data-error-key={errorKey({
                  schedule_id: null,
                  period_id: null,
                  field,
                })}
              >
                <DurationControl
                  id={isWork ? "workMin" : "breakMin"}
                  label={isWork ? S.workLength : S.breakLen}
                  value={draft[field]}
                  max={isWork ? 240 : 60}
                  presets={isWork ? [15, 30, 45, 60] : [3, 5, 10]}
                  S={S}
                  error={error ? S[`validation_${error.code}`] : undefined}
                  onChange={(value) => onChange(field, value)}
                />
              </div>
            );
          })}
        </div>
        <Text size="sm" c="dimmed" className="simple-summary">
          {fmt(S.simpleSummary, {
            work: draft.work_minutes === "" ? "—" : draft.work_minutes,
            rest: draft.break_minutes === "" ? "—" : draft.break_minutes,
          })}
        </Text>
      </div>
      <div
        className="timer-mode-panel schedule-overview"
        hidden={!draft.use_schedule}
        role="region"
        aria-label={S.scheduleOverview}
      >
        <Text size="sm" c="dimmed" className="timer-mode-description">
          {S.scheduledDescription}
        </Text>
        {enabled.length ? (
          <>
            <Group
              justify="space-between"
              wrap="wrap"
              className="schedule-overview-heading"
            >
              <Text size="xs" c="dimmed">
                {fmt(S.enabledScheduleCount, { n: enabled.length })}
              </Text>
              <Button
                variant="subtle"
                size="compact-sm"
                onClick={onEditSchedules}
              >
                {S.editSchedule}
              </Button>
            </Group>
            <Stack gap={10}>
              {enabled.map((schedule) => (
                <article className="schedule-overview-card" key={schedule.id}>
                  <Text fw={600} size="sm" className="schedule-overview-name">
                    {schedule.name || S.previewUnnamed}
                  </Text>
                  <Text size="xs" c="dimmed" mt={3}>
                    {repeatLabel(schedule, S)}
                  </Text>
                  {schedule.periods.length ? (
                    <div className="schedule-overview-periods">
                      {schedule.periods.map((period) => (
                        <div
                          className="schedule-overview-period"
                          key={period.id}
                        >
                          <Text
                            size="sm"
                            fw={550}
                            className="schedule-overview-time"
                          >
                            {clockMinutes(period.start, false) === undefined
                              ? "—"
                              : period.start}
                            –
                            {clockMinutes(period.end) === undefined
                              ? "—"
                              : period.end}
                          </Text>
                          <Text size="xs" c="dimmed">
                            {fmt(S.scheduleDefaultRhythm, {
                              work:
                                period.work_minutes === ""
                                  ? "—"
                                  : period.work_minutes,
                              rest:
                                period.break_minutes === ""
                                  ? "—"
                                  : period.break_minutes,
                            })}
                          </Text>
                          {!!period.overrides?.length && (
                            <Text size="xs" c="dimmed">
                              {fmt(S.scheduleAdjustedRounds, {
                                n: period.overrides.length,
                              })}
                            </Text>
                          )}
                        </div>
                      ))}
                    </div>
                  ) : (
                    <Text size="xs" c="dimmed" mt={12}>
                      {S.emptyPeriodsDescription}
                    </Text>
                  )}
                </article>
              ))}
            </Stack>
          </>
        ) : (
          <div className="schedule-mode-empty">
            <CalendarBlankIcon size={29} aria-hidden="true" />
            <Text fw={600} size="sm">
              {draft.schedules.length ? S.noEnabledSchedules : S.emptySchedules}
            </Text>
            <Text size="xs" c="dimmed">
              {draft.schedules.length
                ? S.noEnabledSchedulesDescription
                : S.emptySchedulesDescription}
            </Text>
            <Button
              variant="light"
              size="compact-sm"
              leftSection={!draft.schedules.length && <PlusIcon size={16} />}
              onClick={draft.schedules.length ? onEditSchedules : onAddSchedule}
            >
              {draft.schedules.length
                ? S.editSchedule
                : plainLabel(S.addSchedule)}
            </Button>
          </div>
        )}
        {!!durationErrors.length && (
          <div className="inactive-duration-error" role="alert">
            <Text size="xs">{S.simpleNeedsCorrection}</Text>
            <Button
              variant="light"
              size="compact-sm"
              color="red"
              onClick={correctSimple}
              data-error-key={errorKey(durationErrors[0])}
            >
              {S.correctSimple}
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
