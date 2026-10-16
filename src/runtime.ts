import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { STR, type Lang } from "./i18n";
import { normalizeSettings, type Settings } from "./model";
import type { TimerState } from "./timer-state";

export interface Runtime {
  settings: Settings;
  timer: TimerState;
  platform: string;
}
interface RuntimeStore {
  snapshot?: Runtime;
  subscribers: Set<() => void>;
}
// 开发热更新保留完整状态，避免组件刷新时读到尚未完成初始化的新模块。
const store: RuntimeStore = import.meta.hot?.data.runtimeStore ?? {
  subscribers: new Set(),
};
if (import.meta.hot) import.meta.hot.data.runtimeStore = store;
const subscribers = store.subscribers;
function publish(value: Runtime) {
  store.snapshot = value;
  subscribers.forEach((fn) => fn());
}
export function resolveLang(value: string): Lang {
  return value === "zh" || value === "en"
    ? value
    : (navigator.language || "en").toLowerCase().startsWith("zh")
      ? "zh"
      : "en";
}
export function useRuntime() {
  return useSyncExternalStore(
    (fn) => {
      subscribers.add(fn);
      return () => {
        subscribers.delete(fn);
      };
    },
    () => store.snapshot!,
  );
}
export function useSettings() {
  return useSyncExternalStore(
    (fn) => {
      subscribers.add(fn);
      return () => {
        subscribers.delete(fn);
      };
    },
    () => store.snapshot!.settings,
  );
}
export function usePlatform() {
  return useSyncExternalStore(
    (fn) => {
      subscribers.add(fn);
      return () => {
        subscribers.delete(fn);
      };
    },
    () => store.snapshot!.platform,
  );
}
export function useStrings(language?: string) {
  const settings = useSettings();
  return STR[resolveLang(language ?? settings.language)];
}
export async function refreshSettings() {
  const settings = normalizeSettings(await invoke<Settings>("get_settings"));
  if (store.snapshot) publish({ ...store.snapshot, settings });
  return settings;
}
export async function initializeRuntime() {
  await listen<TimerState>("tick", ({ payload }) => {
    if (store.snapshot) publish({ ...store.snapshot, timer: payload });
  });
  await listen("settings_changed", () => {
    void refreshSettings().catch(console.error);
  });
  const [settings, timer, platform] = await Promise.all([
    invoke<Settings>("get_settings"),
    invoke<TimerState>("timer_info"),
    invoke<string>("get_platform"),
  ]);
  publish({ settings: normalizeSettings(settings), timer, platform });
}
export function formatTime(seconds: number) {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
