import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { currentLang, applyStatic } from "./i18n";

interface Settings {
  work_minutes: number;
  break_minutes: number;
  sound: boolean;
  show_widget: boolean;
  autostart: boolean;
  language: string;
  widget_size: number;
  tips: string[];
}

const workMin = document.getElementById("workMin") as HTMLInputElement;
const breakMin = document.getElementById("breakMin") as HTMLInputElement;
const widgetSize = document.getElementById("widgetSize") as HTMLInputElement;
const tipsList = document.getElementById("tipsList") as HTMLTextAreaElement;
const language = document.getElementById("language") as HTMLSelectElement;
const btnSave = document.getElementById("btnSave") as HTMLButtonElement;
const toast = document.getElementById("toast")!;

const switches = {
  sound: document.getElementById("swSound")!,
  show_widget: document.getElementById("swWidget")!,
  autostart: document.getElementById("swAuto")!,
} as Record<string, HTMLElement>;

function setSwitch(key: string, on: boolean) {
  switches[key].classList.toggle("on", on);
}

async function load() {
  const s = await invoke<Settings>("get_settings");
  workMin.value = String(s.work_minutes);
  breakMin.value = String(s.break_minutes);
  widgetSize.value = String(Math.min(140, Math.max(40, s.widget_size)));
  tipsList.value = s.tips.join("\n");
  language.value = ["zh", "en"].includes(s.language) ? s.language : "system";
  setSwitch("sound", s.sound);
  setSwitch("show_widget", s.show_widget);
  setSwitch("autostart", s.autostart);
}

function current(): Settings {
  return {
    work_minutes: Math.min(240, Math.max(1, Number(workMin.value) || 45)),
    break_minutes: Math.min(60, Math.max(1, Number(breakMin.value) || 5)),
    widget_size: Math.min(140, Math.max(40, Number(widgetSize.value) || 64)),
    sound: switches.sound.classList.contains("on"),
    show_widget: switches.show_widget.classList.contains("on"),
    autostart: switches.autostart.classList.contains("on"),
    language: language.value,
    tips: tipsList.value
      .split("\n")
      .map((t) => t.trim())
      .filter(Boolean),
  };
}

for (const el of Object.values(switches)) {
  el.addEventListener("click", () => el.classList.toggle("on"));
}

btnSave.addEventListener("click", async () => {
  btnSave.disabled = true;
  try {
    await invoke("save_settings", { settings: current() });
    await load(); // 语言切换后文案与默认小字可能已变，重新加载
    toast.classList.add("show");
    setTimeout(() => {
      toast.classList.remove("show");
      invoke("hide_settings");
    }, 600);
  } finally {
    btnSave.disabled = false;
  }
});

document.getElementById("btnClose")!.addEventListener("click", () => invoke("hide_settings"));
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") invoke("hide_settings");
});
window.addEventListener("contextmenu", (e) => e.preventDefault());

// 标题栏拖动（点在文字/图标上也生效）
const win = getCurrentWindow();
const shead = document.querySelector<HTMLElement>(".shead")!;
shead.addEventListener("mousedown", (e) => {
  if (e.button === 0 && !(e.target as HTMLElement).closest("button")) {
    e.preventDefault();
    win.startDragging();
  }
});

await load();
listen("settings_open", load);

// 语言初始化：静态文案 + 语言下拉框
(async () => {
  const lang = await currentLang();
  applyStatic(lang);
  const select = document.querySelector<HTMLElement>(".selwrap select")!;
  if (lang === "en") select.style.fontWeight = "600";
})();
