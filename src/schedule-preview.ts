import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { fmt } from "./i18n";
import type { TimerState } from "./timer-state";
import type { Guidance } from "./guidance";

interface PreviewRound {
  number: number;
  work_start: number;
  break_start: number | null;
  break_end: number | null;
  work_seconds: number;
  break_seconds: number;
  content: string | null;
  guidance: Guidance | null;
}
interface PreviewData {
  local_date: string;
  current: { state: TimerState; break_start: number | null; break_end: number | null } | null;
  schedules: {
    id: string; name: string; enabled: boolean; date: string; applies_today: boolean; uses_progress: boolean;
    repeat: { kind: string; date?: string; weekdays?: number[] };
    periods: { id: string; start: string; end: string; rounds: PreviewRound[] }[];
    gaps: { start: number; end: number }[];
  }[];
}

/** 浮窗只渲染 Rust 返回的预计安排，不推进计时或改写表单。 */
export function initSchedulePreview(getDraft: () => unknown, getStrings: () => Record<string, string>) {
  const dialog = document.getElementById("schedulePreview") as HTMLDialogElement;
  const trigger = document.getElementById("previewSchedule") as HTMLButtonElement;
  const close = document.getElementById("closePreview") as HTMLButtonElement;
  const body = document.getElementById("previewBody")!;
  const note = document.getElementById("previewNote")!;
  let request = 0;
  let lastRendered = "";

  function clock(seconds: number | null) {
    if (seconds === null) return "—";
    const days = Math.floor(seconds / 86400);
    const withinDay = ((seconds % 86400) + 86400) % 86400;
    const time = `${String(Math.floor(withinDay / 3600)).padStart(2, "0")}:${String(Math.floor(withinDay / 60) % 60).padStart(2, "0")}`;
    return days === 0 ? time : `${time} (${days > 0 ? "+" : ""}${days})`;
  }
  function element(tag: string, text: string, className = "") {
    const item = document.createElement(tag);
    item.textContent = text;
    if (className) item.className = className;
    return item;
  }
  function rhythm(work: number, rest: number) { return fmt(getStrings().previewRhythm, { work: work / 60, rest: rest / 60 }); }
  function render(data: PreviewData) {
    const S = getStrings();
    const content = document.createDocumentFragment();
    if (data.current) {
      const current = data.current;
      const active = document.createElement("section");
      active.className = "preview-current";
      const phase = current.state.paused ? S.paused : current.state.mode === "break" ? S.onBreak : current.state.mode === "remind" ? S.reminding : S.working;
      active.append(element("h3", `${S.previewCurrentRound} · ${current.state.round ? fmt(S.roundNumber, { n: current.state.round.number }) : S.simpleTitle} · ${phase}`));
      active.append(element("p", rhythm(current.state.work_seconds, current.state.break_seconds), "muted"));
      const times = current.state.mode === "break"
        ? fmt(S.previewBreakEnds, { end: clock(current.break_end) })
        : current.break_start === null ? S.previewNoBreakBeforeEnd : fmt(S.previewBreakTimes, { start: clock(current.break_start), end: clock(current.break_end) });
      active.append(element("p", times, "preview-time"));
      if (current.state.mode === "remind") active.append(element("p", S.previewWaiting, "muted"));
      if (current.state.tip) active.append(element("p", current.state.tip, "preview-content-text"));
      if (current.state.guidance) active.append(element("p", S.guidance, "muted"));
      if (!current.state.tip && !current.state.guidance) active.append(element("p", S.previewRandomTip, "preview-content-text"));
      content.append(active);
    }
    if (!data.schedules.length) content.append(element("p", S.emptySchedulesDescription, "preview-empty"));
    for (const schedule of data.schedules) {
      const section = document.createElement("section");
      section.className = "preview-schedule";
      section.append(element("h3", schedule.name || S.previewUnnamed));
      const repeat = schedule.repeat.kind === "once"
        ? `${S.repeatOnce} · ${schedule.repeat.date}`
        : `${S.repeatWeekly} · ${(schedule.repeat.weekdays ?? []).map((day) => S[`weekday${day}`]).join(" / ")}`;
      section.append(element("p", repeat, "muted"));
      section.append(element("p", schedule.enabled ? (schedule.uses_progress ? S.previewUsingProgress : S.previewFromStart) : S.previewDisabled, "preview-plan-note"));
      if (schedule.repeat.kind === "weekly") section.append(element("p", `${schedule.date}${schedule.applies_today ? "" : ` · ${S.previewNotToday}`}`, "muted"));
      if (!schedule.periods.length) section.append(element("p", S.emptyPeriodsDescription, "preview-empty"));
      for (const period of schedule.periods) {
        const card = document.createElement("section");
        card.className = "preview-period";
        card.append(element("h4", `${period.start}–${period.end} · ${S.executionPeriod}`));
        const matchingCurrent = data.current?.state.round;
        const currentHere = schedule.uses_progress && matchingCurrent?.schedule_id === schedule.id && matchingCurrent.period_id === period.id && matchingCurrent.date === schedule.date;
        const firstBreak = currentHere ? data.current!.break_start : period.rounds[0]?.break_start;
        if (firstBreak !== undefined && firstBreak !== null) card.append(element("p", `${currentHere ? S.previewNextBreak : S.previewFirstBreak} · ${clock(firstBreak)}`, "preview-first-break"));
        if (!period.rounds.length) card.append(element("p", S.previewNoUpcoming, "muted"));
        for (const round of period.rounds) {
          const row = document.createElement("div");
          row.className = "preview-round";
          row.append(element("h5", fmt(S.roundNumber, { n: round.number })));
          row.append(element("p", `${S.previewWorkStarts} ${clock(round.work_start)} · ${rhythm(round.work_seconds, round.break_seconds)}`, "muted"));
          row.append(element("p", round.break_start === null ? S.previewNoBreakBeforeEnd : fmt(S.previewBreakTimes, { start: clock(round.break_start), end: clock(round.break_end) }), "preview-time"));
          if (round.content) row.append(element("p", round.content, "preview-content-text"));
          if (round.guidance) row.append(element("p", S.guidance, "muted"));
          if (!round.content && !round.guidance) row.append(element("p", S.previewRandomTip, "preview-content-text"));
          card.append(row);
        }
        section.append(card);
        for (const gap of schedule.gaps.filter((gap) => clock(gap.start) === period.end)) {
          section.append(element("p", fmt(S.quietPeriod, { start: clock(gap.start), end: clock(gap.end) }), "preview-gap"));
        }
      }
      content.append(section);
    }
    const scrollTop = body.scrollTop;
    body.replaceChildren(content);
    body.scrollTop = scrollTop;
    note.textContent = `${S.previewNote}${data.current?.state.paused ? ` ${S.previewPaused}` : ""}`;
  }

  async function refresh() {
    if (!dialog.open) return;
    const revision = ++request;
    try {
      const data = await invoke<PreviewData>("preview_schedule", { draft: getDraft() });
      if (!dialog.open || revision !== request) return;
      const serialized = JSON.stringify(data);
      if (serialized === lastRendered) return;
      lastRendered = serialized;
      render(data);
    } catch (error) {
      if (!dialog.open || revision !== request) return;
      const S = getStrings();
      const response = error as { errors?: { code: string }[] };
      const message = response?.errors?.map((issue) => S[`validation_${issue.code}`] ?? S.fixSchedule).join("\n") ?? String(error);
      body.replaceChildren(element("p", `${S.previewUnavailable}\n${message}`, "preview-error"));
      lastRendered = "";
    }
  }
  function closeDialog() {
    request += 1;
    dialog.close();
    trigger.focus();
  }
  trigger.addEventListener("click", () => {
    lastRendered = "";
    body.replaceChildren(element("p", getStrings().previewLoading, "preview-empty"));
    dialog.showModal();
    close.focus();
    refresh();
  });
  close.addEventListener("click", closeDialog);
  dialog.addEventListener("cancel", (event) => { event.preventDefault(); closeDialog(); });
  dialog.addEventListener("click", (event) => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) closeDialog();
  });
  dialog.addEventListener("keydown", (event) => {
    if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); closeDialog(); }
    if (event.key === "Tab") {
      event.preventDefault();
      (document.activeElement === close ? body : close).focus();
    }
  });
  listen("tick", refresh);
  listen("settings_changed", refresh);
}
