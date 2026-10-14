import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { STR, initLang, fmt } from "./i18n";
import type { TimerState } from "./timer-state";

interface ReminderPayload {
  breakSeconds: number;
  postponeSeconds: number;
  sound: boolean;
  tip: string;
  satMinutes: number;
}

const cardRemind = document.getElementById("cardRemind")!;
const cardBreak = document.getElementById("cardBreak")!;
const tipText = document.getElementById("tipText")!;
const stretchText = document.getElementById("stretchText")!;
const breakTimeEl = document.getElementById("breakTime")!;
const ringCap = document.getElementById("ringCap")!;
const ring = document.getElementById("ring") as unknown as SVGCircleElement;
const btnBreak = document.getElementById("btnBreak") as HTMLButtonElement;

const RING_LEN = 2 * Math.PI * 64;
let S = STR.zh;

function showRemindCard() {
  cardBreak.hidden = true;
  cardRemind.hidden = false;
}

function showBreakCard() {
  cardRemind.hidden = true;
  cardBreak.hidden = false;
}

function fmtTime(secs: number): string {
  return `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;
}

function chime() {
  try {
    const ctx = new AudioContext();
    const now = ctx.currentTime;
    for (const [freq, at] of [[880, 0], [1174.66, 0.18]] as const) {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = "sine";
      osc.frequency.value = freq;
      gain.gain.setValueAtTime(0.0001, now + at);
      gain.gain.exponentialRampToValueAtTime(0.18, now + at + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + at + 0.4);
      osc.connect(gain).connect(ctx.destination);
      osc.start(now + at);
      osc.stop(now + at + 0.45);
    }
  } catch {
    // 音频不可用时静默
  }
}

function renderState(state: TimerState) {
  const round = state.round ? `${fmt(S.roundNumber, { n: state.round.number })} · ` : "";
  if (state.mode === "remind") {
    document.getElementById("satText")!.textContent = round + fmt(S.satFor, { n: state.work_seconds / 60 });
    tipText.textContent = state.tip ?? "";
    btnBreak.textContent = fmt(S.startBreak, { n: state.break_seconds / 60 });
    showRemindCard();
  } else if (state.mode === "break") {
    breakTimeEl.textContent = fmtTime(state.remaining);
    ringCap.textContent = round + (state.paused ? S.paused : fmt(S.untilBack, { t: fmtTime(state.remaining) }));
    stretchText.textContent = state.tip ?? "";
    const progress = state.break_seconds > 0 ? state.remaining / state.break_seconds : 0;
    ring.style.strokeDashoffset = String(RING_LEN * (1 - progress));
    showBreakCard();
  }
}

// 内容、进度及首次打开均读取完整后端状态；提醒事件只负责一次性提示音。
listen<ReminderPayload>("reminder", ({ payload }) => { if (payload.sound) chime(); });
listen<TimerState>("tick", ({ payload }) => renderState(payload));

btnBreak.addEventListener("click", () => invoke("start_break"));
document.getElementById("btnPostpone")!.addEventListener("click", () => invoke("postpone"));
document.getElementById("btnEnd")!.addEventListener("click", () => invoke("end_break"));
window.addEventListener("contextmenu", (e) => e.preventDefault());

S = STR[await initLang()];
renderState(await invoke<TimerState>("timer_info"));
listen("settings_changed", async () => {
  S = STR[await initLang()];
  renderState(await invoke<TimerState>("timer_info"));
});
