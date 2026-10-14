import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { STR, currentLang } from "./i18n";
import {
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
  availableMonitors,
} from "@tauri-apps/api/window";

interface Tick {
  mode: "work" | "remind" | "break" | "idle";
  remaining: number;
  paused: boolean;
}

interface Settings {
  work_minutes: number;
  break_minutes: number;
  sound: boolean;
  show_widget: boolean;
  autostart: boolean;
  widget_size: number;
  tips: string[];
}

const ball = document.getElementById("ball")!;
const ring = document.getElementById("ring") as unknown as SVGCircleElement;
const head = document.getElementById("head") as unknown as SVGCircleElement;
const num = document.getElementById("num")!;
const win = getCurrentWindow();

let workTotal = 45 * 60;
let breakTotal = 5 * 60;
let lastSize = 64;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let S = STR.zh;

function render(t: Tick) {
  const total = t.mode === "break" ? breakTotal : workTotal;
  const progress = total > 0 ? Math.min(1, t.remaining / total) : 0;
  ring.style.strokeDashoffset = String(100 - progress * 100);
  // 进度头部小珠：随倒计时沿圆环移动
  const phi = progress * 2 * Math.PI;
  head.setAttribute("cx", String(50 + 43 * Math.cos(phi)));
  head.setAttribute("cy", String(50 + 43 * Math.sin(phi)));
  head.style.opacity = t.paused || t.mode === "idle" ? "0" : "1";
  // 始终读秒，让用户每秒都能感知时间在走
  const mm = Math.floor(t.remaining / 60);
  const ss = t.remaining % 60;
  num.textContent = t.mode === "idle" ? S.idle : `${mm}:${String(ss).padStart(2, "0")}`;
  num.style.fontSize = `${Math.round(lastSize * (t.mode === "idle" ? 0.24 : 0.3))}px`;
  ball.classList.toggle("paused", t.paused);
  ball.classList.toggle("break", t.mode === "break");
  ball.classList.toggle("idle", t.mode === "idle");
}

function applySize(size: number) {
  lastSize = size;
  ball.style.width = `${size}px`;
  ball.style.height = `${size}px`;
  num.style.fontSize = `${Math.round(size * 0.3)}px`;
  // Windows 有最小窗口宽度限制；macOS 只留出阴影所需的边距。
  const winSize = navigator.userAgent.includes("Windows")
    ? Math.max(size + 24, 140)
    : size + 24;
  win.setSize(new LogicalSize(winSize, winSize));
}

async function loadSize() {
  const s = await invoke<Settings>("get_settings");
  workTotal = s.work_minutes * 60;
  breakTotal = s.break_minutes * 60;
  applySize(Math.min(140, Math.max(40, s.widget_size)));
  return s;
}

// 计时状态
listen<Tick>("tick", ({ payload }) => render(payload));

// 屏蔽 WebView 右键菜单；整个球任意位置可拖动
window.addEventListener("contextmenu", (e) => e.preventDefault());
ball.addEventListener("mousedown", (e) => {
  if (e.button === 0) {
    e.preventDefault();
    win.startDragging();
  }
});

// 滚轮调节大小（自动保存）
ball.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    const delta = e.deltaY < 0 ? 4 : -4;
    const next = Math.min(140, Math.max(40, lastSize + delta));
    if (next === lastSize) return;
    applySize(next);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      const s = await invoke<Settings>("get_settings");
      await invoke("save_settings", {
        settings: { ...s, widget_size: next },
      });
    }, 500);
  },
  { passive: false }
);

listen("settings_changed", async () => {
  S = STR[await currentLang()];
  await loadSize();
  render(await invoke<Tick>("timer_info"));
});

// 恢复上次拖动的位置
const saved = localStorage.getItem("widgetPos");
if (saved) {
  try {
    const { x, y } = JSON.parse(saved);
    if (Number.isFinite(x) && Number.isFinite(y)) {
      const size = await win.outerSize();
      const monitors = await availableMonitors();
      const monitor = monitors.find(({ workArea: area }) =>
        x + size.width / 2 >= area.position.x &&
        x + size.width / 2 < area.position.x + area.size.width &&
        y + size.height / 2 >= area.position.y &&
        y + size.height / 2 < area.position.y + area.size.height
      );
      if (monitor) {
        const area = monitor.workArea;
        const safeX = Math.max(area.position.x, Math.min(x, area.position.x + area.size.width - size.width));
        const safeY = Math.max(area.position.y, Math.min(y, area.position.y + area.size.height - size.height));
        await win.setPosition(new PhysicalPosition(safeX, safeY));
      }
    }
  } catch {
    // 位置失效时忽略（如换了显示器）
  }
}
let moveTimer: ReturnType<typeof setTimeout> | undefined;
await win.onMoved(({ payload }) => {
  clearTimeout(moveTimer);
  moveTimer = setTimeout(() => {
    localStorage.setItem(
      "widgetPos",
      JSON.stringify({ x: payload.x, y: payload.y })
    );
  }, 400);
});

// 初始渲染放最后，即使失败也不影响拖动等已注册的能力
await loadSize();
S = STR[await currentLang()];
render(await invoke<Tick>("timer_info"));
