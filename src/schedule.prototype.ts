// 临时原型：在现有 /settings.html 上通过 ?prototype=schedule 进入。
// 默认保留完整简单计时，开启日程后使用已确认的时段卡片。
// 状态只存内存；这里只演示编辑、推迟休息和时段衔接，不是生产计时引擎。
import "./schedule.prototype.css";
import { icon } from "./prototype-icons";

type Adjustment = { round: number; work?: number; rest?: number; content?: string };
type Period = { id: string; name: string; start: string; end: string; work: number; rest: number; adjustments: Adjustment[] };
type Schedule = { id: string; name: string; enabled: boolean; repeat: string; days: number[]; date: string; periods: Period[] };
type Theme = "system" | "light" | "dark";
const requestedTheme = new URLSearchParams(location.search).get("theme");
const systemTheme = matchMedia("(prefers-color-scheme: dark)");

type Phase = "work" | "remind" | "break" | "idle";
const sample: Schedule = {
  id: "weekday", name: "工作日", enabled: true, repeat: "weekly", days: [1, 2, 3, 4, 5], date: "2026-10-14",
  periods: [
    { id: "morning", name: "上午", start: "09:00", end: "12:00", work: 55, rest: 5, adjustments: [{ round: 1, work: 50, rest: 10, content: "爬楼" }, { round: 2, content: "深蹲" }] },
    { id: "afternoon", name: "下午", start: "13:30", end: "18:00", work: 60, rest: 5, adjustments: [] },
  ],
};
const state = {
  useSchedule: new URLSearchParams(location.search).get("mode") === "schedule", schedules: [structuredClone(sample)], selected: "weekday",
  theme: (requestedTheme === "light" || requestedTheme === "dark" ? requestedTheme : "system") as Theme, view: "edit",
  closed: false, error: "", saved: false,
  openPeriods: new Set<string>(), sound: true, idleReset: true,
  simple: { work: 45, rest: 5 }, autostart: false, language: "system", showWidget: true, widgetSize: 64,
  platform: navigator.platform.toLowerCase().includes("mac") ? "macos" : "windows",
  tips: "走动一下，顺便接杯水喝\n站起来，伸展一下肩颈",
};
let applied: Schedule[] = structuredClone(state.schedules);
let appliedSimple = structuredClone(state.simple);
let appliedTips = state.tips;
let toastTimer: ReturnType<typeof setTimeout>;
const sim = {
  minute: 9 * 60, phase: "work" as Phase, remaining: state.useSchedule ? 50 : 45, round: 1,
  period: "morning", schedule: "weekday", paused: false, postponed: false,
  useSchedule: state.useSchedule,
  active: state.useSchedule ? { work: 50, rest: 10, content: "爬楼" } : { work: 45, rest: 5, content: state.tips.split("\n")[0] },
};
const days = ["一", "二", "三", "四", "五", "六", "日"];
const escape = (value: unknown) => String(value ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
const minutes = (time: string) => time.split(":").reduce((h, m) => h * 60 + Number(m), 0);
const time = (value: number) => `${String(Math.floor(value / 60) % 24).padStart(2, "0")}:${String(Math.floor(value % 60)).padStart(2, "0")}`;
const plan = () => state.schedules.find((s) => s.id === state.selected)!;
const runtimePlan = () => applied.find((s) => s.id === sim.schedule)!;
const resolveRound = (p: Period, round: number) => {
  const adjustment = p.adjustments.find((a) => a.round === round);
  return { work: adjustment?.work ?? p.work, rest: adjustment?.rest ?? p.rest, content: adjustment?.content || chooseDemoTip() };
};
const button = (text: string, action: string, extra = "", style = "") => `<button type="button" class="pbutton ${style}" data-action="${action}" ${extra}>${text}</button>`;
const switchButton = (label: string, checked: boolean, action: string) => `<button type="button" class="pswitch" role="switch" aria-label="${label}" aria-checked="${checked}" data-action="${action}"></button>`;
const field = (label: string, type: string, value: unknown, attributes: string, unit = "") => `<label class="pfield">${label}<span class="unit-field"><input type="${type}" value="${escape(value)}" ${attributes}>${unit ? `<em>${unit}</em>` : ""}</span></label>`;

function SimpleSettings() {
  const duration = (key: "work" | "rest", label: string, presets: number[], max: number) => `<div class="simple-duration"><label for="simple-${key}">${label}</label>
    <div class="duration-field"><input id="simple-${key}" type="number" min="1" max="${max}" data-simple="${key}" value="${state.simple[key]}" aria-describedby="simple-summary"><div class="number-stepper">${button(icon("up"), "duration-step", `data-field="${key}" data-delta="1" aria-label="增加${label}"`, "stepper-button")}${button(icon("down"), "duration-step", `data-field="${key}" data-delta="-1" aria-label="减少${label}"`, "stepper-button")}</div><span>分钟</span></div>
    <div class="duration-presets">${presets.map(n => `<button type="button" class="${state.simple[key] === n ? "selected" : ""}" aria-label="${key === "work" ? "工作" : "休息"} ${n} 分钟" aria-pressed="${state.simple[key] === n}" data-action="preset" data-field="${key}" data-value="${n}">${n}</button>`).join("")}</div></div>`;
  return `<section class="simple-settings" aria-label="工作与休息时长"><div class="durations">${duration("work", "工作时长", [15,30,45,60], 240)}${duration("rest", "休息时长", [3,5,10], 60)}</div><p id="simple-summary" class="simple-summary">每工作 <b>${state.simple.work}</b> 分钟，提醒休息 <b>${state.simple.rest}</b> 分钟。</p></section>`;
}

function CommonSettings() {
  const toggle = (label: string, key: "sound" | "autostart" | "idleReset" | "showWidget", hint = "") => `<div class="preference-row"><div><strong>${label}</strong>${hint ? `<p>${hint}</p>` : ""}</div><button type="button" class="pswitch" role="switch" aria-label="${label}" aria-checked="${state[key]}" data-action="preference" data-key="${key}"></button></div>`;
  return `<section class="common-settings"><div class="common-heading"><h2>通用设置</h2>${state.useSchedule ? '<span>简单计时与日程共用</span>' : ""}</div>
    <div class="preference-row"><label for="preferenceLanguage">语言</label><div class="select-field"><select id="preferenceLanguage" data-general="language"><option value="system" ${state.language === "system" ? "selected" : ""}>跟随系统</option><option value="zh" ${state.language === "zh" ? "selected" : ""}>简体中文</option><option value="en" ${state.language === "en" ? "selected" : ""}>English</option></select>${icon("down")}</div></div>
    ${toggle("提示音", "sound")}${toggle("开机自启", "autostart")}${toggle("离开自动重置", "idleReset")}
    ${state.platform === "windows" ? `${toggle("显示悬浮计时条", "showWidget")}${state.showWidget ? `<div class="preference-row"><label for="preferenceWidgetSize">悬浮球大小</label><div class="preference-size">${field("", "number", state.widgetSize, 'id="preferenceWidgetSize" min="40" max="140" data-general="widgetSize"', "px")}</div></div>` : ""}` : ""}
    <div class="tips-editor"><label for="preferenceTips">提醒小字</label><span>每行一条，随机展示</span><textarea id="preferenceTips" data-general="tips" rows="2" spellcheck="false">${escape(state.tips)}</textarea>${state.useSchedule ? '<p>为某一轮填写休息内容后，该轮优先显示你填写的内容。</p>' : ""}</div>
  </section>`;
}

function chooseDemoTip() {
  const tips = appliedTips.split("\n").map((tip) => tip.trim()).filter(Boolean);
  const alternatives = tips.filter((tip) => tip !== sim.active.content);
  const candidates = alternatives.length ? alternatives : tips;
  return candidates.length ? candidates[Math.floor(Math.random() * candidates.length)] : "走动一下，顺便接杯水喝";
}

function startSimple(round: number) {
  sim.useSchedule = false; sim.round = round; sim.period = "";
  sim.phase = "work"; sim.postponed = false; sim.remaining = appliedSimple.work;
  sim.active = { ...appliedSimple, content: chooseDemoTip() };
}

function repeatLabel(schedule = plan()) {
  if (schedule.repeat === "once") return `${schedule.date} · 仅一次`;
  if (schedule.days.length === 7) return "每天";
  if (schedule.days.join(",") === "1,2,3,4,5") return "周一至周五";
  return schedule.days.length ? schedule.days.map((d) => `周${days[d - 1]}`).join("、") : "请选择星期";
}

function repeatEditor() {
  const s = plan();
  return `<div class="repeat-control"><div class="repeat-top"><strong>什么时候执行</strong><div class="segmented">
    <button data-action="repeat" data-value="weekly" class="${s.repeat === "weekly" ? "selected" : ""}">每周重复</button>
    <button data-action="repeat" data-value="once" class="${s.repeat === "once" ? "selected" : ""}">仅一次</button></div></div>
    ${s.repeat === "weekly" ? `<div class="days">${days.map((d, i) => `<button class="day ${s.days.includes(i + 1) ? "selected" : ""}" aria-label="星期${d}" aria-pressed="${s.days.includes(i + 1)}" data-action="day" data-day="${i + 1}">${d}</button>`).join("")}<button class="day-shortcut" data-action="weekdays">工作日</button><button class="day-shortcut" data-action="everyday">每天</button></div>` : field("执行日期", "date", s.date, 'data-plan="date"')}
  </div>`;
}

function nameEditor() {
  return `<div class="name-row"><input aria-label="日程名称" data-plan="name" value="${escape(plan().name)}" maxlength="30"><label class="inline-switch">已${plan().enabled ? "启用" : "关闭"}${switchButton("启用这份日程", plan().enabled, "enabled")}</label></div>`;
}

function periodFields(p: Period) {
  return `<div class="period-fields"><div class="field-grid">
    ${field("开始时间", "time", p.start, `data-period="${p.id}" data-field="start"`)}
    ${field("结束时间", "time", p.end, `data-period="${p.id}" data-field="end"`)}
    </div><div class="field-grid">
    ${field("每轮工作", "number", p.work, `min="1" max="240" data-period="${p.id}" data-field="work"`, "分钟")}
    ${field("每次休息", "number", p.rest, `min="1" max="60" data-period="${p.id}" data-field="rest"`, "分钟")}
    </div></div>`;
}

function adjustments(p: Period) {
  return `<div class="overrides">${p.adjustments.length ? p.adjustments.map((a, i) => `<div class="override"><div class="override-heading"><span>第 <input aria-label="调整的轮次" class="round-input" type="number" min="1" max="99" data-period="${p.id}" data-adjust="${i}" data-field="round" value="${a.round}"> 轮</span>${button("恢复默认", "remove-adjust", `data-period="${p.id}" data-index="${i}"`, "ghost small")}</div>
    <div class="field-grid">${field("工作", "number", a.work ?? p.work, `min="1" max="240" data-period="${p.id}" data-adjust="${i}" data-field="work"`, "分钟")}${field("休息", "number", a.rest ?? p.rest, `min="1" max="60" data-period="${p.id}" data-adjust="${i}" data-field="rest"`, "分钟")}</div>
    <label class="pfield"><textarea rows="2" aria-label="第 ${a.round} 轮休息内容" placeholder="休息时做什么？可以写一项或多项活动" data-period="${p.id}" data-adjust="${i}" data-field="content">${escape(a.content ?? "")}</textarea></label>
  </div>`).join("") : '<p class="muted" style="margin-bottom:12px">每轮都使用默认时长。需要时再单独调整。</p>'}
    ${button(`${icon("plus")} 调整另一轮`, "add-adjust", `data-period="${p.id}"`, "ghost small")}
    <p class="muted" style="margin-top:8px;font-size:10px">休息内容自由填写，整次休息共用一个倒计时。</p></div>`;
}

function periodCard(p: Period, index: number) {
  const opened = state.openPeriods.has(p.id);
  return `<section class="period-card"><div class="period-head"><span class="period-icon ${index ? "pm" : ""}">${icon(index ? "afternoon" : "sun")}</span><strong>${escape(p.name)}</strong>${button("移除", "remove-period", `data-period="${p.id}"`, "ghost small")}</div>
    ${periodFields(p)}<button class="round-toggle" aria-expanded="${opened}" data-action="toggle-adjust" data-period="${p.id}"><span>${p.adjustments.length ? `${p.adjustments.length} 轮有单独安排` : "需要调整某一轮？"}<b>${p.adjustments.map((a) => `第 ${a.round} 轮${a.content ? ` · ${escape(a.content).slice(0, 12)}` : ""}`).join(" / ")}</b></span><span>${opened ? "收起" : "展开"}${icon(opened ? "up" : "down")}</span></button>${opened ? adjustments(p) : ""}</section>`;
}

function agenda() {
  const s = plan();
  const items = s.periods.slice().sort((a, b) => minutes(a.start) - minutes(b.start));
  return `<div class="agenda">${items.length ? "" : '<div class="agenda-empty"><h3>还没有执行时段</h3><p>添加时段后，这里会显示当天概览。</p></div>'}${items.map((p, i) => {
    const first = resolveRound(p, 1), second = resolveRound(p, 2);
    const firstEnd = minutes(p.start) + first.work;
    const gap = i > 0 && minutes(p.start) > minutes(items[i - 1].end);
    return `${gap ? `<div class="agenda-item"><time>${items[i - 1].end} — ${p.start}</time><p>留给自己，不提醒</p></div>` : ""}<div class="agenda-item"><time>${p.start} — ${p.end}</time><p>${escape(p.name)} · 工作 ${p.work} / 休息 ${p.rest}</p><span class="mini-tag">${firstEnd < minutes(p.end) ? `${time(firstEnd)} 首次休息` : "时段结束后待机"}</span><p class="muted">第一轮 ${first.work}/${first.rest}${p.adjustments[0]?.content ? ` · ${escape(first.content)}` : ""}<br>第二轮 ${second.work}/${second.rest}${p.adjustments.find((a) => a.round === 2)?.content ? ` · ${escape(second.content)}` : ""}</p></div>`;
  }).join("")}</div>`;
}

function ScheduleEditor() {
  return `<div class="schedule-layout"><div>${nameEditor()}${repeatEditor()}${!plan().periods.length ? '<div class="empty-periods"><h3>还没有执行时段</h3><p>添加一个时段，设定它的工作与休息节奏。</p></div>' : ""}${plan().periods.map((p, i) => `${i && minutes(p.start) > minutes(plan().periods[i - 1].end) ? `<div class="gap-label">${plan().periods[i - 1].end}–${p.start} 不提醒</div>` : ""}${periodCard(p, i)}`).join("")}${button(`${icon("plus")} 添加执行时段`, "add-period", "", "add-period")}</div></div>`;
}

function runsOnDemoDay(s: Schedule) {
  return s.enabled && (s.repeat === "weekly" ? s.days.includes(3) : s.date === "2026-10-14");
}

function activePeriod() {
  const s = runtimePlan();
  if (!state.useSchedule || !s || !runsOnDemoDay(s)) return undefined;
  return s.periods.find((p) => sim.minute >= minutes(p.start) && sim.minute < minutes(p.end));
}

function startRound(p: Period, round: number) {
  sim.period = p.id; sim.round = round; sim.phase = "work"; sim.postponed = false;
  sim.active = resolveRound(p, round); sim.remaining = sim.active.work;
}

function reconcile() {
  if (sim.phase === "break") return;
  if (!state.useSchedule) {
    if (sim.useSchedule || sim.phase === "idle") startSimple(1);
    return;
  }
  if (!sim.useSchedule) { sim.useSchedule = true; sim.phase = "idle"; sim.period = ""; }
  const p = activePeriod();
  if (!p) { sim.phase = "idle"; sim.remaining = 0; sim.postponed = false; }
  else if (sim.phase === "idle" || p.id !== sim.period) startRound(p, 1);
}

function finishBreak() {
  const changed = sim.useSchedule !== state.useSchedule;
  if (!state.useSchedule) { startSimple(changed ? 1 : sim.round + 1); return; }
  sim.useSchedule = true;
  const p = activePeriod();
  if (p) startRound(p, !changed && p.id === sim.period ? sim.round + 1 : 1);
  else { sim.phase = "idle"; sim.remaining = 0; }
}

function advance(amount: number) {
  for (let i = 0; i < amount; i++) {
    sim.minute++;
    if (sim.phase === "break") {
      if (!sim.paused && --sim.remaining <= 0) finishBreak();
      continue;
    }
    if (!sim.useSchedule) {
      if (!sim.paused && sim.phase === "work" && --sim.remaining <= 0) { sim.phase = "remind"; sim.postponed = false; }
      continue;
    }
    const p = activePeriod();
    if (!p || sim.phase === "idle" || p.id !== sim.period) { reconcile(); continue; }
    if (!sim.paused && sim.phase === "work" && --sim.remaining <= 0) { sim.phase = "remind"; sim.postponed = false; }
  }
}

function resetSimulation() {
  const s = applied.find((item) => item.id === state.selected) ?? applied[0];
  sim.schedule = s.id; sim.minute = state.useSchedule ? minutes(s.periods[0]?.start ?? "09:00") : 9 * 60;
  sim.phase = "idle"; sim.paused = false; sim.round = 1; sim.period = "";
  reconcile();
}

function upcoming() {
  const p = activePeriod();
  if (sim.paused) return "已暂停，恢复后更新预计时间。";
  if (sim.phase === "idle") {
    const next = runtimePlan()?.periods.filter((item) => minutes(item.start) > sim.minute).sort((a, b) => minutes(a.start) - minutes(b.start))[0];
    return next && state.useSchedule && runsOnDemoDay(runtimePlan()) ? `下一时段 <b>${next.start} · ${escape(next.name)}</b>，从第一轮开始。` : "当前没有有效安排，保持待机。";
  }
  const end = sim.minute + (sim.phase === "remind" ? sim.active.rest : sim.remaining + (sim.phase === "work" ? sim.active.rest : 0));
  if (!sim.useSchedule) return `预计 <b>${time(end)}</b> 开始下一轮工作，继续使用 ${appliedSimple.work}/${appliedSimple.rest} 的节奏。${sim.phase === "remind" ? "<br>按现在开始休息估算。" : ""}`;
  if (!p || end >= minutes(p.end)) return `本次休息完成后，按当时有效时段继续；没有安排则待机。`;
  return `预计 <b>${time(end)}</b> 开始第 ${sim.round + 1} 轮工作。${sim.phase === "remind" ? "<br>按现在开始休息估算。" : ""}`;
}

function Simulation() {
  const p = runtimePlan()?.periods.find((item) => item.id === sim.period);
  const title = sim.paused ? "先暂停，准备好再继续" : sim.phase === "remind" ? "该起来活动一下了" : sim.phase === "break" ? "这段时间，留给自己" : sim.phase === "idle" ? "现在不提醒" : sim.postponed ? "再忙一会儿，也没关系" : "安心工作，待会儿提醒你";
  const caption = sim.paused ? "当前进度已保留，恢复后继续计时。" : sim.phase === "remind" ? `准备休息 ${sim.active.rest} 分钟，点击后才开始计时。` : sim.phase === "break" ? (sim.useSchedule ? "从实际开始时计满，不因预排钟点缩短。" : "休息结束后，继续下一轮工作。") : sim.phase === "idle" ? "执行时段之外，安静待机。" : `本轮工作 ${sim.active.work} 分钟，再休息 ${sim.active.rest} 分钟。`;
  const status = `${sim.paused ? "已暂停 · " : ""}${sim.phase === "idle" ? "待机" : sim.phase === "remind" ? "该休息了" : `${sim.phase === "break" ? "休息 " : ""}${String(sim.remaining).padStart(2, "0")}:00`}`;
  const desktopPreview = `<div class="desktop-preview"><span>${state.platform === "macos" ? "菜单栏" : "桌面计时"}</span>${state.platform === "macos" ? `<div class="menubar-preview"><span>${icon("clock")}</span><b>${status}</b><span>${time(sim.minute)}</span></div>` : state.showWidget ? `<div class="floating-preview" style="width:${state.widgetSize}px;height:${state.widgetSize}px;font-size:${Math.max(12, state.widgetSize * .25)}px">${sim.phase === "idle" ? "待机" : sim.phase === "remind" ? "休息" : `${String(sim.remaining).padStart(2, "0")}:00`}</div>` : '<p class="muted">悬浮计时条已关闭，可从系统托盘查看进度。</p>'}</div>`;
  const sidebar = state.useSchedule ? `<h3>试试这些时刻</h3><p class="muted">点一下，直接进入对应情景。</p><button class="scenario" data-action="scenario" data-scenario="delay"><strong>01 · 晚 5 分钟才开始休息</strong><span>保留本轮内容，休息仍计满。</span></button><button class="scenario" data-action="scenario" data-scenario="boundary"><strong>02 · 休息还没结束，就到午休</strong><span>先完成这次休息，再保持安静。</span></button><button class="scenario" data-action="scenario" data-scenario="afternoon"><strong>03 · 午休之后，重新开始</strong><span>下一时段从第一轮开始，不补上午。</span></button>` : `<h3>熟悉的提醒方式</h3><p class="muted">一组时长，一直循环使用。</p><div class="simple-flow"><span>工作 ${appliedSimple.work} 分钟</span><i>${icon("below")}</i><span>提醒：开始休息 / 再等等</span><i>${icon("below")}</i><span>休息 ${appliedSimple.rest} 分钟</span><i>${icon("refresh")} 下一轮</i></div><p class="muted">暂停、重置、稍后休息与提前结束，沿用现有操作。</p>`;
  return `<div class="simulation"><div><div class="clock-strip"><div><span class="eyebrow">模拟时间${sim.useSchedule ? " · 10 月 14 日 · 周三" : " · 简单计时"}</span><time>${time(sim.minute)}</time></div>${button("从头演示", "restart", "", "ghost small")}</div><section class="live-card ${sim.phase} ${sim.paused ? "paused" : ""}"><span class="status-chip">${sim.paused ? "已暂停" : sim.phase === "idle" ? "待机" : sim.useSchedule ? `${escape(p?.name ?? "当前时段")} · 第 ${sim.round} 轮${sim.phase === "break" ? " · 休息中" : ""}` : `简单计时 · ${sim.phase === "break" ? "休息中" : sim.phase === "remind" ? "休息提醒" : "工作中"}`}</span><h2>${title}</h2>
    ${sim.phase === "work" || sim.phase === "break" ? `<div class="live-time">${String(sim.remaining).padStart(2, "0")}:00</div>` : ""}<p class="muted">${caption}</p>${sim.phase === "remind" || sim.phase === "break" ? `<div class="live-content">${escape(sim.active.content)}</div>` : ""}
    <div class="live-actions">${sim.phase === "remind" ? `${button(`再等等 · ${Math.min(5, sim.active.work)} 分钟`, "postpone")}${button(`开始休息 ${sim.active.rest} 分钟`, "start-break", sim.paused ? "disabled" : "", "primary")}` : sim.phase === "break" ? `${button("提前结束休息", "end-break")}${button("推进到休息结束", "advance", `data-minutes="${sim.remaining}" ${sim.paused ? "disabled" : ""}`, "primary")}` : sim.phase === "work" ? button(`推进到提醒 ${icon("right")}`, "advance", `data-minutes="${sim.remaining}" ${sim.paused ? "disabled" : ""}`, "primary") : button(`推进到下个时段 ${icon("right")}`, "next-boundary", "", "primary")}</div></section>
    <div class="next-up">${upcoming()}</div><div class="sim-controls">${button("＋1 分钟", "advance", 'data-minutes="1"', "small")}${button("＋5 分钟", "advance", 'data-minutes="5"', "small")}${button(sim.paused ? "恢复" : "暂停", "pause", "", "ghost small")}${button("重置本轮", "reset", "", "ghost small")}</div></div>
    <aside class="scenarios">${sidebar}${desktopPreview}<div class="sim-caption">“＋分钟”只推进演示时钟。<br>设置里的修改需要保存后才用于试运行。${state.useSchedule ? "<br>下午 18:00 是示例结束时间，可自行调整。" : ""}</div></aside></div>`;
}

function toast(message: string) {
  clearTimeout(toastTimer);
  document.querySelector(".prototype-toast")?.remove();
  const element = document.createElement("div"); element.className = "prototype-toast"; element.setAttribute("role", "status"); element.textContent = message;
  document.body.append(element); toastTimer = setTimeout(() => element.remove(), 3500);
}

function validate() {
  if (state.simple.work < 1 || state.simple.work > 240 || state.simple.rest < 1 || state.simple.rest > 60) return "工作设为 1–240 分钟，休息设为 1–60 分钟。";
  if (!state.useSchedule) return "";
  for (const s of state.schedules) {
    if (!s.name.trim()) return "给日程起个名字吧。";
    if (s.repeat === "weekly" && !s.days.length) return `“${s.name}”至少选择一个星期。`;
    if (s.repeat === "once" && !s.date) return "请选择执行日期。";
    for (const p of s.periods) {
      if (!p.start || !p.end || minutes(p.start) >= minutes(p.end)) return `${p.name}的结束时间应晚于开始时间。`;
      if (p.work < 1 || p.work > 240 || p.rest < 1 || p.rest > 60) return "工作设为 1–240 分钟，休息设为 1–60 分钟。";
      if (new Set(p.adjustments.map((a) => a.round)).size !== p.adjustments.length) return "同一个时段中，请为不同轮次分别设置。";
      if (p.adjustments.some((a) => a.round < 1 || (a.work !== undefined && (a.work < 1 || a.work > 240)) || (a.rest !== undefined && (a.rest < 1 || a.rest > 60)))) return "请检查单轮时长和轮次。";
    }
  }
  const matches = (a: Schedule, b: Schedule) => {
    if (a.repeat === "weekly" && b.repeat === "weekly") return a.days.some((d) => b.days.includes(d));
    if (a.repeat === "once" && b.repeat === "once") return a.date === b.date;
    const once = a.repeat === "once" ? a : b, weekly = a.repeat === "weekly" ? a : b;
    const weekday = new Date(`${once.date}T12:00:00`).getDay() || 7;
    return weekly.days.includes(weekday);
  };
  const entries = state.schedules.flatMap((s) => s.periods.map((p) => ({ s, p })));
  for (let i = 0; i < entries.length; i++) for (let j = i + 1; j < entries.length; j++) {
    const a = entries[i], b = entries[j];
    const same = a.s.id === b.s.id;
    if (!same && (!a.s.enabled || !b.s.enabled || !matches(a.s, b.s))) continue;
    if (minutes(a.p.start) < minutes(b.p.end) && minutes(b.p.start) < minutes(a.p.end)) return `“${a.s.name} · ${a.p.name}”与“${b.s.name} · ${b.p.name}”时间重叠；原安排保持不变。`;
  }
  return "";
}

function save() {
  const error = validate();
  state.error = error;
  if (error) { state.saved = false; toast("还差一步，请检查设置提示。"); return false; }
  if (state.useSchedule) applied = structuredClone(state.schedules);
  const simpleChanged = appliedSimple.work !== state.simple.work;
  appliedSimple = structuredClone(state.simple); appliedTips = state.tips;
  if (!state.useSchedule && !sim.useSchedule && sim.phase === "work" && simpleChanged) startSimple(sim.round);
  reconcile();
  state.saved = true; toast("设置已保存到本次演示。"); return true;
}

function render() {
  const root = document.querySelector<HTMLElement>(".scard")!;
  root.classList.toggle("simple-layout", state.view === "edit" && !state.useSchedule);
  root.classList.toggle("closed", state.closed);
  const modeControl = `<div class="mode-row ${state.useSchedule ? "" : "optional-schedule"}"><div><strong>使用日程</strong><p class="muted">${state.useSchedule ? `按时段执行，空档不提醒。关闭后回到简单计时 ${state.simple.work}/${state.simple.rest}。` : "按星期和时段安排不同节奏。"}</p></div>${switchButton("使用日程", state.useSchedule, "use-schedule")}</div>`;
  const scheduleEditor = `<div class="schedule-tabs" aria-label="日程列表">${state.schedules.map((s) => `<button class="schedule-tab ${s.id === state.selected ? "selected" : ""}" aria-pressed="${s.id === state.selected}" data-action="select-schedule" data-id="${s.id}">${escape(s.name)}${!s.enabled ? '<small>已关闭</small>' : ""}</button>`).join("")}${button(`${icon("plus")} 新建日程`, "add-schedule", "", "ghost small")}</div>${ScheduleEditor()}`;
  const error = state.error ? `<div class="form-error" role="alert" tabindex="-1">${icon("alert")}<div><strong>请检查设置</strong><p>${escape(state.error)}</p></div></div>` : "";
  const editor = `${error}${state.useSchedule ? `${modeControl}${scheduleEditor}` : `${SimpleSettings()}${modeControl}`}${CommonSettings()}<div class="bottom-actions"><p class="muted">${state.useSchedule ? "按电脑本地时间执行，休息可以推迟。" : "到点先提醒，由你决定何时休息。"}</p>${button(state.saved ? `${icon("check")} 已保存` : "保存设置", "save", "", "primary save-button")}</div>`;
  root.innerHTML = state.closed ? `<div class="closed-state">${icon("clock")}<h1>设置已收起</h1><p>随时回来调整自己的节奏。</p>${button("重新打开设置", "reopen", "", "primary")}</div>` : `<header class="shead"><span class="app-icon">${icon("clock")}</span><h3>Sit Break</h3><span class="header-slash">/</span><span class="header-label">${state.view === "run" ? "试运行" : "设置"}</span>${button(icon("close"), "close", 'aria-label="收起设置"', "close-button")}</header><main class="proto-body"><div class="page-heading"><div><h1>${state.view === "run" ? "按自己的节奏来" : state.useSchedule ? "日程安排" : "简单计时"}</h1><p>${state.view === "run" ? "到点先提醒，准备好了再休息。" : state.useSchedule ? "设好平常的节奏，只调整不一样的那几轮。" : "设定工作与休息时长，到点提醒你。"}</p></div>${state.view === "run" ? button(`${icon("left")} 返回设置`, "edit", "", "ghost") : state.useSchedule ? button(`${icon("eye")} 预览日程`, "preview", 'aria-haspopup="dialog" aria-controls="schedule-preview"', "schedule-preview-trigger") : ""}</div>${state.view === "run" ? Simulation() : editor}</main>`;
  document.querySelector(".prototype-heading")!.innerHTML = `<span>交互原型 · 节奏优先</span><div class="preview-actions"><label class="theme-preview">外观<select aria-label="预览外观" data-preview-theme><option value="system" ${state.theme === "system" ? "selected" : ""}>跟随系统</option><option value="light" ${state.theme === "light" ? "selected" : ""}>浅色</option><option value="dark" ${state.theme === "dark" ? "selected" : ""}>深色</option></select></label><div class="platform-preview" aria-label="预览系统"><button data-preview-platform="macos" aria-pressed="${state.platform === "macos"}" class="${state.platform === "macos" ? "selected" : ""}">macOS</button><button data-preview-platform="windows" aria-pressed="${state.platform === "windows"}" class="${state.platform === "windows" ? "selected" : ""}">Windows</button></div>${button(`${icon(state.view === "run" ? "left" : "play")} ${state.view === "run" ? "返回设置" : "试运行"}`, state.view === "run" ? "edit" : "run", "", "small")}</div>`;
}

function applyTheme() {
  document.body.dataset.theme = state.theme === "system" ? (systemTheme.matches ? "dark" : "light") : state.theme;
}

// 跟随系统实时更新，只改视觉变量，不重建草稿或运行进度。
systemTheme.addEventListener("change", () => { if (state.theme === "system") applyTheme(); });
document.body.classList.add("settings-prototype");
applyTheme();
document.title = "Sit Break · UI 设计";
document.querySelector(".toast")?.remove();
// 兼容旧原型链接；移除废弃的布局参数。
const previewUrl = new URL(location.href);
previewUrl.searchParams.delete("variant");
history.replaceState(null, "", previewUrl);
document.querySelector(".scard")!.insertAdjacentHTML("afterend", '<div class="prototype-heading"></div>');
const schedulePreview = document.createElement("dialog");
schedulePreview.id = "schedule-preview";
schedulePreview.className = "schedule-preview";
schedulePreview.setAttribute("aria-labelledby", "schedule-preview-title");
schedulePreview.setAttribute("aria-describedby", "schedule-preview-description");
document.body.append(schedulePreview);

function openSchedulePreview() {
  schedulePreview.innerHTML = `<header class="preview-head"><div><h2 id="schedule-preview-title">日程预览</h2><p id="schedule-preview-description">${escape(plan().name)} · ${escape(repeatLabel())} · 时间为预计</p></div>${button(icon("close"), "close-preview", 'aria-label="关闭日程预览" autofocus', "close-button")}</header><div class="preview-scroll">${agenda()}</div><footer class="preview-footer"><p>晚开始休息，后面的轮次跟着顺延。</p>${button(`${icon("play")} 看看实际怎么运行`, "preview-run", "", "primary")}</footer>`;
  schedulePreview.showModal();
}

// 原生浮窗隔离背景并处理 Esc；Tab 在浮窗操作之间循环。
schedulePreview.addEventListener("keydown", (event) => {
  if (event.key !== "Tab") return;
  const controls = schedulePreview.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
  const first = controls[0], last = controls[controls.length - 1];
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
});
schedulePreview.addEventListener("click", (event) => {
  if (event.target === schedulePreview) {
    const rect = schedulePreview.getBoundingClientRect();
    if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) schedulePreview.close();
    return;
  }
  const action = (event.target as HTMLElement).closest<HTMLElement>("[data-action]")?.dataset.action;
  if (action === "close-preview") schedulePreview.close();
  if (action === "preview-run") {
    schedulePreview.close();
    if (save()) { state.view = "run"; resetSimulation(); }
    render();
    window.scrollTo({ top: 0 });
    if (state.error) document.querySelector<HTMLElement>(".form-error")?.focus({ preventScroll: true });
    else {
      const heading = document.querySelector<HTMLElement>(".page-heading h1")!;
      heading.tabIndex = -1;
      heading.focus({ preventScroll: true });
    }
  }
});

document.querySelector(".prototype-heading")!.addEventListener("change", (event) => {
  const target = event.target as HTMLSelectElement;
  if (!target.matches("[data-preview-theme]")) return;
  state.theme = target.value as Theme;
  applyTheme();
  const url = new URL(location.href);
  if (state.theme === "system") url.searchParams.delete("theme"); else url.searchParams.set("theme", state.theme);
  history.replaceState(null, "", url);
});
document.querySelector(".prototype-heading")!.addEventListener("click", (event) => {
  const el = (event.target as HTMLElement).closest<HTMLElement>("[data-preview-platform], [data-action]");
  if (!el) return;
  if (el.dataset.previewPlatform) { state.platform = el.dataset.previewPlatform; render(); }
  else if (el.dataset.action === "run") { if (save()) { state.closed = false; state.view = "run"; resetSimulation(); } render(); window.scrollTo({ top: 0 }); }
  else if (el.dataset.action === "edit") { state.view = "edit"; render(); window.scrollTo({ top: 0 }); }
});

// 在输入时同步草稿，但不重建正在编辑的控件，保留焦点并避免吞掉下一次点击。
document.querySelector(".scard")!.addEventListener("input", (event) => {
  const target = event.target as HTMLInputElement;
  if (target.dataset.plan) (plan() as unknown as Record<string, unknown>)[target.dataset.plan] = target.value;
  if (target.dataset.simple) state.simple[target.dataset.simple as "work" | "rest"] = Math.floor(Number(target.value));
  if (target.dataset.general) (state as unknown as Record<string, unknown>)[target.dataset.general] = target.type === "number" ? Math.min(140, Math.max(40, Math.floor(Number(target.value)) || 64)) : target.value;
  if (target.dataset.period && target.dataset.field) {
    const p = plan().periods.find((item) => item.id === target.dataset.period)!;
    const object = target.dataset.adjust === undefined ? p : p.adjustments[Number(target.dataset.adjust)];
    (object as unknown as Record<string, unknown>)[target.dataset.field] = target.type === "number" ? Math.floor(Number(target.value)) : target.value;
  }
  state.saved = false;
  const summary = document.querySelector(".simple-summary");
  if (summary) summary.innerHTML = `每工作 <b>${state.simple.work}</b> 分钟，提醒休息 <b>${state.simple.rest}</b> 分钟。`;
  document.querySelectorAll<HTMLButtonElement>('.duration-presets button').forEach(button => {
    const selected = state.simple[button.dataset.field as "work" | "rest"] === Number(button.dataset.value);
    button.classList.toggle("selected", selected); button.setAttribute("aria-pressed", String(selected));
  });
  const saveButton = document.querySelector(".save-button"); if (saveButton) saveButton.textContent = "保存设置";
  if (target.dataset.plan === "name") { const tab = document.querySelector(`[data-id="${state.selected}"]`); if (tab) tab.textContent = target.value; }
});

document.querySelector(".scard")!.addEventListener("click", (event) => {
  const el = (event.target as HTMLElement).closest<HTMLElement>("[data-action]");
  if (!el) return;
  if (el.dataset.action === "preview") { openSchedulePreview(); return; }
  const p = plan().periods.find((item) => item.id === el.dataset.period);
  if (el.dataset.action !== "save") { state.error = ""; state.saved = false; }
  switch (el.dataset.action) {
    case "close": state.closed = true; break;
    case "reopen": state.closed = false; break;
    case "duration-step": { const key = el.dataset.field as "work" | "rest"; state.simple[key] = Math.max(1, Math.min(key === "work" ? 240 : 60, state.simple[key] + Number(el.dataset.delta))); break; }
    case "use-schedule": { state.useSchedule = !state.useSchedule; const url = new URL(location.href); if (state.useSchedule) url.searchParams.set("mode", "schedule"); else url.searchParams.delete("mode"); history.replaceState(null, "", url); reconcile(); break; }
    case "preference": { const key = el.dataset.key as "sound" | "autostart" | "idleReset" | "showWidget"; state[key] = !state[key]; break; }
    case "preset": state.simple[el.dataset.field as "work" | "rest"] = Number(el.dataset.value); break;
    case "enabled": plan().enabled = !plan().enabled; break;
    case "repeat": plan().repeat = el.dataset.value!; break;
    case "day": { const day = Number(el.dataset.day); plan().days = plan().days.includes(day) ? plan().days.filter((d) => d !== day) : [...plan().days, day].sort(); break; }
    case "weekdays": plan().days = [1, 2, 3, 4, 5]; break;
    case "everyday": plan().days = [1, 2, 3, 4, 5, 6, 7]; break;
    case "select-schedule": state.selected = el.dataset.id!; break;
    case "add-schedule": { const s = structuredClone(sample); s.id = `schedule-${Date.now()}`; s.name = "新日程"; s.enabled = false; s.days = [6, 7]; s.periods = s.periods.map((item) => ({ ...item, id: `${s.id}-${item.id}`, adjustments: [] })); state.schedules.push(s); state.selected = s.id; break; }
    case "add-period": { const id = `period-${Date.now()}`; const lastEnd = plan().periods[plan().periods.length - 1]?.end ?? "09:00"; plan().periods.push({ id, name: `时段 ${plan().periods.length + 1}`, start: lastEnd, end: time(Math.min(23 * 60 + 59, minutes(lastEnd) + 120)), work: 55, rest: 5, adjustments: [] }); break; }
    case "remove-period": plan().periods = plan().periods.filter((item) => item.id !== el.dataset.period); break;
    case "toggle-adjust": if (state.openPeriods.has(p!.id)) state.openPeriods.delete(p!.id); else state.openPeriods.add(p!.id); break;
    case "add-adjust": { let round = 1; while (p!.adjustments.some((a) => a.round === round)) round++; p!.adjustments.push({ round, content: "" }); break; }
    case "remove-adjust": p!.adjustments.splice(Number(el.dataset.index), 1); break;
    case "save": save(); break;
    case "run": if (save()) { state.view = "run"; resetSimulation(); } break;
    case "edit": state.view = "edit"; break;
    case "restart": resetSimulation(); break;
    case "advance": advance(Number(el.dataset.minutes)); break;
    case "start-break": sim.phase = "break"; sim.remaining = sim.active.rest; break;
    case "postpone": sim.phase = "work"; sim.remaining = Math.max(1, Math.min(5, sim.active.work)); sim.postponed = true; break;
    case "end-break": finishBreak(); break;
    case "pause": sim.paused = !sim.paused; break;
    case "reset": { const current = activePeriod(); if (current || !sim.useSchedule) { sim.phase = "work"; sim.remaining = sim.active.work; sim.postponed = false; } else reconcile(); break; }
    case "next-boundary": { const next = runtimePlan().periods.flatMap((item) => [minutes(item.start), minutes(item.end)]).filter((m) => m > sim.minute).sort((a, b) => a - b)[0]; if (next) advance(next - sim.minute); else toast("今天没有下一个执行时段，可以从头演示。"); break; }
    case "scenario": {
      resetSimulation(); const first = runtimePlan().periods[0];
      if (!first || !activePeriod()) { toast("示例按周三演示，请先启用一份周三有效的安排。"); break; }
      if (el.dataset.scenario === "delay") { advance(sim.remaining); if (sim.phase === "remind") { sim.phase = "work"; sim.remaining = Math.min(5, sim.active.work); sim.postponed = true; advance(sim.remaining); } }
      if (el.dataset.scenario === "boundary") { sim.minute = minutes(first.end) - 2; startRound(first, 3); sim.phase = "break"; sim.remaining = sim.active.rest; }
      if (el.dataset.scenario === "afternoon") { sim.minute = minutes(first.end); sim.phase = "idle"; reconcile(); }
      break;
    }
  }
  const focusAction = el.dataset.action;
  const focusKeys = Object.entries(el.dataset).map(([key, value]) => `[data-${key.replace(/[A-Z]/g, c => `-${c.toLowerCase()}`)}="${CSS.escape(value ?? "")}"]`).join("");
  render();
  document.querySelector<HTMLElement>(focusKeys)?.focus({ preventScroll: true });
  if (focusAction === "run" || focusAction === "edit" || focusAction === "reopen") window.scrollTo({ top: 0 });
  if (focusAction === "save" && state.error) document.querySelector(".form-error")?.scrollIntoView({ block: "center", behavior: "smooth" });
});

render();
