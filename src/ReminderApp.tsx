import { useCallback, useEffect, useRef, useState } from "react";
import { Alert, Badge, Button, Progress, Text } from "@mantine/core";
import { ClockIcon, CoffeeIcon, LightbulbIcon } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { GuidanceVideo } from "./Guidance";
import { fmt } from "./i18n";
import { formatTime, useRuntime, useStrings } from "./runtime";

function chime() {
  try {
    const context = new AudioContext(),
      now = context.currentTime;
    for (const [frequency, at] of [
      [880, 0],
      [1174.66, 0.18],
    ]) {
      const oscillator = context.createOscillator(),
        gain = context.createGain();
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0.0001, now + at);
      gain.gain.exponentialRampToValueAtTime(0.18, now + at + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + at + 0.4);
      oscillator.connect(gain).connect(context.destination);
      oscillator.start(now + at);
      oscillator.stop(now + at + 0.45);
    }
    setTimeout(() => {
      void context.close();
    }, 800);
  } catch {
    /* 系统音频不可用时仍显示提醒。 */
  }
}
export default function ReminderApp() {
  const { timer } = useRuntime(),
    S = useStrings();
  const content = useRef<HTMLDivElement>(null),
    viewport = useRef<HTMLDivElement>(null);
  const [failure, setFailure] = useState("");
  const plain = timer.mode === "break" && !timer.guidance;
  const round = timer.round
    ? `${fmt(S.roundNumber, { n: timer.round.number })} · `
    : "";
  const fit = useCallback(() => {
    if (!plain || !content.current || !viewport.current) return;
    const height = Math.max(
      244,
      Math.min(
        368,
        Math.ceil(
          window.innerHeight -
            viewport.current.getBoundingClientRect().height +
            Math.max(26, content.current.getBoundingClientRect().height),
        ),
      ),
    );
    if (height !== window.innerHeight)
      void invoke("fit_reminder_height", { height }).catch(console.error);
  }, [plain]);
  useEffect(() => {
    const observer = new ResizeObserver(() => requestAnimationFrame(fit));
    if (content.current) observer.observe(content.current);
    window.addEventListener("resize", fit);
    const frame = requestAnimationFrame(fit);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", fit);
      cancelAnimationFrame(frame);
    };
  }, [fit, timer.tip]);
  useEffect(() => {
    const off = listen<{ sound: boolean }>("reminder", ({ payload }) => {
      if (payload.sound) chime();
    });
    return () => {
      void off.then((fn) => fn());
    };
  }, []);
  async function command(name: string) {
    try {
      setFailure("");
      await invoke(name);
    } catch {
      setFailure(S.operationFailed);
    }
  }
  if (timer.mode !== "break")
    return (
      <div className="reminder-surface reminder-prompt" id="cardRemind">
        <Badge
          variant="light"
          leftSection={<ClockIcon size={15} />}
        >{`${timer.paused ? `${S.paused} · ` : ""}${round}${fmt(S.satFor, { n: timer.work_seconds / 60 })}`}</Badge>
        <h1>{S.timeToMove}</h1>
        <Text size="sm" c="dimmed" className="reminder-subtitle">
          {S.standUp}
        </Text>
        {timer.tip?.trim() && (
          <div className="reminder-tip">
            <LightbulbIcon size={19} />
            <Text size="sm" id="tipText">
              {timer.tip}
            </Text>
          </div>
        )}
        {failure && (
          <Alert color="red" p="xs">
            {failure}
          </Alert>
        )}
        <div className="reminder-actions">
          <Button id="btnBreak" onClick={() => void command("start_break")}>
            {fmt(S.startBreak, { n: timer.break_seconds / 60 })}
          </Button>
          <Button
            variant="subtle"
            id="btnPostpone"
            onClick={() => void command("postpone")}
          >
            {S.snooze}
          </Button>
        </div>
      </div>
    );
  return (
    <div
      className={`reminder-surface break-card${timer.guidance ? " has-guidance" : ""}`}
      id="cardBreak"
    >
      {timer.guidance && (
        <div className="guidance-content" id="guidanceContent">
          <GuidanceVideo guidance={timer.guidance} S={S} />
        </div>
      )}
      <div className="break-details">
        <div className="break-header">
          <Badge variant="light" leftSection={<CoffeeIcon size={17} />}>
            {S.onBreak}
            {timer.paused ? ` · ${S.paused}` : ""}
          </Badge>
          <div className="break-clock">
            <span
              className="time"
              id="breakTime"
              role="timer"
              aria-label={fmt(S.untilBack, { t: formatTime(timer.remaining) })}
            >
              {formatTime(timer.remaining)}
            </span>
            <Text size="xs" c="dimmed" id="ringCap">
              {round}
              {S.remainingTime}
            </Text>
          </div>
        </div>
        <Progress
          className="break-progress"
          id="breakProgress"
          size={4}
          color="plum"
          value={Math.max(
            0,
            Math.min(100, (timer.remaining / (timer.break_seconds || 1)) * 100),
          )}
          aria-hidden="true"
        />
        <div
          className="stretch"
          id="stretchText"
          tabIndex={0}
          ref={viewport}
          hidden={Boolean(timer.guidance) && !timer.tip?.trim()}
        >
          <div id="stretchContent" ref={content}>
            {timer.tip ?? ""}
          </div>
        </div>
        {failure && (
          <Alert color="red" p="xs">
            {failure}
          </Alert>
        )}
        <Button
          fullWidth
          variant="light"
          id="btnEnd"
          onClick={() => void command("end_break")}
        >
          {S.endBreak}
        </Button>
      </div>
    </div>
  );
}
