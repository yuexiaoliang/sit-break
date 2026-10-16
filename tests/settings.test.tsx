import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { settings, timer } from "./fixtures";
const backend = vi.hoisted(() => ({
  settings: {} as any,
  listeners: new Map<string, Set<(e: any) => void>>(),
  calls: [] as string[],
  saveGate: null as null | Promise<void>,
}));
vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (id: string) => "/" + id,
  invoke: vi.fn(async (name: string, args?: any) => {
    backend.calls.push(name);
    if (name === "get_settings") return structuredClone(backend.settings);
    if (name === "timer_info") return timer;
    if (name === "get_platform") return "macos";
    if (name === "save_settings") {
      await backend.saveGate;
      backend.settings = structuredClone(args.settings);
      for (const fn of backend.listeners.get("settings_changed") ?? [])
        fn({ payload: null });
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (name: string, fn: (e: any) => void) => {
    if (!backend.listeners.has(name)) backend.listeners.set(name, new Set());
    backend.listeners.get(name)!.add(fn);
    return () => backend.listeners.get(name)?.delete(fn);
  },
}));
import SettingsApp from "../src/SettingsApp";
import { AppTheme } from "../src/theme";
import { initializeRuntime } from "../src/runtime";
async function setup() {
  await initializeRuntime();
  render(
    <AppTheme>
      <SettingsApp />
    </AppTheme>,
  );
  return userEvent.setup();
}
beforeEach(() => {
  backend.settings = settings();
  backend.calls = [];
  backend.listeners.clear();
  backend.saveGate = null;
});
describe("设置主要交互", () => {
  it("独立编辑日程入口保留时长草稿，不自动打开使用日程", async () => {
    const user = await setup();
    await user.clear(screen.getByLabelText("工作时长"));
    await user.type(screen.getByLabelText("工作时长"), "30");
    await user.click(
      screen.getByRole("button", { name: "编辑日程", exact: true }),
    );
    expect(
      screen.getByRole("heading", { name: "日程安排", exact: true }),
    ).toBeTruthy();
    await user.click(
      screen.getByRole("button", { name: "计时与休息", exact: true }),
    );
    expect((screen.getByLabelText("工作时长") as HTMLInputElement).value).toBe(
      "30",
    );
    expect(
      (
        screen.getByRole("switch", {
          name: "使用日程",
          exact: true,
        }) as HTMLInputElement
      ).checked,
    ).toBe(false);
    await user.click(
      screen.getByRole("button", { name: "保存设置", exact: true }),
    );
    await screen.findByText("已保存");
    expect(backend.settings.work_minutes).toBe(30);
    expect(backend.settings.use_schedule).toBe(false);
  });
  it("24:00可保存；清空一段时间后即使换页也必须纠错，不能静默保存旧值", async () => {
    const user = await setup();
    await user.click(
      screen.getByRole("button", { name: "日程安排", exact: true }),
    );
    await user.click(
      screen.getByRole("button", { name: "保存设置", exact: true }),
    );
    await screen.findByText("已保存");
    const hours = screen.getByLabelText("开始时间 · 小时");
    await user.click(hours);
    await user.keyboard("{Backspace}");
    expect((hours as HTMLInputElement).value).toBe("");
    await user.click(
      screen.getByRole("button", { name: "提醒内容", exact: true }),
    );
    backend.calls = [];
    await user.click(
      screen.getByRole("button", { name: "保存设置", exact: true }),
    );
    expect(backend.calls).not.toContain("save_settings");
    expect(
      screen.getByRole("heading", { name: "日程安排", exact: true }),
    ).toBeTruthy();
    expect(
      (screen.getByLabelText("开始时间 · 小时") as HTMLInputElement).value,
    ).toBe("");
    expect(document.getElementById("saveError")?.textContent).toBeTruthy();
  });
  it("保存期间继续输入保留新草稿，第二次保存写入新值", async () => {
    const user = await setup();
    let release!: () => void;
    backend.saveGate = new Promise<void>((done) => {
      release = done;
    });
    await user.clear(screen.getByLabelText("工作时长"));
    await user.type(screen.getByLabelText("工作时长"), "30");
    await user.click(
      screen.getByRole("button", { name: "保存设置", exact: true }),
    );
    await waitFor(() => expect(backend.calls).toContain("save_settings"));
    await user.clear(screen.getByLabelText("工作时长"));
    await user.type(screen.getByLabelText("工作时长"), "60");
    await act(async () => release());
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toContain("未保存"),
    );
    expect((screen.getByLabelText("工作时长") as HTMLInputElement).value).toBe(
      "60",
    );
    expect(backend.settings.work_minutes).toBe(30);
    backend.saveGate = null;
    await user.click(
      screen.getByRole("button", { name: "保存设置", exact: true }),
    );
    await screen.findByText("已保存");
    expect(backend.settings.work_minutes).toBe(60);
  });
  it("主题即时预览，并与语言一起保存；下拉Escape不关闭设置窗口", async () => {
    const user = await setup();
    await user.click(
      screen.getByRole("button", { name: "通用设置", exact: true }),
    );
    const general = screen.getByRole("region", { name: "通用设置" });
    await user.click(within(general).getByText("深色", { exact: true }));
    expect(document.documentElement.dataset.mantineColorScheme).toBe("dark");
    await user.click(screen.getByRole("combobox", { name: "语言" }));
    backend.calls = [];
    await user.keyboard("{Escape}");
    expect(backend.calls).not.toContain("hide_settings");
    await user.click(screen.getByRole("combobox", { name: "语言" }));
    await user.click(
      await screen.findByRole("option", { name: "English", exact: true }),
    );
    await user.click(within(general).getByText("Light", { exact: true }));
    expect(document.documentElement.lang).toBe("en");
    await user.click(within(general).getByText("Dark", { exact: true }));
    expect(document.documentElement.lang).toBe("en");
    await user.click(
      screen.getByRole("button", { name: "Save settings", exact: true }),
    );
    await screen.findByText("Saved");
    expect(backend.settings.appearance).toBe("dark");
    expect(backend.settings.language).toBe("en");
  });
});
