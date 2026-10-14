// 临时原型：在现有 /settings.html 上通过 ?prototype=schedule&variant=A/B/C
// 比较三种日程设置结构。问题：默认安排＋单轮调整，哪种方式最容易理解？
// 状态只存内存；这里只演示编辑、推迟休息和时段衔接，不是生产计时引擎。
import "./schedule.prototype.css";
import { mountPrototypeSwitcher, variants } from "./prototype-switcher";

type Adjustment = { round: number; work?: number; rest?: number; content?: string };
type Period = { id: string; name: string; start: string; end: string; work: number; rest: number; adjustments: Adjustment[] };
type Schedule = { id: string; name: string; enabled: boolean; repeat: string; days: number[]; date: string; periods: Period[] };
type Phase = "work" | "remind" | "break" | "idle";
const sample: Schedule = {
  id: "weekday", name: "工作日", enabled: true, repeat: "weekly", days: [1, 2, 3, 4, 5], date: "2026-10-14",
  periods: [
    { id: "morning", name: "上午", start: "09:00", end: "12:00", work: 55, rest: 5, adjustments: [{ round: 1, work: 50, rest: 10, content: "爬楼" }, { round: 2, content: "深蹲" }] },
    { id: "afternoon", name: "下午", start: "13:30", end: "18:00", work: 60, rest: 5, adjustments: [] },
  ],
};
const state = {
  useSchedule: true, schedules: [structuredClone(sample)], selected: "weekday",
  variant: new URLSearchParams(location.search).get("variant") ?? "A", view: "edit", step: 1,
  period: "morning", openPeriods: new Set<string>(), sound: true, idleReset: true,
};
let applied: Schedule[] = structuredClone(state.schedules);
let toastTimer: ReturnType<typeof setTimeout>;
const sim = {
  minute: 9 * 60, phase: "work" as Phase, remaining: 50, round: 1,
  period: "morning", schedule: "weekday", paused: false, postponed: false,
  active: { work: 50, rest: 10, content: "爬楼" },
};
const days = ["一", "二", "三", "四", "五", "六", "日"];
const escape = (value: unknown) => String(value ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
const minutes = (time: string) => time.split(":").reduce((h, m) => h * 60 + Number(m), 0);
const time = (value: number) => `${String(Math.floor(value / 60) % 24).padStart(2, "0")}:${String(Math.floor(value % 60)).padStart(2, "0")}`;
const plan = () => state.schedules.find((s) => s.id === state.selected)!;
const runtimePlan = () => applied.find((s) => s.id === sim.schedule)!;
const selectedPeriod = () => plan().periods.find((p) => p.id === state.period) ?? plan().periods[0];
const resolveRound = (p: Period, round: number) => {
  const adjustment = p.adjustments.find((a) => a.round === round);
  return { work: adjustment?.work ?? p.work, rest: adjustment?.rest ?? p.rest, content: adjustment?.content || "走动一下，顺便接杯水喝" };
};
const button = (text: string, action: string, extra = "", style = "") => `<button type="button" class="pbutton ${style}" data-action="${action}" ${extra}>${text}</button>`;
const switchButton = (label: string, checked: boolean, action: string) => `<button type="button" class="pswitch" role="switch" aria-label="${label}" aria-checked="${checked}" data-action="${action}"></button>`;
const field = (label: string, type: string, value: unknown, attributes: string, unit = "") => `<label class="pfield">${label}<span class="unit-field"><input type="${type}" value="${escape(value)}" ${attributes}>${unit ? `<em>${unit}</em>` : ""}</span></label>`;

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
    ${button("＋ 调整另一轮", "add-adjust", `data-period="${p.id}"`, "ghost small")}
    <p class="muted" style="margin-top:8px;font-size:10px">休息内容自由填写，整次休息共用一个倒计时。</p></div>`;
}

function periodCard(p: Period, index: number, showAdjustments = true) {
  const opened = state.openPeriods.has(p.id);
  return `<section class="period-card"><div class="period-head"><span class="period-icon ${index ? "pm" : ""}">${index ? "◒" : "☼"}</span><strong>${escape(p.name)}</strong>${button("移除", "remove-period", `data-period="${p.id}"`, "ghost small")}</div>
    ${periodFields(p)}${showAdjustments ? `<button class="round-toggle" data-action="toggle-adjust" data-period="${p.id}"><span>${p.adjustments.length ? `${p.adjustments.length} 轮有单独安排` : "需要调整某一轮？"}<b>${p.adjustments.map((a) => `第 ${a.round} 轮${a.content ? ` · ${escape(a.content).slice(0, 12)}` : ""}`).join(" / ")}</b></span><span>${opened ? "收起 −" : "展开 ＋"}</span></button>${opened ? adjustments(p) : ""}` : ""}</section>`;
}

function agenda() {
  const s = plan();
  const items = s.periods.slice().sort((a, b) => minutes(a.start) - minutes(b.start));
  return `<aside class="agenda"><h3>这一天，大致这样</h3><p class="agenda-note">${escape(repeatLabel())} · 时间为预计</p>${items.map((p, i) => {
    const first = resolveRound(p, 1), second = resolveRound(p, 2);
    const firstEnd = minutes(p.start) + first.work;
    const gap = i > 0 && minutes(p.start) > minutes(items[i - 1].end);
    return `${gap ? `<div class="agenda-item"><time>${items[i - 1].end} — ${p.start}</time><p>留给自己，不提醒</p></div>` : ""}<div class="agenda-item"><time>${p.start} — ${p.end}</time><p>${escape(p.name)} · 工作 ${p.work} / 休息 ${p.rest}</p><span class="mini-tag">${firstEnd < minutes(p.end) ? `${time(firstEnd)} 首次休息` : "时段结束后待机"}</span><p class="muted">第一轮 ${first.work}/${first.rest}${p.adjustments[0]?.content ? ` · ${escape(first.content)}` : ""}<br>第二轮 ${second.work}/${second.rest}${p.adjustments.find((a) => a.round === 2)?.content ? ` · ${escape(second.content)}` : ""}</p></div>`;
  }).join("")}<p class="agenda-note" style="margin-bottom:12px">晚开始休息，后面的轮次跟着顺延。</p>${button("▷ 看看实际怎么运行", "run", "", "primary")}</aside>`;
}

export function VariantA() {
  return `<div class="a-layout"><div>${nameEditor()}${repeatEditor()}${plan().periods.map((p, i) => `${i && minutes(p.start) > minutes(plan().periods[i - 1].end) ? `<div class="gap-label">${plan().periods[i - 1].end}–${p.start} 不提醒</div>` : ""}${periodCard(p, i)}`).join("")}${button("＋ 添加执行时段", "add-period", "", "add-period")}</div>${agenda()}</div>`;
}

export function VariantB() {
  const p = selectedPeriod();
  return `${nameEditor()}${repeatEditor()}<div class="timeline-layout"><div><div class="timeline-toolbar"><strong class="section-label">点击一个时段，调整它</strong>${button("＋ 时段", "add-period", "", "ghost small")}</div>${plan().periods.map((item, i) => `<div class="timeline-row"><time class="timeline-time">${item.start}</time><button class="timeline-block ${p?.id === item.id ? "selected" : ""}" data-action="select-period" data-period="${item.id}"><strong>${escape(item.name)}</strong><p>工作 ${item.work} 分钟 / 休息 ${item.rest} 分钟</p>${item.adjustments.length ? item.adjustments.map((a) => `<div class="timeline-round">第 ${a.round} 轮 · ${a.work ?? item.work}/${a.rest ?? item.rest}${a.content ? ` · ${escape(a.content)}` : ""}</div>`).join("") : '<div class="timeline-round">每轮使用默认安排</div>'}<p class="muted">到 ${item.end} 结束</p></button></div>${plan().periods[i + 1] && minutes(plan().periods[i + 1].start) > minutes(item.end) ? `<div class="timeline-gap">${item.end}–${plan().periods[i + 1].start}<br>不提醒，留给自己</div>` : ""}`).join("")}</div><aside class="inspector">${p ? `<span class="eyebrow">编辑执行时段</span><h3>${escape(p.name)} · ${p.start}–${p.end}</h3><p class="muted">设好默认节奏，再调整特殊轮次。</p>${periodFields(p)}<div class="repeat-top" style="margin-top:20px"><strong>单轮安排</strong>${button("移除此时段", "remove-period", `data-period="${p.id}"`, "ghost small")}</div>${adjustments(p)}` : '<p class="muted">添加一个时段开始安排。</p>'}</aside></div>`;
}

export function VariantC() {
  const labels = ["重复日期", "执行时段", "单轮安排", "看一遍"];
  const contents = [
    `<h2 class="step-title">哪些日子，照这个节奏？</h2><p class="step-description">像设置手机闹钟一样，选好重复的日子。</p>${nameEditor()}${repeatEditor()}`,
    `<h2 class="step-title">给一天留出几个时段</h2><p class="step-description">时段之间不提醒；每段只需要一组默认时长。</p>${plan().periods.map((p, i) => periodCard(p, i, false)).join("")}${button("＋ 添加执行时段", "add-period", "", "add-period")}`,
    `<h2 class="step-title">有哪一轮，需要不一样？</h2><p class="step-description">这一步可以跳过，其余轮次继续使用默认值。</p><div class="wizard-optional">例如第一轮爬楼 10 分钟，第二轮深蹲 5 分钟。</div>${plan().periods.map((p) => `<section class="period-card"><div class="period-head"><strong>${escape(p.name)}</strong><span class="muted">默认 ${p.work}/${p.rest}</span></div>${adjustments(p)}</section>`).join("")}`,
    `<h2 class="step-title">安排好了，按自己的节奏来</h2><p class="step-description">到点先提醒；忙的时候，可以“再等等”。</p>${agenda()}`,
  ];
  return `<div class="wizard"><div class="steps">${labels.map((label, i) => `<button data-action="step" data-step="${i + 1}" class="${state.step === i + 1 ? "active" : state.step > i + 1 ? "complete" : ""}"><span>0${i + 1}</span><strong>${label}</strong></button>`).join("")}</div>${contents[state.step - 1]}<div class="wizard-controls">${button("← 上一步", "step", `data-step="${Math.max(1, state.step - 1)}" ${state.step === 1 ? "disabled" : ""}`, "ghost")}${state.step === 4 ? button("保存并试运行 →", "run", "", "primary") : button(state.step === 3 ? "看看一天的安排 →" : "下一步 →", "step", `data-step="${state.step + 1}"`, "primary")}</div></div>`;
}

function activePeriod() {
  const s = runtimePlan();
  if (!state.useSchedule || !s?.enabled) return undefined;
  if (s.repeat === "weekly" ? !s.days.includes(3) : s.date !== "2026-10-14") return undefined;
  return s.periods.find((p) => sim.minute >= minutes(p.start) && sim.minute < minutes(p.end));
}

function startRound(p: Period, round: number) {
  sim.period = p.id; sim.round = round; sim.phase = "work"; sim.postponed = false;
  sim.active = resolveRound(p, round); sim.remaining = sim.active.work;
}

function reconcile() {
  if (sim.phase === "break") return;
  const p = activePeriod();
  if (!p) { sim.phase = "idle"; sim.remaining = 0; sim.postponed = false; }
  else if (sim.phase === "idle" || p.id !== sim.period) startRound(p, 1);
}

function finishBreak() {
  const p = activePeriod();
  if (p) startRound(p, p.id === sim.period ? sim.round + 1 : 1);
  else { sim.phase = "idle"; sim.remaining = 0; }
}

function advance(amount: number) {
  for (let i = 0; i < amount; i++) {
    sim.minute++;
    if (sim.phase === "break") {
      if (!sim.paused && --sim.remaining <= 0) finishBreak();
      continue;
    }
    const p = activePeriod();
    if (!p || sim.phase === "idle" || p.id !== sim.period) { reconcile(); continue; }
    if (!sim.paused && sim.phase === "work" && --sim.remaining <= 0) { sim.phase = "remind"; sim.postponed = false; }
  }
}

function resetSimulation() {
  const s = applied.find((item) => item.id === state.selected)!;
  sim.schedule = s.id; sim.minute = minutes(s.periods[0]?.start ?? "09:00");
  sim.phase = "idle"; sim.paused = false; sim.round = 1; sim.period = "";
  reconcile();
}

function upcoming() {
  const p = activePeriod();
  if (sim.paused) return "已暂停，恢复后更新预计时间。";
  if (sim.phase === "idle") {
    const next = runtimePlan()?.periods.filter((item) => minutes(item.start) > sim.minute).sort((a, b) => minutes(a.start) - minutes(b.start))[0];
    return next && state.useSchedule && runtimePlan().enabled ? `下一时段 <b>${next.start} · ${escape(next.name)}</b>，从第一轮开始。` : "当前没有有效安排，保持待机。";
  }
  const end = sim.minute + (sim.phase === "remind" ? sim.active.rest : sim.remaining + (sim.phase === "work" ? sim.active.rest : 0));
  if (!p || end >= minutes(p.end)) return `本次休息完成后，按当时有效时段继续；没有安排则待机。`;
  return `预计 <b>${time(end)}</b> 开始第 ${sim.round + 1} 轮工作。${sim.phase === "remind" ? "<br>按现在开始休息估算。" : ""}`;
}

function Simulation() {
  const p = runtimePlan()?.periods.find((item) => item.id === sim.period);
  const title = sim.phase === "remind" ? "该起来活动一下了" : sim.phase === "break" ? "这段时间，留给自己" : sim.phase === "idle" ? "现在不提醒" : sim.postponed ? "再忙一会儿，也没关系" : "安心工作，待会儿提醒你";
  const caption = sim.phase === "remind" ? `准备休息 ${sim.active.rest} 分钟，点击后才开始计时。` : sim.phase === "break" ? "从实际开始时计满，不因预排钟点缩短。" : sim.phase === "idle" ? "执行时段之外，安静待机。" : `本轮工作 ${sim.active.work} 分钟，再休息 ${sim.active.rest} 分钟。`;
  return `<div class="simulation"><div><div class="clock-strip"><div><span class="eyebrow">模拟时间 · 10 月 14 日 · 周三</span><time>${time(sim.minute)}</time></div>${button("从头演示", "restart", "", "ghost small")}</div><section class="live-card ${sim.phase}"><span class="status-chip">${sim.paused ? "已暂停" : sim.phase === "idle" ? "待机" : `${escape(p?.name ?? "当前时段")} · 第 ${sim.round} 轮${sim.phase === "break" ? " · 休息中" : ""}`}</span><h2>${title}</h2>
    ${sim.phase === "work" || sim.phase === "break" ? `<div class="live-time">${String(sim.remaining).padStart(2, "0")}:00</div>` : ""}<p class="muted">${caption}</p>${sim.phase === "remind" || sim.phase === "break" ? `<div class="live-content">${escape(sim.active.content)}</div>` : ""}
    <div class="live-actions">${sim.phase === "remind" ? `${button("再等等 · 5 分钟", "postpone")}${button(`开始休息 ${sim.active.rest} 分钟`, "start-break", sim.paused ? "disabled" : "", "primary")}` : sim.phase === "break" ? `${button("提前结束休息", "end-break")}${button("推进到休息结束", "advance", `data-minutes="${sim.remaining}" ${sim.paused ? "disabled" : ""}`, "primary")}` : sim.phase === "work" ? button("推进到提醒 →", "advance", `data-minutes="${sim.remaining}" ${sim.paused ? "disabled" : ""}`, "primary") : button("推进到下个时段 →", "next-boundary", "", "primary")}</div></section>
    <div class="next-up">${upcoming()}</div><div class="sim-controls">${button("＋1 分钟", "advance", 'data-minutes="1"', "small")}${button("＋5 分钟", "advance", 'data-minutes="5"', "small")}${button(sim.paused ? "恢复" : "暂停", "pause", "", "ghost small")}${button("重置本轮", "reset", "", "ghost small")}</div></div>
    <aside class="scenarios"><h3>试试这些时刻</h3><p class="muted">点一下，直接进入对应情景。</p><button class="scenario" data-action="scenario" data-scenario="delay"><strong>01 · 晚 5 分钟才开始休息</strong><span>保留本轮内容，休息仍计满。</span></button><button class="scenario" data-action="scenario" data-scenario="boundary"><strong>02 · 休息还没结束，就到午休</strong><span>先完成这次休息，再保持安静。</span></button><button class="scenario" data-action="scenario" data-scenario="afternoon"><strong>03 · 午休之后，重新开始</strong><span>下一时段从第一轮开始，不补上午。</span></button><div class="sim-caption">“＋分钟”只推进演示时钟。<br>设置里的修改需要保存后才用于试运行。<br>下午 18:00 是示例结束时间，可自行调整。</div></aside></div>`;
}

function toast(message: string) {
  clearTimeout(toastTimer);
  document.querySelector(".prototype-toast")?.remove();
  const element = document.createElement("div"); element.className = "prototype-toast"; element.setAttribute("role", "status"); element.textContent = message;
  document.body.append(element); toastTimer = setTimeout(() => element.remove(), 3500);
}

function validate() {
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
  if (error) { toast(error); return false; }
  applied = structuredClone(state.schedules); reconcile();
  toast("已保存到本次演示，刷新页面会恢复示例。"); return true;
}

function render() {
  const selected = variants.find((v) => v.key === state.variant) ?? variants[0];
  const root = document.querySelector<HTMLElement>(".scard")!;
  const general = `<details class="general-settings"><summary>通用设置 · 提示音、离开检测与语言</summary><div class="general-grid"><label><input type="checkbox" data-general="sound" ${state.sound ? "checked" : ""}>提示音</label><label><input type="checkbox" data-general="idleReset" ${state.idleReset ? "checked" : ""}>离开自动重置</label><span>界面语言 · 简体中文</span></div><p class="muted">此处保留设置页上下文，试运行不播放声音或读取电脑空闲状态。</p></details>`;
  root.innerHTML = `<div class="shead"><span class="app-icon">◷</span><h3>Sit Break</h3><span class="header-slash">/</span><span class="muted">设置</span><div class="window-dots"><i></i><i></i><i></i></div></div><main class="proto-body"><div class="page-heading"><div><span class="eyebrow">${state.view === "run" ? "让安排跟着你的节奏" : "工作与休息"}</span><h1>${state.view === "run" ? "试着过一遍今天" : "日程安排"}</h1><p>${state.view === "run" ? "不用等真实时间，看看每个操作之后会怎样。" : "设好平常的节奏，只调整不一样的那几轮。"}</p></div>${button(state.view === "run" ? "← 返回设置" : "▷ 试运行", state.view === "run" ? "edit" : "run", "", "ghost")}</div>
    ${state.view === "run" ? Simulation() : `<div class="mode-row"><div><strong>使用日程</strong><p class="muted">${state.useSchedule ? "按安排的时段执行，空档不提醒。" : "当前使用简单计时：工作 45 分钟，休息 5 分钟。"}</p></div>${switchButton("使用日程", state.useSchedule, "use-schedule")}</div>${state.useSchedule ? `<div class="schedule-tabs">${state.schedules.map((s) => `<button class="schedule-tab ${s.id === state.selected ? "selected" : ""}" data-action="select-schedule" data-id="${s.id}">${s.enabled ? '<span class="dot"></span>' : ""}${escape(s.name)}</button>`).join("")}${button("＋ 新建日程", "add-schedule", "", "ghost small")}</div>${state.variant === "B" ? VariantB() : state.variant === "C" ? VariantC() : VariantA()}` : `<div class="simple-empty"><span class="eyebrow">简单计时</span><h2>工作 45 分钟 · 休息 5 分钟</h2><p>原有节奏继续使用，你保存的日程会保留。</p></div>`}<div class="bottom-actions"><p class="muted">按电脑本地时间执行。<br>休息可以推迟，预计时间随之变化。</p>${button("保存日程", "save", "", "primary")}</div>`}${general}</main>`;
  document.querySelector(".prototype-heading")!.innerHTML = `<span><b>交互原型 · ${selected.key}</b>${selected.description}</span><span>仅用于比较方案 · 数据留在本次打开中</span>`;
  document.querySelector(".prototype-state pre")!.textContent = JSON.stringify({ variant: state.variant, view: state.view, wizardStep: state.step, useSchedule: state.useSchedule, draftSchedules: state.schedules, appliedSchedules: applied, simulation: sim }, null, 2);
  console.info("日程原型状态", { variant: state.variant, view: state.view, simulation: structuredClone(sim) });
}

document.body.classList.add("settings-prototype");
document.querySelector(".toast")?.remove();
document.body.insertAdjacentHTML("afterbegin", '<div class="prototype-heading"></div>');
document.body.insertAdjacentHTML("beforeend", '<details class="prototype-state"><summary>查看原型数据 · 当前方案、安排与模拟进度</summary><pre></pre></details>');
mountPrototypeSwitcher((key) => { state.variant = key; render(); });

document.querySelector(".scard")!.addEventListener("change", (event) => {
  const target = event.target as HTMLInputElement;
  if (target.dataset.plan) (plan() as unknown as Record<string, unknown>)[target.dataset.plan] = target.value;
  if (target.dataset.general) (state as unknown as Record<string, unknown>)[target.dataset.general] = target.checked;
  if (target.dataset.period && target.dataset.field) {
    const p = plan().periods.find((item) => item.id === target.dataset.period)!;
    const object = target.dataset.adjust === undefined ? p : p.adjustments[Number(target.dataset.adjust)];
    (object as unknown as Record<string, unknown>)[target.dataset.field] = target.type === "number" ? Math.floor(Number(target.value)) : target.value;
  }
  render();
});

document.querySelector(".scard")!.addEventListener("click", (event) => {
  const el = (event.target as HTMLElement).closest<HTMLElement>("[data-action]");
  if (!el) return;
  const p = plan().periods.find((item) => item.id === el.dataset.period);
  switch (el.dataset.action) {
    case "use-schedule": state.useSchedule = !state.useSchedule; reconcile(); break;
    case "enabled": plan().enabled = !plan().enabled; break;
    case "repeat": plan().repeat = el.dataset.value!; break;
    case "day": { const day = Number(el.dataset.day); plan().days = plan().days.includes(day) ? plan().days.filter((d) => d !== day) : [...plan().days, day].sort(); break; }
    case "weekdays": plan().days = [1, 2, 3, 4, 5]; break;
    case "everyday": plan().days = [1, 2, 3, 4, 5, 6, 7]; break;
    case "select-schedule": state.selected = el.dataset.id!; state.period = plan().periods[0]?.id ?? ""; break;
    case "add-schedule": { const s = structuredClone(sample); s.id = `schedule-${Date.now()}`; s.name = "新日程"; s.enabled = false; s.days = [6, 7]; s.periods = s.periods.map((item) => ({ ...item, id: `${s.id}-${item.id}`, adjustments: [] })); state.schedules.push(s); state.selected = s.id; state.period = s.periods[0].id; state.step = 1; break; }
    case "add-period": { const id = `period-${Date.now()}`; const lastEnd = plan().periods[plan().periods.length - 1]?.end ?? "09:00"; plan().periods.push({ id, name: `时段 ${plan().periods.length + 1}`, start: lastEnd, end: time(Math.min(23 * 60 + 59, minutes(lastEnd) + 120)), work: 55, rest: 5, adjustments: [] }); state.period = id; break; }
    case "remove-period": plan().periods = plan().periods.filter((item) => item.id !== el.dataset.period); state.period = plan().periods[0]?.id ?? ""; break;
    case "select-period": state.period = el.dataset.period!; break;
    case "toggle-adjust": if (state.openPeriods.has(p!.id)) state.openPeriods.delete(p!.id); else state.openPeriods.add(p!.id); break;
    case "add-adjust": { let round = 1; while (p!.adjustments.some((a) => a.round === round)) round++; p!.adjustments.push({ round, content: "" }); break; }
    case "remove-adjust": p!.adjustments.splice(Number(el.dataset.index), 1); break;
    case "step": state.step = Number(el.dataset.step); break;
    case "save": save(); break;
    case "run": if (save()) { state.view = "run"; resetSimulation(); } break;
    case "edit": state.view = "edit"; break;
    case "restart": resetSimulation(); break;
    case "advance": advance(Number(el.dataset.minutes)); break;
    case "start-break": sim.phase = "break"; sim.remaining = sim.active.rest; break;
    case "postpone": sim.phase = "work"; sim.remaining = Math.max(1, Math.min(5, sim.active.work)); sim.postponed = true; break;
    case "end-break": finishBreak(); break;
    case "pause": sim.paused = !sim.paused; break;
    case "reset": { const current = activePeriod(); if (current) { sim.phase = "work"; sim.remaining = sim.active.work; sim.postponed = false; } else reconcile(); break; }
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
  render();
});

render();
