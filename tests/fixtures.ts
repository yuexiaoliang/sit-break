import type { Settings } from "../src/model";
export function settings(): Settings {
  return {
    work_minutes: 45,
    break_minutes: 5,
    widget_size: 64,
    use_schedule: false,
    sound: true,
    show_widget: false,
    autostart: false,
    idle_reset: true,
    language: "zh",
    appearance: "light",
    tips: [],
    schedules: [
      {
        id: "s1",
        name: "工作日",
        enabled: true,
        repeat: { kind: "weekly", weekdays: [1, 2, 3, 4, 5] },
        periods: [
          {
            id: "p1",
            start: "09:00",
            end: "24:00",
            work_minutes: 45,
            break_minutes: 5,
            overrides: [],
          },
        ],
      },
    ],
  };
}
export const timer = {
  mode: "work" as const,
  remaining: 2700,
  paused: false,
  work_seconds: 2700,
  break_seconds: 300,
  tip: null,
  guidance: null,
  round: null,
};
