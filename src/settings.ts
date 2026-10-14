import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { STR, applyStatic, currentLang, fmt, type Lang } from "./i18n";

interface ScheduleDraft {
  id: string;
  name: string;
  enabled: boolean;
  repeat: { kind: string; date?: string; [key: string]: unknown };
  periods: unknown[];
  [key: string]: unknown;
}

interface Settings {
  work_minutes: number;
  break_minutes: number;
  use_schedule: boolean;
  schedules: ScheduleDraft[];
  sound: boolean;
  show_widget: boolean;
  autostart: boolean;
  idle_reset: boolean;
  language: string;
  widget_size: number;
  tips: string[];
}

const form = document.getElementById("settingsForm") as HTMLFormElement;
const workMin = document.getElementById("workMin") as HTMLInputElement;
const breakMin = document.getElementById("breakMin") as HTMLInputElement;
const widgetSize = document.getElementById("widgetSize") as HTMLInputElement;
const tipsList = document.getElementById("tipsList") as HTMLTextAreaElement;
const language = document.getElementById("language") as HTMLSelectElement;
const btnSave = document.getElementById("btnSave") as HTMLButtonElement;
const saveStatus = document.getElementById("saveStatus")!;
const scheduleList = document.getElementById("scheduleList")!;
const switches = {
  use_schedule: document.getElementById("swSchedule")!,
  sound: document.getElementById("swSound")!,
  show_widget: document.getElementById("swWidget")!,
  autostart: document.getElementById("swAuto")!,
  idle_reset: document.getElementById("swIdleReset")!,
};
let stored: Settings;
let schedules: ScheduleDraft[] = [];
let S = STR.zh;

function isOn(key: keyof typeof switches) {
  return switches[key].getAttribute("aria-checked") === "true";
}

function setSwitch(key: keyof typeof switches, on: boolean) {
  switches[key].setAttribute("aria-checked", String(on));
}

function renderMode() {
  const scheduled = isOn("use_schedule");
  document.getElementById("simpleSettings")!.hidden = scheduled;
  document.getElementById("scheduleSettings")!.hidden = !scheduled;
  // 隐藏的简单时长仍保留草稿，不参与当前可见表单的原生校验。
  workMin.disabled = scheduled;
  breakMin.disabled = scheduled;
  document.querySelectorAll<HTMLInputElement>("#scheduleSettings input").forEach((input) => { input.disabled = !scheduled; });
  document.getElementById("pageTitle")!.textContent = scheduled ? S.scheduleTitle : S.simpleTitle;
  document.getElementById("pageDescription")!.textContent = scheduled ? S.scheduleDescription : S.simpleDescription;
  document.getElementById("modeDescription")!.textContent = scheduled
    ? fmt(S.scheduleModeDescription, { work: workMin.value, rest: breakMin.value })
    : S.scheduleOptionalDescription;
}

function renderDurations() {
  document.querySelectorAll<HTMLElement>("[data-duration]").forEach((group) => {
    const input = document.getElementById(group.dataset.duration!) as HTMLInputElement;
    group.querySelectorAll<HTMLElement>("[data-minutes]").forEach((button) => {
      const selected = button.dataset.minutes === input.value;
      button.classList.toggle("selected", selected);
      button.setAttribute("aria-pressed", String(selected));
    });
  });
  document.getElementById("simpleSummary")!.textContent = fmt(S.simpleSummary, { work: workMin.value, rest: breakMin.value });
}

function localDate() {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}

function renderSchedules() {
  scheduleList.replaceChildren();
  document.getElementById("emptySchedules")!.hidden = schedules.length > 0;
  for (const schedule of schedules) {
    const card = document.createElement("article");
    card.className = "schedule-card";
    card.innerHTML = `<div class="schedule-card-header"><label><span data-i18n="scheduleName"></span><input type="text" maxlength="100" /></label><button type="button" class="switch" role="switch"></button></div><label class="date-label"><span data-i18n="onceDate"></span><input type="date" required /></label><div class="empty-periods"><h2 data-i18n="emptyPeriods"></h2><p data-i18n="emptyPeriodsDescription"></p></div><button type="button" class="secondary-button" data-i18n="removeSchedule"></button>`;
    const name = card.querySelector<HTMLInputElement>("input[type=text]")!;
    name.value = schedule.name;
    name.addEventListener("input", () => { schedule.name = name.value; });
    const enabled = card.querySelector<HTMLButtonElement>(".switch")!;
    enabled.setAttribute("aria-checked", String(schedule.enabled));
    enabled.setAttribute("aria-label", S.enableSchedule);
    enabled.addEventListener("click", () => {
      schedule.enabled = !schedule.enabled;
      enabled.setAttribute("aria-checked", String(schedule.enabled));
    });
    const date = card.querySelector<HTMLInputElement>("input[type=date]")!;
    date.value = schedule.repeat?.date ?? "";
    date.addEventListener("input", () => {
      schedule.repeat = { ...schedule.repeat, kind: "once", date: date.value };
    });
    card.querySelector<HTMLButtonElement>(".secondary-button")!.addEventListener("click", () => {
      schedules = schedules.filter((item) => item.id !== schedule.id);
      renderSchedules();
      applyStatic(activeLang());
    });
    scheduleList.appendChild(card);
  }
  applyStatic(activeLang());
  renderMode();
}

function activeLang(): Lang { return S === STR.en ? "en" : "zh"; }

async function translate() {
  const lang = await currentLang();
  S = STR[lang];
  applyStatic(lang);
  renderMode();
  renderDurations();
  document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
  document.querySelectorAll<HTMLElement>("[data-i18n-label]").forEach((el) => {
    el.setAttribute("aria-label", S[el.dataset.i18nLabel!]);
  });
  scheduleList.querySelectorAll<HTMLElement>(".switch").forEach((el) => {
    el.setAttribute("aria-label", S.enableSchedule);
  });
}

async function load() {
  stored = await invoke<Settings>("get_settings");
  schedules = structuredClone(stored.schedules ?? []);
  workMin.value = String(stored.work_minutes);
  breakMin.value = String(stored.break_minutes);
  widgetSize.value = String(stored.widget_size);
  tipsList.value = stored.tips.join("\n");
  language.value = ["zh", "en"].includes(stored.language) ? stored.language : "system";
  for (const key of Object.keys(switches) as (keyof typeof switches)[]) setSwitch(key, Boolean(stored[key]));
  saveStatus.textContent = "";
  await translate();
  renderSchedules();
}

function current(): Settings {
  const duration = (input: HTMLInputElement, saved: number, max: number) => {
    const value = Number(input.value);
    return Number.isInteger(value) && value >= 1 && value <= max ? value : saved;
  };
  return {
    ...stored,
    work_minutes: duration(workMin, stored.work_minutes, 240),
    break_minutes: duration(breakMin, stored.break_minutes, 60),
    use_schedule: isOn("use_schedule"),
    schedules,
    widget_size: Math.min(140, Math.max(40, Math.trunc(Number(widgetSize.value) || 64))),
    sound: isOn("sound"), show_widget: isOn("show_widget"),
    autostart: isOn("autostart"), idle_reset: isOn("idle_reset"),
    language: language.value,
    tips: tipsList.value.split("\n").map((tip) => tip.trim()).filter(Boolean),
  };
}

for (const key of Object.keys(switches) as (keyof typeof switches)[]) {
  switches[key].addEventListener("click", () => {
    setSwitch(key, !isOn(key));
    if (key === "use_schedule") renderMode();
  });
}
document.querySelectorAll<HTMLElement>("[data-duration]").forEach((group) => {
  const input = document.getElementById(group.dataset.duration!) as HTMLInputElement;
  group.querySelectorAll<HTMLElement>("[data-minutes]").forEach((button) => {
    button.addEventListener("click", () => { input.value = button.dataset.minutes!; renderDurations(); });
  });
  input.addEventListener("input", renderDurations);
});
document.getElementById("addSchedule")!.addEventListener("click", () => {
  schedules.push({ id: crypto.randomUUID(), name: S.newSchedule, enabled: true, repeat: { kind: "once", date: localDate() }, periods: [] });
  renderSchedules();
  scheduleList.lastElementChild?.querySelector<HTMLInputElement>("input[type=text]")?.focus();
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  btnSave.disabled = true;
  saveStatus.classList.remove("error");
  try {
    await invoke("save_settings", { settings: current() });
    await load();
    saveStatus.textContent = S.saved;
  } catch (error) {
    saveStatus.classList.add("error");
    saveStatus.textContent = `${S.saveFailed} ${String(error)}`;
  } finally {
    btnSave.disabled = false;
  }
});

document.getElementById("btnClose")!.addEventListener("click", () => invoke("hide_settings"));
window.addEventListener("keydown", (event) => { if (event.key === "Escape") invoke("hide_settings"); });
window.addEventListener("contextmenu", (event) => event.preventDefault());
const win = getCurrentWindow();
document.querySelector(".shead")!.addEventListener("mousedown", (event) => {
  const mouse = event as MouseEvent;
  if (mouse.button !== 0 || (mouse.target as HTMLElement).closest("button")) return;
  mouse.preventDefault();
  win.startDragging();
});

const platform = await invoke<string>("get_platform");
document.querySelectorAll<HTMLElement>("[data-widget-setting]").forEach((row) => { row.hidden = platform === "macos"; });
await load();
listen("settings_open", load);
