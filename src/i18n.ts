import { invoke } from "@tauri-apps/api/core";

export type Lang = "zh" | "en";

export const STR: Record<Lang, Record<string, string>> = {
  zh: {
    pause: "暂停计时",
    resume: "开始计时",
    reset: "重置计时",
    settings: "设置…",
    quit: "退出",
    minutesUnit: "分钟",
    working: "工作中",
    paused: "已暂停",
    onBreak: "休息中",
    reminding: "提醒中",
    interval: "提醒间隔",
    breakLen: "休息时长",
    ballSize: "悬浮球大小",
    everyN: "每 {n} 分钟",
    forN: "{n} 分钟",
    sound: "提示音",
    showWidget: "显示悬浮计时条",
    autostart: "开机自启",
    language: "语言",
    langSys: "跟随系统",
    tipsLabel: "提醒小字（每行一条，随机展示）",
    save: "保存",
    saved: "已保存 ✓",
    satFor: "已连续坐了 {n} 分钟",
    timeToMove: "该起来活动一下了",
    standUp: "站起来走两步，身体会谢谢你",
    startBreak: "开始休息 {n} 分钟",
    snooze: "再等等",
    untilBack: "{t} 分钟后回到工作",
    endBreak: "提前结束休息",
  },
  en: {
    pause: "Pause",
    resume: "Resume",
    reset: "Reset",
    settings: "Settings…",
    quit: "Quit",
    minutesUnit: "min",
    working: "Working",
    paused: "Paused",
    onBreak: "On a break",
    reminding: "Reminding",
    interval: "Reminder interval",
    breakLen: "Break length",
    ballSize: "Ball size",
    everyN: "Every {n} min",
    forN: "{n} min",
    sound: "Sound",
    showWidget: "Show floating timer",
    autostart: "Launch at startup",
    language: "Language",
    langSys: "System",
    tipsLabel: "Tips (one per line, shown randomly)",
    save: "Save",
    saved: "Saved ✓",
    satFor: "Sitting for {n} min",
    timeToMove: "Time to move",
    standUp: "Stand up and take a short walk — your body will thank you",
    startBreak: "Take a break ({n} min)",
    snooze: "Snooze",
    untilBack: "Back to work in {t}",
    endBreak: "End break",
  },
};

export function fmt(tpl: string, params: Record<string, string | number>): string {
  return tpl.replace(/\{(\w+)\}/g, (_, k) => String(params[k] ?? ""));
}

export async function currentLang(): Promise<Lang> {
  try {
    const s = await invoke<{ language: string }>("get_settings");
    if (s.language === "zh") return "zh";
    if (s.language === "en") return "en";
  } catch {
    // 设置不可用时跟随浏览器语言
  }
  return (navigator.language || "en").toLowerCase().startsWith("zh") ? "zh" : "en";
}

export function applyStatic(lang: Lang) {
  const dict = STR[lang];
  document.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => {
    const v = dict[el.dataset.i18n!];
    if (v !== undefined) el.textContent = v;
  });
}

// 读取当前语言并刷新静态文案；语言切换后各窗口调用它重新生效
export async function initLang(): Promise<Lang> {
  const lang = await currentLang();
  applyStatic(lang);
  return lang;
}
