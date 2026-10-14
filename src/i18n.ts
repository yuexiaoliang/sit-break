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
    ballSize: "悬浮计时条大小",
    idle: "待机",
    idleDescription: "离开达到休息时长，重新计时",
    idleStatus: "现在不提醒，打开设置继续配置日程。",
    settingsTitle: "设置",
    closeSettings: "关闭设置",
    workLength: "工作时长",
    simpleTitle: "简单计时",
    simpleDescription: "设定工作与休息时长，到点提醒你。",
    simpleSummary: "每工作 {work} 分钟，提醒休息 {rest} 分钟。",
    useSchedule: "使用日程",
    scheduleTitle: "日程安排",
    scheduleDescription: "设好平常的节奏，只调整不一样的那几轮。",
    scheduleOptionalDescription: "按日期和时段安排不同节奏。",
    scheduleModeDescription: "按时段执行，空档不提醒。关闭后回到简单计时 {work}/{rest}。",
    emptySchedules: "还没有日程",
    emptySchedulesDescription: "添加一份日程，安排自己的节奏。没有执行时段时保持待机。",
    addSchedule: "＋ 添加日程",
    newSchedule: "新日程",
    scheduleName: "日程名称",
    enableSchedule: "启用日程",
    onceDate: "仅执行一次的日期",
    emptyPeriods: "还没有执行时段",
    emptyPeriodsDescription: "这份日程尚未安排执行时段，保存后保持待机。",
    removeSchedule: "删除日程",
    commonSettings: "通用设置",
    sharedSettings: "两种计时方式共用",
    soundDescription: "提醒时播放轻柔提示音",
    autostartDescription: "登录电脑后自动开始",
    tipsHeading: "提醒小字",
    tipsHint: "每行一条，随机展示",
    tipsDescription: "留空时使用默认提示。",
    saveSettings: "保存设置",
    saveFailed: "保存失败，请重试。",
    everyN: "每 {n} 分钟",
    forN: "{n} 分钟",
    sound: "提示音",
    showWidget: "显示悬浮计时条",
    autostart: "开机自启",
    idleReset: "离开自动重置",
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
    ballSize: "Floating timer size",
    idle: "Idle",
    idleDescription: "Reset after being away for a full break",
    idleStatus: "No reminders now. Open settings to arrange a schedule.",
    settingsTitle: "Settings",
    closeSettings: "Close settings",
    workLength: "Work length",
    simpleTitle: "Simple timer",
    simpleDescription: "Set your work and break lengths. We'll remind you.",
    simpleSummary: "A {rest} min break after every {work} min of work.",
    useSchedule: "Use schedules",
    scheduleTitle: "Schedules",
    scheduleDescription: "Set your usual rhythm, adjust only what differs.",
    scheduleOptionalDescription: "Arrange your rhythm by date and time period.",
    scheduleModeDescription: "Reminders only during scheduled periods. Turn off for the {work}/{rest} simple timer.",
    emptySchedules: "No schedules yet",
    emptySchedulesDescription: "Add a schedule to arrange your rhythm. The timer stays idle without a time period.",
    addSchedule: "+ Add schedule",
    newSchedule: "New schedule",
    scheduleName: "Schedule name",
    enableSchedule: "Enable schedule",
    onceDate: "One-time date",
    emptyPeriods: "No time periods yet",
    emptyPeriodsDescription: "This schedule has no time periods. It will stay idle after saving.",
    removeSchedule: "Delete schedule",
    commonSettings: "General settings",
    sharedSettings: "Shared by both timer modes",
    soundDescription: "Play a gentle chime with reminders",
    autostartDescription: "Start automatically when you sign in",
    tipsHeading: "Reminder tips",
    tipsHint: "One per line, shown randomly",
    tipsDescription: "Leave blank to use the default tips.",
    saveSettings: "Save settings",
    saveFailed: "Couldn't save. Please try again.",
    everyN: "Every {n} min",
    forN: "{n} min",
    sound: "Sound",
    showWidget: "Show floating timer",
    autostart: "Launch at startup",
    idleReset: "Auto-reset when away",
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
