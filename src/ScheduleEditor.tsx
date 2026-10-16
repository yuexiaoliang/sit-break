import { useEffect, useState } from "react";
import {
  Accordion,
  ActionIcon,
  Alert,
  Button,
  Chip,
  Divider,
  Group,
  NumberInput,
  SegmentedControl,
  Stack,
  Switch,
  Text,
  Textarea,
  TextInput,
  Title,
  Tooltip,
} from "@mantine/core";
import { DateInput } from "@mantine/dates";
import { CalendarBlankIcon, PlusIcon, TrashIcon } from "@phosphor-icons/react";
import {
  ClockField,
  plainLabel,
  type ClockRegistry,
  type Strings,
} from "./components";
import { GuidancePicker } from "./Guidance";
import {
  errorKey,
  localDate,
  newPeriod,
  newSchedule,
  quietGaps,
  type PeriodDraft,
  type ScheduleDraft,
  type SettingsDraft,
  type ValidationError,
} from "./model";
import { fmt } from "./i18n";

export function ScheduleEditor({
  draft,
  onChange,
  onBusy,
  onEditing,
  errors,
  registry,
  S,
  importing,
}: {
  draft: SettingsDraft;
  onChange: (schedules: ScheduleDraft[]) => void;
  onBusy: (delta: number) => void;
  onEditing: () => void;
  errors: ValidationError[];
  registry: ClockRegistry;
  S: Strings;
  importing: boolean;
}) {
  const [expanded, setExpanded] = useState<string[]>([]);
  useEffect(() => {
    const ids = errors
      .filter((e) => e.period_id && e.field.startsWith("overrides."))
      .map((e) => e.period_id!);
    if (ids.length)
      setExpanded((current) => [...new Set([...current, ...ids])]);
  }, [errors]);
  function error(sid: string, pid: string | null, field: string) {
    const issue = errors.find(
      (e) => e.schedule_id === sid && e.period_id === pid && e.field === field,
    );
    return issue ? (S[`validation_${issue.code}`] ?? S.fixSchedule) : undefined;
  }
  function fieldKey(sid: string, pid: string | null, field: string) {
    return errorKey({ schedule_id: sid, period_id: pid, field });
  }
  function update(schedule: ScheduleDraft) {
    onChange(draft.schedules.map((s) => (s.id === schedule.id ? schedule : s)));
  }
  function updatePeriod(schedule: ScheduleDraft, period: PeriodDraft) {
    update({
      ...schedule,
      periods: schedule.periods.map((p) => (p.id === period.id ? period : p)),
    });
  }
  function focusLast(selector: string) {
    requestAnimationFrame(() => {
      const fields = document.querySelectorAll<HTMLInputElement>(selector);
      fields[fields.length - 1]?.focus();
    });
  }
  return (
    <Stack gap={24} id="scheduleList">
      {!draft.use_schedule && (
        <Alert variant="light" color="plum" icon={<CalendarBlankIcon />}>
          {S.scheduleOffNote}
        </Alert>
      )}
      {!draft.schedules.length && (
        <div className="empty-state" id="emptySchedules">
          <CalendarBlankIcon size={32} />
          <Title order={3}>{S.emptySchedules}</Title>
          <Text c="dimmed" size="sm">
            {S.emptySchedulesDescription}
          </Text>
        </div>
      )}
      {draft.schedules.map((schedule) => (
        <article
          className="schedule-card"
          data-schedule-id={schedule.id}
          key={schedule.id}
        >
          <Group
            justify="space-between"
            wrap="nowrap"
            align="end"
            className="schedule-heading"
          >
            <TextInput
              label={S.scheduleName}
              value={schedule.name}
              maxLength={100}
              onChange={(e) =>
                update({ ...schedule, name: e.currentTarget.value })
              }
              data-field="name"
              className="schedule-name"
            />
            <Switch
              aria-label={`${S.enableSchedule} · ${schedule.name || S.newSchedule}`}
              checked={schedule.enabled}
              onChange={(e) =>
                update({ ...schedule, enabled: e.currentTarget.checked })
              }
            />
            <Tooltip label={S.removeSchedule}>
              <ActionIcon
                color="red"
                disabled={importing}
                aria-label={S.removeSchedule}
                onClick={() =>
                  onChange(draft.schedules.filter((s) => s.id !== schedule.id))
                }
              >
                <TrashIcon size={18} />
              </ActionIcon>
            </Tooltip>
          </Group>
          <div className="repeat-settings">
            <Text size="sm" fw={600} mb={6}>
              {S.repeatRule}
            </Text>
            <SegmentedControl
              value={schedule.repeat.kind}
              aria-label={S.repeatRule}
              onChange={(kind) =>
                update({
                  ...schedule,
                  repeat:
                    kind === "once"
                      ? { kind, date: schedule.repeat.date ?? localDate() }
                      : {
                          kind: "weekly",
                          weekdays: schedule.repeat.weekdays ?? [1, 2, 3, 4, 5],
                        },
                })
              }
              data={[
                { value: "weekly", label: S.repeatWeekly },
                { value: "once", label: S.repeatOnce },
              ]}
              fullWidth
            />
          </div>
          {schedule.repeat.kind === "once" ? (
            <DateInput
              previousLabel={S.previousCalendar}
              nextLabel={S.nextCalendar}
              clearButtonProps={{ "aria-label": S.clearDate }}
              value={schedule.repeat.date ?? null}
              valueFormat="YYYY-MM-DD"
              label={S.onceDate}
              placeholder="YYYY-MM-DD"
              clearable
              onChange={(date) =>
                update({
                  ...schedule,
                  repeat: { kind: "once", date: date ?? "" },
                })
              }
              error={error(schedule.id, null, "date")}
              data-error-key={fieldKey(schedule.id, null, "date")}
              data-field="date"
            />
          ) : (
            <div
              className="weekly-settings"
              data-error-key={fieldKey(schedule.id, null, "weekdays")}
            >
              <Text size="sm" fw={600} mb={6}>
                {S.repeatWeekdays}
              </Text>
              <Chip.Group
                multiple
                value={(schedule.repeat.weekdays ?? []).map(String)}
                onChange={(values) =>
                  update({
                    ...schedule,
                    repeat: {
                      kind: "weekly",
                      weekdays: values.map(Number).sort((a, b) => a - b),
                    },
                  })
                }
              >
                <Group gap={5} className="weekday-buttons">
                  {[1, 2, 3, 4, 5, 6, 7].map((day) => (
                    <Chip
                      key={day}
                      value={String(day)}
                      radius={7}
                      size="sm"
                      variant="light"
                    >
                      {S[`weekday${day}`]}
                    </Chip>
                  ))}
                </Group>
              </Chip.Group>
              {error(schedule.id, null, "weekdays") && (
                <Text size="xs" c="red" mt={5}>
                  {error(schedule.id, null, "weekdays")}
                </Text>
              )}
              <Group gap={12} mt={6}>
                {[
                  [1, 2, 3, 4, 5],
                  [1, 2, 3, 4, 5, 6, 7],
                ].map((days, index) => (
                  <Button
                    key={index}
                    variant="subtle"
                    size="compact-xs"
                    onClick={() =>
                      update({
                        ...schedule,
                        repeat: { kind: "weekly", weekdays: days },
                      })
                    }
                  >
                    {index ? S.everyDay : S.workdays}
                  </Button>
                ))}
              </Group>
            </div>
          )}
          <div className="quiet-periods" aria-live="polite">
            {quietGaps(schedule.periods).map((gap) => (
              <Text key={gap.start} size="xs" c="dimmed">
                {fmt(S.quietPeriod, gap)}
              </Text>
            ))}
          </div>
          {!schedule.periods.length && (
            <Text size="sm" c="dimmed" className="empty-periods">
              {S.emptyPeriodsDescription}
            </Text>
          )}
          {schedule.periods.map((period, index) => (
            <section
              className="period-card"
              data-period-id={period.id}
              key={period.id}
            >
              <Group justify="space-between" mb={16}>
                <Text fw={600} size="sm">
                  {S.executionPeriod}{" "}
                  {schedule.periods.length > 1 ? index + 1 : ""}
                </Text>
                <ActionIcon
                  color="red"
                  aria-label={S.removePeriod}
                  disabled={importing}
                  onClick={() =>
                    update({
                      ...schedule,
                      periods: schedule.periods.filter(
                        (p) => p.id !== period.id,
                      ),
                    })
                  }
                >
                  <TrashIcon size={17} />
                </ActionIcon>
              </Group>
              <div className="period-grid">
                {(["start", "end"] as const).map((field) => (
                  <div
                    key={field}
                    data-error-key={fieldKey(schedule.id, period.id, field)}
                  >
                    <ClockField
                      id={`${period.id}/${field}`}
                      field={field}
                      label={field === "start" ? S.periodStart : S.periodEnd}
                      value={period[field]}
                      S={S}
                      registry={registry}
                      onEditing={onEditing}
                      error={error(schedule.id, period.id, field)}
                      onChange={(value) =>
                        updatePeriod(schedule, { ...period, [field]: value })
                      }
                    />
                  </div>
                ))}
                {(["work_minutes", "break_minutes"] as const).map((field) => (
                  <NumberInput
                    key={field}
                    label={
                      field === "work_minutes" ? S.defaultWork : S.defaultBreak
                    }
                    value={period[field]}
                    min={1}
                    max={field === "work_minutes" ? 240 : 60}
                    suffix={` ${S.minutesUnit}`}
                    data-field={field}
                    data-error-key={fieldKey(schedule.id, period.id, field)}
                    error={error(schedule.id, period.id, field)}
                    onChange={(value) =>
                      updatePeriod(schedule, { ...period, [field]: value })
                    }
                  />
                ))}
              </div>
              <Text c="dimmed" size="xs" mt={10} className="field-hint">
                {S.periodEndHint}
              </Text>
              <Accordion
                multiple
                value={expanded.includes(period.id) ? [period.id] : []}
                onChange={(values) =>
                  setExpanded((current) =>
                    values.length
                      ? [...new Set([...current, period.id])]
                      : current.filter((id) => id !== period.id),
                  )
                }
                className="round-adjustments"
                variant="default"
                transitionDuration={220}
              >
                <Accordion.Item value={period.id}>
                  <Accordion.Control>
                    <Text size="sm" fw={500}>
                      {S.roundAdjustments}{" "}
                      <Text span c="dimmed" size="xs">
                        ({period.overrides?.length ?? 0})
                      </Text>
                    </Text>
                  </Accordion.Control>
                  <Accordion.Panel>
                    <Text size="xs" c="dimmed" mb={12}>
                      {S.roundInheritance}
                    </Text>
                    {(period.overrides ?? []).map((round, roundIndex) => {
                      const patch = (changes: Partial<typeof round>) =>
                        updatePeriod(schedule, {
                          ...period,
                          overrides: period.overrides!.map((r, i) =>
                            i === roundIndex ? { ...r, ...changes } : r,
                          ),
                        });
                      return (
                        <div className="round-adjustment" key={roundIndex}>
                          <Group justify="space-between" align="end" mb={12}>
                            <NumberInput
                              label={S.adjustmentRound}
                              value={round.round}
                              min={1}
                              max={4294967295}
                              className="round-number"
                              data-field={`overrides.${roundIndex}.round`}
                              data-error-key={fieldKey(
                                schedule.id,
                                period.id,
                                `overrides.${roundIndex}.round`,
                              )}
                              error={error(
                                schedule.id,
                                period.id,
                                `overrides.${roundIndex}.round`,
                              )}
                              onChange={(value) => patch({ round: value })}
                            />
                            <ActionIcon
                              color="red"
                              aria-label={S.removeAdjustment}
                              disabled={importing}
                              onClick={() =>
                                updatePeriod(schedule, {
                                  ...period,
                                  overrides: period.overrides!.filter(
                                    (_, i) => i !== roundIndex,
                                  ),
                                })
                              }
                            >
                              <TrashIcon size={17} />
                            </ActionIcon>
                          </Group>
                          <div className="period-grid">
                            {(["work_minutes", "break_minutes"] as const).map(
                              (field) => (
                                <NumberInput
                                  key={field}
                                  label={
                                    field === "work_minutes"
                                      ? S.workLength
                                      : S.breakLen
                                  }
                                  value={round[field] ?? ""}
                                  min={1}
                                  max={field === "work_minutes" ? 240 : 60}
                                  placeholder={fmt(S.inheritDefault, {
                                    n: period[field],
                                  })}
                                  data-field={`overrides.${roundIndex}.${field}`}
                                  data-error-key={fieldKey(
                                    schedule.id,
                                    period.id,
                                    `overrides.${roundIndex}.${field}`,
                                  )}
                                  error={error(
                                    schedule.id,
                                    period.id,
                                    `overrides.${roundIndex}.${field}`,
                                  )}
                                  onChange={(value) =>
                                    patch({ [field]: value })
                                  }
                                />
                              ),
                            )}
                          </div>
                          <Textarea
                            label={S.breakContent}
                            value={round.content ?? ""}
                            onChange={(e) =>
                              patch({ content: e.currentTarget.value })
                            }
                            minRows={3}
                            autosize
                            mt={12}
                          />
                          <Text size="xs" c="dimmed" mt={5}>
                            {S.breakContentHint}
                          </Text>
                          <GuidancePicker
                            S={S}
                            value={round.guidance}
                            onChange={(guidance) => patch({ guidance })}
                            onBusy={onBusy}
                            error={error(
                              schedule.id,
                              period.id,
                              `overrides.${roundIndex}.guidance`,
                            )}
                            field={`overrides.${roundIndex}.guidance`}
                          />
                        </div>
                      );
                    })}
                    <Button
                      variant="subtle"
                      size="compact-sm"
                      leftSection={<PlusIcon size={15} />}
                      onClick={() => {
                        const rounds = period.overrides ?? [];
                        let number = 1;
                        while (rounds.some((r) => Number(r.round) === number))
                          number++;
                        updatePeriod(schedule, {
                          ...period,
                          overrides: [...rounds, { round: number }],
                        });
                        focusLast(
                          `[data-period-id="${period.id}"] .round-number input`,
                        );
                      }}
                    >
                      {plainLabel(S.addAdjustment)}
                    </Button>
                  </Accordion.Panel>
                </Accordion.Item>
              </Accordion>
            </section>
          ))}
          <Button
            variant="light"
            size="sm"
            leftSection={<PlusIcon size={16} />}
            onClick={() => {
              update({
                ...schedule,
                periods: [...schedule.periods, newPeriod(schedule, draft)],
              });
              focusLast(
                `[data-schedule-id="${schedule.id}"] .clock-field input[data-field=start]`,
              );
            }}
          >
            {plainLabel(S.addPeriod)}
          </Button>
          <Divider mt={24} />
        </article>
      ))}
      <Button
        variant="light"
        leftSection={<PlusIcon size={17} />}
        id="addSchedule"
        className="add-schedule"
        onClick={() => {
          onChange([...draft.schedules, newSchedule(S.newSchedule)]);
          focusLast(".schedule-name input");
        }}
      >
        {plainLabel(S.addSchedule)}
      </Button>
    </Stack>
  );
}
