import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { STR, initLang, fmt } from "./i18n";

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
let breakTotal = 0;
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

listen<ReminderPayload>("reminder", ({ payload }) => {
  document.getElementById("satText")!.textContent = fmt(S.satFor, {
    n: payload.satMinutes,
  });
  tipText.textContent = payload.tip;
  btnBreak.textContent = fmt(S.startBreak, {
    n: Math.round(payload.breakSeconds / 60),
  });
  showRemindCard();
  if (payload.sound) chime();
});

listen<{ seconds: number }>("break_started", ({ payload }) => {
  breakTotal = payload.seconds;
  breakTimeEl.textContent = fmtTime(breakTotal);
  ring.style.strokeDashoffset = "0";
  stretchText.textContent = document.getElementById("tipText")!.textContent;
  showBreakCard();
});

listen<{ mode: string; remaining: number }>("tick", ({ payload }) => {
  if (payload.mode !== "break") return;
  breakTimeEl.textContent = fmtTime(payload.remaining);
  ringCap.textContent = fmt(S.untilBack, { t: fmtTime(payload.remaining) });
  const progress = breakTotal > 0 ? payload.remaining / breakTotal : 0;
  ring.style.strokeDashoffset = String(RING_LEN * (1 - progress));
});

listen("work_started", showRemindCard);

btnBreak.addEventListener("click", () => invoke("start_break"));
document.getElementById("btnPostpone")!.addEventListener("click", () => invoke("postpone"));
document.getElementById("btnEnd")!.addEventListener("click", () => invoke("end_break"));
window.addEventListener("contextmenu", (e) => e.preventDefault());

(async () => {
  S = STR[await initLang()];
})();
listen("settings_changed", async () => {
  S = STR[await initLang()];
});
