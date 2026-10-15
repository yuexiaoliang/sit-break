import type { Guidance } from "./guidance";

/** 后端计时应用入口的完整状态，初次读取与 tick 事件共用。 */
export interface TimerState {
  mode: "work" | "remind" | "break" | "idle";
  remaining: number;
  paused: boolean;
  work_seconds: number;
  break_seconds: number;
  tip: string | null;
  guidance: Guidance | null;
  round: { schedule_id: string; period_id: string; date: string; number: number } | null;
}
