import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { STR, initLang, fmt } from "./i18n";

interface Tick {
  mode: "work" | "remind" | "break";
  remaining: number;
  paused: boolean;
}

interface Settings {
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

function fmtTime(secs: number): string {
  return `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;
}

function renderTick(t: Tick) {
  ptime.textContent = fmtTime(t.remaining);
  if (t.paused) {
    pmode.textContent = S.paused;
    pdot.classList.add("paused");
    pPauseText.textContent = S.resume;
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
  if (s.work_minutes !== lastWork) {
    renderChips(ivChips, INTERVAL_PRESETS, s.work_minutes, "apply_interval");
    lastWork = s.work_minutes;
  }
  if (s.break_minutes !== lastBreak) {
    renderChips(bkChips, BREAK_PRESETS, s.break_minutes, "apply_break");
    lastBreak = s.break_minutes;
  }
  ivCur.textContent = fmt(S.everyN, { n: s.work_minutes });
  bkCur.textContent = fmt(S.forN, { n: s.break_minutes });
}

listen<Tick>("tick", ({ payload }) => renderTick(payload));
renderTick(await invoke<Tick>("timer_info"));

pPause.addEventListener("click", () => invoke("toggle_pause"));
document.getElementById("pReset")!.addEventListener("click", () => invoke("reset_timer"));
document.getElementById("pSettings")!.addEventListener("click", () => invoke("open_settings"));
document.getElementById("pQuit")!.addEventListener("click", () => invoke("quit_app"));
window.addEventListener("contextmenu", (e) => e.preventDefault());

(async () => {
  S = STR[await initLang()];
  await refresh();
})();
listen("settings_changed", async () => {
  S = STR[await initLang()];
  await refresh();
});
