import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { STR, applyStatic, currentLang, fmt, type Lang } from "./i18n";

interface PeriodDraft {
  id: string;
  start: string;
  end: string;
  work_minutes: number;
  break_minutes: number;
  overrides?: RoundDraft[];
}

interface RoundDraft {
  round: number;
  work_minutes?: number;
  break_minutes?: number;
  content?: string;
}

interface ValidationError {
  schedule_id: string | null;
  period_id: string | null;
  field: string;
  code: string;
}

interface ScheduleDraft {
  id: string;
  name: string;
  enabled: boolean;
  repeat: { kind: string; date?: string; [key: string]: unknown };
  periods: PeriodDraft[];
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
const expandedAdjustments = new Set<string>();

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
  document.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("#scheduleSettings input, #scheduleSettings textarea").forEach((input) => { input.disabled = !scheduled; });
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

function renderRoundAdjustments(period: PeriodDraft, periodCard: HTMLElement) {
  periodCard.querySelector(".round-adjustments")?.remove();
  const overrides = period.overrides ??= [];
  const details = document.createElement("details");
  details.className = "round-adjustments";
  details.open = expandedAdjustments.has(period.id);
  details.innerHTML = `<summary><span data-i18n="roundAdjustments"></span><span class="adjustment-count"></span></summary><p class="period-note" data-i18n="roundInheritance"></p><div class="round-list"></div><button type="button" class="secondary-button add-adjustment" data-i18n="addAdjustment"></button>`;
  details.querySelector(".adjustment-count")!.textContent = `(${overrides.length})`;
  details.addEventListener("toggle", () => {
    if (details.open) expandedAdjustments.add(period.id);
    else expandedAdjustments.delete(period.id);
  });
  const list = details.querySelector<HTMLElement>(".round-list")!;
  overrides.forEach((adjustment, index) => {
    const row = document.createElement("div");
    row.className = "round-adjustment";
    row.innerHTML = `<div class="round-adjustment-head"><label><span data-i18n="adjustmentRound"></span><input type="number" min="1" max="4294967295" step="1" required data-field="overrides.${index}.round" /></label><button type="button" class="text-button remove-adjustment" data-i18n="removeAdjustment"></button></div><div class="period-grid"><label><span data-i18n="workLength"></span><input type="number" min="1" max="240" step="1" data-inherit-field="work_minutes" data-field="overrides.${index}.work_minutes" /></label><label><span data-i18n="breakLen"></span><input type="number" min="1" max="60" step="1" data-inherit-field="break_minutes" data-field="overrides.${index}.break_minutes" /></label></div><label class="round-content"><span data-i18n="breakContent"></span><textarea rows="3" data-field="overrides.${index}.content"></textarea></label><p class="period-note" data-i18n="breakContentHint"></p>`;
    const round = row.querySelector<HTMLInputElement>(`[data-field="overrides.${index}.round"]`)!;
    round.value = String(adjustment.round);
    round.addEventListener("input", () => { adjustment.round = Number(round.value); round.removeAttribute("aria-invalid"); });
    for (const field of ["work_minutes", "break_minutes"] as const) {
      const input = row.querySelector<HTMLInputElement>(`[data-inherit-field="${field}"]`)!;
      input.value = adjustment[field] === undefined ? "" : String(adjustment[field]);
      input.placeholder = fmt(S.inheritDefault, { n: period[field] });
      input.addEventListener("input", () => {
        if (input.value === "") delete adjustment[field];
        else adjustment[field] = Number(input.value);
        input.removeAttribute("aria-invalid");
      });
    }
    const content = row.querySelector<HTMLTextAreaElement>("textarea")!;
    content.value = adjustment.content ?? "";
    content.addEventListener("input", () => {
      if (content.value === "") delete adjustment.content;
      else adjustment.content = content.value;
    });
    row.querySelector<HTMLButtonElement>(".remove-adjustment")!.addEventListener("click", () => {
      overrides.splice(index, 1);
      if (details.open) expandedAdjustments.add(period.id);
      renderRoundAdjustments(period, periodCard);
      applyStatic(activeLang());
      renderMode();
    });
    list.appendChild(row);
  });
  details.querySelector<HTMLButtonElement>(".add-adjustment")!.addEventListener("click", () => {
    let round = 1;
    while (overrides.some((adjustment) => adjustment.round === round)) round += 1;
    overrides.push({ round });
    expandedAdjustments.add(period.id);
    renderRoundAdjustments(period, periodCard);
    applyStatic(activeLang());
    renderMode();
    periodCard.querySelector<HTMLInputElement>(`.round-adjustment:last-child input`)?.focus();
  });
  periodCard.appendChild(details);
}

function renderSchedules() {
  scheduleList.replaceChildren();
  document.getElementById("emptySchedules")!.hidden = schedules.length > 0;
  for (const schedule of schedules) {
    const card = document.createElement("article");
    card.className = "schedule-card";
    card.dataset.scheduleId = schedule.id;
    card.innerHTML = `<div class="schedule-card-header"><label><span data-i18n="scheduleName"></span><input type="text" maxlength="100" data-field="name" /></label><button type="button" class="switch" role="switch"></button></div><label class="date-label"><span data-i18n="onceDate"></span><input type="date" required data-field="date" /></label><div class="period-list"></div><div class="empty-periods"><h2 data-i18n="emptyPeriods"></h2><p data-i18n="emptyPeriodsDescription"></p></div><div class="schedule-actions"><button type="button" class="secondary-button add-period" data-i18n="addPeriod"></button><button type="button" class="secondary-button remove-schedule" data-i18n="removeSchedule"></button></div>`;
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
    card.querySelector<HTMLButtonElement>(".remove-schedule")!.addEventListener("click", () => {
      schedules = schedules.filter((item) => item.id !== schedule.id);
      renderSchedules();
      applyStatic(activeLang());
    });
    card.querySelector<HTMLElement>(".empty-periods")!.hidden = schedule.periods.length > 0;
    const addPeriod = card.querySelector<HTMLButtonElement>(".add-period")!;
    addPeriod.hidden = schedule.periods.length > 0;
    addPeriod.addEventListener("click", () => {
      schedule.periods.push({ id: crypto.randomUUID(), start: "09:00", end: "12:00", work_minutes: Number(workMin.value) || 45, break_minutes: Number(breakMin.value) || 5, overrides: [] });
      renderSchedules();
    });
    const periods = card.querySelector<HTMLElement>(".period-list")!;
    for (const period of schedule.periods) {
      const periodCard = document.createElement("section");
      periodCard.className = "period-card";
      periodCard.dataset.periodId = period.id;
      periodCard.innerHTML = `<div class="period-head"><strong data-i18n="executionPeriod"></strong><button type="button" class="text-button" data-i18n="removePeriod"></button></div><div class="period-grid"><label><span data-i18n="periodStart"></span><input type="time" required data-field="start" /></label><label><span data-i18n="periodEnd"></span><input type="text" inputmode="numeric" pattern="([01][0-9]|2[0-3]):[0-5][0-9]|24:00" placeholder="HH:MM" required data-field="end" /></label><label><span data-i18n="defaultWork"></span><div class="period-duration"><input type="number" min="1" max="240" step="1" required data-field="work_minutes" /><span data-i18n="minutesUnit"></span></div></label><label><span data-i18n="defaultBreak"></span><div class="period-duration"><input type="number" min="1" max="60" step="1" required data-field="break_minutes" /><span data-i18n="minutesUnit"></span></div></label></div><p class="period-note" data-i18n="periodEndHint"></p>`;
      for (const field of ["start", "end", "work_minutes", "break_minutes"] as const) {
        const input = periodCard.querySelector<HTMLInputElement>(`[data-field="${field}"]`)!;
        input.value = String(period[field]);
        input.addEventListener("input", () => {
          input.removeAttribute("aria-invalid");
          if (field === "start" || field === "end") period[field] = input.value;
          else {
            period[field] = Number(input.value);
            periodCard.querySelectorAll<HTMLInputElement>(`[data-inherit-field="${field}"]`).forEach((inherited) => {
              inherited.placeholder = fmt(S.inheritDefault, { n: period[field] });
            });
          }
        });
      }
      periodCard.querySelector<HTMLButtonElement>(".text-button")!.addEventListener("click", () => {
        schedule.periods = schedule.periods.filter((item) => item.id !== period.id);
        renderSchedules();
      });
      renderRoundAdjustments(period, periodCard);
      periods.appendChild(periodCard);
    }
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
  expandedAdjustments.clear();
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

function describeSaveError(error: unknown): string {
  const failure = error as { kind?: string; errors?: ValidationError[]; message?: string };
  if (failure.kind !== "validation" || !failure.errors) return `${S.saveFailed} ${failure.message ?? String(error)}`;
  let first: HTMLInputElement | undefined;
  const messages = failure.errors.map((issue) => {
    const schedule = schedules.find((item) => item.id === issue.schedule_id);
    const period = schedule?.periods.find((item) => item.id === issue.period_id);
    const card = Array.from(scheduleList.querySelectorAll<HTMLElement>(".schedule-card")).find((item) => item.dataset.scheduleId === issue.schedule_id);
    const scope = issue.period_id ? Array.from(card?.querySelectorAll<HTMLElement>(".period-card") ?? []).find((item) => item.dataset.periodId === issue.period_id) : card;
    const input = scope?.querySelector<HTMLInputElement>(`[data-field="${issue.field}"]`);
    input?.setAttribute("aria-invalid", "true");
    if (input?.closest("details")) input.closest("details")!.open = true;
    first ??= input ?? undefined;
    const location = [schedule?.name, period ? `${period.start}–${period.end}` : ""].filter(Boolean).join(" · ");
    return `${location ? `${location}: ` : ""}${S[`validation_${issue.code}`] ?? S.fixSchedule}`;
  });
  first?.focus();
  return messages.join("\n");
}

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
    saveStatus.textContent = describeSaveError(error);
  } finally {
    btnSave.disabled = false;
  }
});

form.addEventListener("invalid", (event) => {
  const details = (event.target as HTMLElement).closest("details");
  if (details) details.open = true;
}, true);

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
