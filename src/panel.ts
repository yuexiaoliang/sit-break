import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { STR, initLang, fmt } from "./i18n";

import type { TimerState } from "./timer-state";

interface Settings {
  use_schedule: boolean;
  work_minutes: number;
  break_minutes: number;
  sound: boolean;
  show_widget: boolean;
  autostart: boolean;
  language: string;
  widget_size: number;
  tips: string[];
}

const INTERVAL_PRESETS = [15, 30, 45, 60];
const BREAK_PRESETS = [3, 5, 10];

const ptime = document.getElementById("ptime")!;
const pmode = document.getElementById("pmode")!;
const pdot = document.getElementById("pdot")!;
const pPause = document.getElementById("pPause") as HTMLButtonElement;
const pPauseText = document.getElementById("pPauseText")!;
const ivChips = document.getElementById("ivChips")!;
const bkChips = document.getElementById("bkChips")!;
const ivCur = document.getElementById("ivCur")!;
const bkCur = document.getElementById("bkCur")!;

let S = STR.zh;
let lastTick: TimerState | undefined;
let useSchedule = false;

function fmtTime(secs: number): string {
  return `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;
}

function renderTick(t: TimerState) {
  lastTick = t;
  ptime.textContent = t.mode === "idle" ? "—" : fmtTime(t.remaining);
  pdot.classList.toggle("idle", t.mode === "idle");
  document.querySelector(".pcard")!.classList.toggle("idle", t.mode === "idle");
  const idleStatus = document.getElementById("idleStatus")!;
  idleStatus.hidden = t.mode !== "idle";
  idleStatus.textContent = S.idleStatus;
  pPauseText.textContent = t.paused ? S.resume : S.pause;
  if (t.paused) {
    pmode.textContent = S.paused;
    pdot.classList.add("paused");
    pPauseText.textContent = S.resume;
  } else if (t.mode === "idle") {
    pmode.textContent = S.idle;
    pdot.classList.remove("paused");
  } else if (t.mode === "break") {
    pmode.textContent = S.onBreak;
    pdot.classList.remove("paused");
    pPauseText.textContent = S.pause;
  } else if (t.mode === "remind") {
    pmode.textContent = S.reminding;
    pdot.classList.remove("paused");
  } else {
    pmode.textContent = S.working;
    pdot.classList.remove("paused");
    pPauseText.textContent = S.pause;
  }
  if (t.round) pmode.textContent += ` · ${fmt(S.roundNumber, { n: t.round.number })}`;
  ivCur.textContent = t.work_seconds ? fmt(t.round ? S.forN : S.everyN, { n: t.work_seconds / 60 }) : "—";
  bkCur.textContent = t.break_seconds ? fmt(S.forN, { n: t.break_seconds / 60 }) : "—";
}

function renderChips(
  container: HTMLElement,
  presets: number[],
  current: number,
  command: string
) {
  container.innerHTML = "";
  const values = presets.includes(current)
    ? presets
    : [...presets, current].sort((a, b) => a - b);
  for (const v of values) {
    const b = document.createElement("button");
    b.className = "chip" + (v === current ? " active" : "");
    b.textContent = String(v);
    b.addEventListener("click", async () => {
      await invoke(command, { minutes: v });
      await refresh();
    });
    container.appendChild(b);
  }
}

let lastWork = -1;
let lastBreak = -1;

async function refresh() {
  const s = await invoke<Settings>("get_settings");
  useSchedule = s.use_schedule;
  ivChips.hidden = useSchedule;
  bkChips.hidden = useSchedule;
  document.querySelector<HTMLElement>("[data-i18n=interval]")!.textContent = useSchedule ? S.workLength : S.interval;
  if (s.work_minutes !== lastWork) {
    renderChips(ivChips, INTERVAL_PRESETS, s.work_minutes, "apply_interval");
    lastWork = s.work_minutes;
  }
  if (s.break_minutes !== lastBreak) {
    renderChips(bkChips, BREAK_PRESETS, s.break_minutes, "apply_break");
    lastBreak = s.break_minutes;
  }
  if (lastTick) renderTick(lastTick);
}

listen<TimerState>("tick", ({ payload }) => renderTick(payload));
renderTick(await invoke<TimerState>("timer_info"));

pPause.addEventListener("click", () => invoke("toggle_pause"));
document.getElementById("pReset")!.addEventListener("click", () => invoke("reset_timer"));
document.getElementById("pSettings")!.addEventListener("click", () => invoke("open_settings"));
document.getElementById("pQuit")!.addEventListener("click", () => invoke("quit_app"));
window.addEventListener("contextmenu", (e) => e.preventDefault());

(async () => {
  S = STR[await initLang()];
  await refresh();
  if (lastTick) renderTick(lastTick);
})();
listen("settings_changed", async () => {
  S = STR[await initLang()];
  await refresh();
  if (lastTick) renderTick(lastTick);
});
