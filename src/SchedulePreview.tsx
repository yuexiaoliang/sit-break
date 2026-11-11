import { useEffect, useRef, useState } from "react";
import {
  Alert,
  Divider,
  Loader,
  Modal,
  ScrollArea,
  SegmentedControl,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { fmt } from "./i18n";
import type { Strings } from "./components";
import type { Settings, Guidance, RepeatDraft } from "./model";
import type { TimerState } from "./timer-state";

type PreviewMode = "plan" | "progress";

interface PreviewRound {
  number: number;
  work_start: number;
  break_start: number | null;
  break_end: number | null;
  work_seconds: number;
  break_seconds: number;
  content: string | null;
  guidance: Guidance | null;
}
interface PreviewData {
  current: {
    state: TimerState;
    break_start: number | null;
    break_end: number | null;
  } | null;
  schedules: {
    id: string;
    name: string;
    enabled: boolean;
    date: string;
    applies_today: boolean;
    uses_progress: boolean;
    repeat: RepeatDraft;
    periods: {
      id: string;
      start: string;
      end: string;
      rounds: PreviewRound[];
    }[];
    gaps: { start: number; end: number }[];
  }[];
}
function clock(seconds: number | null) {
  if (seconds === null) return "—";
  if (seconds === 86400) return "24:00";
  const days = Math.floor(seconds / 86400),
    day = ((seconds % 86400) + 86400) % 86400;
  return `${String(Math.floor(day / 3600)).padStart(2, "0")}:${String(Math.floor(day / 60) % 60).padStart(2, "0")}${days ? ` (${days > 0 ? "+" : ""}${days})` : ""}`;
}
export function SchedulePreview({
  opened,
  onClose,
  getDraft,
  S,
}: {
  opened: boolean;
  onClose: () => void;
  getDraft: () => Settings;
  S: Strings;
}) {
  const [mode, setMode] = useState<PreviewMode>("plan");
  const [data, setData] = useState<PreviewData | null>(null),
    [error, setError] = useState("");
  const getter = useRef(getDraft);
  getter.current = getDraft;
  useEffect(() => {
    if (!opened) return;
    let active = true,
      request = 0;
    setData(null);
    setError("");
    async function refresh() {
      const revision = ++request;
      try {
        const next = await invoke<PreviewData>("preview_schedule", {
          draft: getter.current(),
          mode,
        });
        if (active && revision === request) {
          setData(next);
          setError("");
        }
      } catch (failure) {
        if (active && revision === request) {
          setData(null);
          const response = failure as {
            errors?: { code: string }[];
            message?: string;
          };
          setError(
            response.errors
              ?.map((e) => S[`validation_${e.code}`] ?? S.fixSchedule)
              .join("\n") ??
              response.message ??
              String(failure),
          );
        }
      }
    }
    void refresh();
    const off = mode === "progress" ? listen("tick", refresh) : undefined;
    return () => {
      active = false;
      void off?.then((fn) => fn());
    };
  }, [opened, mode, S]);
  const rhythm = (work: number, rest: number) =>
    fmt(S.previewRhythm, { work: work / 60, rest: rest / 60 });
  const current = data?.current;
  const currentHasBreak =
    current && (current.state.mode === "break" || current.break_start !== null);
  return (
    <Modal
      opened={opened}
      onClose={() => {
        onClose();
        setMode("plan");
      }}
      title={
        <div>
          <Title order={3}>{S.previewTitle}</Title>
          <Text size="xs" c="dimmed" mt={4}>
            {S.previewCaption}
          </Text>
        </div>
      }
      size={520}
      scrollAreaComponent={ScrollArea.Autosize}
      classNames={{ content: "schedule-preview", body: "preview-body" }}
      closeButtonProps={{ "aria-label": S.closePreview }}
    >
      <SegmentedControl
        fullWidth
        size="sm"
        aria-label={S.previewTitle}
        value={mode}
        onChange={(value) => {
          setData(null);
          setError("");
          setMode(value as PreviewMode);
        }}
        data={[
          { value: "plan", label: S.previewPlan },
          { value: "progress", label: S.previewProgress },
        ]}
      />
      {!data && !error && (
        <Stack align="center" p="xl">
          <Loader size={24} />
          <Text size="sm" c="dimmed">
            {S.previewLoading}
          </Text>
        </Stack>
      )}
      {error && (
        <Alert color="red" title={S.previewUnavailable} role="alert" mt={16}>
          {error}
        </Alert>
      )}
      {data && (
        <div
          id="previewBody"
          className="preview-content"
          role="region"
          aria-label={S.previewContent}
          tabIndex={0}
        >
          {current && (
            <section className="preview-current">
              <Title order={4}>
                {S.previewCurrentRound} ·{" "}
                {current.state.round
                  ? fmt(S.roundNumber, { n: current.state.round.number })
                  : S.simpleTitle}
              </Title>
              <Text size="sm" mt={5}>
                {current.state.paused
                  ? S.paused
                  : current.state.mode === "break"
                    ? S.onBreak
                    : current.state.mode === "remind"
                      ? S.reminding
                      : S.working}
                {currentHasBreak &&
                  ` · ${rhythm(current.state.work_seconds, current.state.break_seconds)}`}
              </Text>
              <Text size="sm" c="plum" mt={6}>
                {current.state.mode === "break"
                  ? fmt(S.previewBreakEnds, {
                      end: clock(current.break_end),
                    })
                  : currentHasBreak
                    ? fmt(S.previewBreakTimes, {
                        start: clock(current.break_start),
                        end: clock(current.break_end),
                      })
                    : S.previewNoBreakBeforeEnd}
              </Text>
              {currentHasBreak && current.state.mode === "remind" && (
                <Text size="xs" c="dimmed" mt={5}>
                  {S.previewWaiting}
                </Text>
              )}
              {currentHasBreak && current.state.tip && (
                <Text size="sm" className="pre-wrap" mt={8}>
                  {current.state.tip}
                </Text>
              )}
              {currentHasBreak && !current.state.tip && !current.state.guidance && (
                <Text size="sm" mt={8}>
                  {S.previewRandomTip}
                </Text>
              )}
            </section>
          )}
          {!data.schedules.length && (
            <Text size="sm" c="dimmed">
              {S.emptySchedulesDescription}
            </Text>
          )}
          {data.schedules.map((schedule) => (
            <section className="preview-schedule" key={schedule.id}>
              <Title order={4}>{schedule.name || S.previewUnnamed}</Title>
              <Text size="xs" c="dimmed" mt={4}>
                {schedule.repeat.kind === "once"
                  ? `${S.repeatOnce} · ${schedule.repeat.date}`
                  : `${S.repeatWeekly} · ${(schedule.repeat.weekdays ?? []).map((day) => S[`weekday${day}`]).join(" / ")}`}
              </Text>
              {(!schedule.enabled || mode === "progress") && (
                <Text size="xs" c="plum" mt={5}>
                  {!schedule.enabled
                    ? S.previewDisabled
                    : schedule.uses_progress
                      ? S.previewUsingProgress
                      : S.previewFromStart}
                </Text>
              )}
              {mode === "progress" && schedule.repeat.kind === "weekly" && (
                <Text size="xs" c="dimmed" mt={4}>
                  {schedule.date}
                  {schedule.applies_today ? "" : ` · ${S.previewNotToday}`}
                </Text>
              )}
              {!schedule.periods.length && (
                <Text size="sm" c="dimmed" mt={12}>
                  {S.emptyPeriodsDescription}
                </Text>
              )}
              {schedule.periods.map((period) => {
                const round = current?.state.round;
                const currentHere =
                  schedule.uses_progress &&
                  round?.schedule_id === schedule.id &&
                  round.period_id === period.id &&
                  round.date === schedule.date;
                const currentBreak = currentHere ? current?.break_start : null;
                return (
                  <section className="preview-period" key={period.id}>
                    <Text fw={600} size="sm">
                      {period.start}–{period.end} · {S.executionPeriod}
                    </Text>
                    {currentBreak != null && (
                      <Text c="plum" size="xs" mt={6}>
                        {S.previewNextBreak} · {clock(currentBreak)}
                      </Text>
                    )}
                    {!period.rounds.length && (
                      <Text size="xs" c="dimmed" mt={8}>
                        {schedule.uses_progress
                          ? S.previewNoUpcoming
                          : S.previewNoCompleteRounds}
                      </Text>
                    )}
                    {period.rounds.map((item) => (
                      <div className="preview-round" key={item.number}>
                        <Text fw={600} size="sm">
                          {fmt(S.roundNumber, { n: item.number })}
                        </Text>
                        <Text size="xs" c="dimmed" mt={4}>
                          {item.break_start === null
                            ? fmt(S.previewWorkTimes, {
                                start: clock(item.work_start),
                                end: period.end,
                              })
                            : `${S.previewWorkStarts} ${clock(item.work_start)} · ${rhythm(item.work_seconds, item.break_seconds)}`}
                        </Text>
                        <Text size="xs" c="plum" mt={4}>
                          {item.break_start === null
                            ? S.previewNoBreakBeforeEnd
                            : fmt(S.previewBreakTimes, {
                                start: clock(item.break_start),
                                end: clock(item.break_end),
                              })}
                        </Text>
                        {item.break_start !== null && (
                          <Text size="sm" mt={8} className="pre-wrap">
                            {item.content ||
                              (item.guidance ? S.guidance : S.previewRandomTip)}
                          </Text>
                        )}
                      </div>
                    ))}
                    {schedule.gaps
                      .filter((gap) => clock(gap.start) === period.end)
                      .map((gap) => (
                        <Text key={gap.start} size="xs" c="dimmed" mt={10}>
                          {fmt(S.quietPeriod, {
                            start: clock(gap.start),
                            end: clock(gap.end),
                          })}
                        </Text>
                      ))}
                  </section>
                );
              })}
            </section>
          ))}
          {mode === "progress" && (
            <>
              <Divider my={18} />
              <Text size="xs" c="dimmed" id="previewNote">
                {S.previewNote}
                {current?.state.paused ? ` ${S.previewPaused}` : ""}
              </Text>
            </>
          )}
        </div>
      )}
    </Modal>
  );
}
