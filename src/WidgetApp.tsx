import { useEffect, useRef, useState } from "react";
import { RingProgress } from "@mantine/core";
import { invoke } from "@tauri-apps/api/core";
import {
  availableMonitors,
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
} from "@tauri-apps/api/window";
import { fmt } from "./i18n";
import { formatTime, useRuntime, useStrings } from "./runtime";
import type { Settings } from "./model";
export default function WidgetApp() {
  const { timer, settings, platform } = useRuntime(),
    S = useStrings();
  const [size, setSize] = useState(settings.widget_size),
    ball = useRef<HTMLDivElement>(null);
  const sizeRef = useRef(size);
  sizeRef.current = size;
  useEffect(() => {
    setSize(settings.widget_size);
  }, [settings.widget_size]);
  useEffect(() => {
    void getCurrentWindow()
      .setSize(
        new LogicalSize(
          platform === "windows" ? Math.max(size + 24, 140) : size + 24,
          size + 24,
        ),
      )
      .catch(console.error);
  }, [size, platform]);
  useEffect(() => {
    const win = getCurrentWindow();
    let moveTimer: ReturnType<typeof setTimeout>,
      stopped = false;
    const off = win.onMoved(({ payload }) => {
      clearTimeout(moveTimer);
      moveTimer = setTimeout(
        () => localStorage.setItem("widgetPos", JSON.stringify(payload)),
        400,
      );
    });
    async function restore() {
      const raw = localStorage.getItem("widgetPos");
      if (!raw) return;
      try {
        const { x, y } = JSON.parse(raw);
        if (!Number.isFinite(x) || !Number.isFinite(y)) return;
        const outer = await win.outerSize(),
          monitors = await availableMonitors();
        const area = monitors.find(
          ({ workArea: a }) =>
            x + outer.width / 2 >= a.position.x &&
            x + outer.width / 2 < a.position.x + a.size.width &&
            y + outer.height / 2 >= a.position.y &&
            y + outer.height / 2 < a.position.y + a.size.height,
        )?.workArea;
        if (area && !stopped)
          await win.setPosition(
            new PhysicalPosition(
              Math.max(
                area.position.x,
                Math.min(x, area.position.x + area.size.width - outer.width),
              ),
              Math.max(
                area.position.y,
                Math.min(y, area.position.y + area.size.height - outer.height),
              ),
            ),
          );
      } catch {
        /* 显示器或旧位置失效时保留默认位置。 */
      }
    }
    void restore();
    return () => {
      stopped = true;
      clearTimeout(moveTimer);
      void off.then((fn) => fn());
    };
  }, []);
  useEffect(() => {
    let save: ReturnType<typeof setTimeout>;
    function wheel(event: WheelEvent) {
      event.preventDefault();
      const next = Math.max(
        40,
        Math.min(140, sizeRef.current + (event.deltaY < 0 ? 4 : -4)),
      );
      sizeRef.current = next;
      setSize(next);
      clearTimeout(save);
      save = setTimeout(() => {
        void invoke<Settings>("get_settings")
          .then((latest) =>
            invoke("save_settings", {
              settings: { ...latest, widget_size: next },
            }),
          )
          .catch(console.error);
      }, 500);
    }
    const element = ball.current!;
    element.addEventListener("wheel", wheel, { passive: false });
    return () => {
      element.removeEventListener("wheel", wheel);
      clearTimeout(save);
    };
  }, []);
  const total =
    timer.mode === "break" ? timer.break_seconds : timer.work_seconds;
  const progress = Math.max(
    0,
    Math.min(100, total ? (timer.remaining / total) * 100 : 0),
  );
  const status = [
    timer.paused ? S.paused : "",
    { work: S.working, remind: S.reminding, break: S.onBreak, idle: S.idle }[
      timer.mode
    ],
    timer.round ? fmt(S.roundNumber, { n: timer.round.number }) : "",
    timer.mode === "idle"
      ? ""
      : fmt(S.actualDurations, {
          work: timer.work_seconds / 60,
          rest: timer.break_seconds / 60,
        }),
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <div
      ref={ball}
      className={`timer-ball${timer.paused ? " paused" : ""}`}
      id="ball"
      style={{ width: size, height: size }}
      role="timer"
      title={status}
      aria-label={`${formatTime(timer.remaining)} · ${status}`}
      onMouseDown={(event) => {
        if (event.button === 0) {
          event.preventDefault();
          void getCurrentWindow().startDragging().catch(console.error);
        }
      }}
    >
      <RingProgress
        size={size}
        thickness={Math.max(2, size * 0.045)}
        roundCaps
        sections={[
          { value: progress, color: timer.mode === "idle" ? "gray" : "plum" },
        ]}
        rootColor="var(--surface-border)"
        label={
          <div
            id="num"
            className="widget-time"
            style={{
              fontSize: Math.round(
                size * (timer.mode === "idle" ? 0.24 : 0.27),
              ),
            }}
          >
            {timer.mode === "idle" ? S.idle : formatTime(timer.remaining)}
          </div>
        }
      />
    </div>
  );
}
