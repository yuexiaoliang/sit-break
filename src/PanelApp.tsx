import { useState } from "react";
import { Alert, Button, Group, Text } from "@mantine/core";
import {
  ArrowCounterClockwiseIcon,
  GearSixIcon,
  PauseIcon,
  PlayIcon,
  PowerIcon,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { fmt } from "./i18n";
import { formatTime, useRuntime, useStrings } from "./runtime";
export default function PanelApp() {
  const { settings, timer } = useRuntime(),
    S = useStrings();
  const [failure, setFailure] = useState("");
  async function command(name: string, minutes?: number) {
    try {
      setFailure("");
      await invoke(name, minutes === undefined ? undefined : { minutes });
    } catch {
      setFailure(S.operationFailed);
    }
  }
  const status = timer.paused
    ? S.paused
    : { work: S.working, break: S.onBreak, remind: S.reminding, idle: S.idle }[
        timer.mode
      ];
  function chips(values: number[], current: number, name: string, id: string) {
    return (
      <Group id={id} gap={5} wrap="nowrap" className="panel-presets">
        {(values.includes(current)
          ? values
          : [...values, current].sort((a, b) => a - b)
        ).map((value) => (
          <Button
            size="compact-xs"
            key={value}
            variant={value === current ? "filled" : "default"}
            aria-pressed={value === current}
            onClick={() => void command(name, value)}
          >
            {value}
          </Button>
        ))}
      </Group>
    );
  }
  return (
    <div className="panel-surface pcard">
      <div className="panel-status">
        <span
          className={`status-dot${timer.paused ? " paused" : ""}${timer.mode === "idle" ? " idle" : ""}`}
          id="pdot"
        />
        <Text size="xs" id="pmode">
          {status}
          {timer.round
            ? ` · ${fmt(S.roundNumber, { n: timer.round.number })}`
            : ""}
        </Text>
        <Text id="ptime" fw={650}>
          {timer.mode === "idle" ? "—" : formatTime(timer.remaining)}
        </Text>
      </div>
      {timer.mode === "idle" && (
        <Text size="xs" c="dimmed" id="idleStatus">
          {S.idleStatus}
        </Text>
      )}
      <Group gap={8} grow className="panel-actions">
        <Button
          variant="light"
          size="compact-sm"
          id="pPause"
          leftSection={
            timer.paused ? <PlayIcon size={15} /> : <PauseIcon size={15} />
          }
          onClick={() => void command("toggle_pause")}
        >
          <span id="pPauseText">{timer.paused ? S.resume : S.pause}</span>
        </Button>
        <Button
          variant="default"
          size="compact-sm"
          id="pReset"
          leftSection={<ArrowCounterClockwiseIcon size={15} />}
          onClick={() => void command("reset_timer")}
        >
          {S.reset}
        </Button>
      </Group>
      <div className="panel-section">
        <Group justify="space-between" gap={3}>
          <Text size="xs" c="dimmed">
            {settings.use_schedule ? S.workLength : S.interval}
          </Text>
          <Text size="xs" id="ivCur">
            {timer.work_seconds
              ? fmt(timer.round ? S.forN : S.everyN, {
                  n: timer.work_seconds / 60,
                })
              : "—"}
          </Text>
        </Group>
        {!settings.use_schedule &&
          chips(
            [15, 30, 45, 60],
            settings.work_minutes,
            "apply_interval",
            "ivChips",
          )}
      </div>
      <div className="panel-section">
        <Group justify="space-between" gap={3}>
          <Text size="xs" c="dimmed">
            {S.breakLen}
          </Text>
          <Text size="xs" id="bkCur">
            {timer.break_seconds
              ? fmt(S.forN, { n: timer.break_seconds / 60 })
              : "—"}
          </Text>
        </Group>
        {!settings.use_schedule &&
          chips([3, 5, 10], settings.break_minutes, "apply_break", "bkChips")}
      </div>
      {failure && (
        <Alert color="red" p={5} fz="xs">
          {failure}
        </Alert>
      )}
      <Group justify="space-between" className="panel-footer" gap={0}>
        <Button
          variant="subtle"
          size="compact-sm"
          id="pSettings"
          leftSection={<GearSixIcon size={17} />}
          onClick={() => void command("open_settings")}
        >
          {S.settings}
        </Button>
        <Button
          variant="subtle"
          color="gray"
          size="compact-sm"
          id="pQuit"
          leftSection={<PowerIcon size={17} />}
          onClick={() => void command("quit_app")}
        >
          {S.quit}
        </Button>
      </Group>
    </div>
  );
}
