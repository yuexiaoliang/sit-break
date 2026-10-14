/// <reference types="vite/client" />

// 临时原型入口：只在开发环境显式选择后加载，普通设置页仍走现有逻辑。
if (import.meta.env.DEV && new URLSearchParams(location.search).get("prototype") === "schedule") {
  await import("./schedule.prototype");
} else {
  await import("./settings");
}

export {};
