import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { settings, timer } from "./fixtures";
const backend = vi.hoisted(() => ({
  settings: {} as any,
  listeners: new Map<string, Set<(e: any) => void>>(),
  calls: [] as string[],
  saveGate: null as null | Promise<void>,
  capacities: [] as Array<{
    complete_rounds: number;
    last_adjustable_round: number;
    remaining_minutes: number;
  }>,
  preview: null as null | ((mode: string) => unknown),
}));
vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (id: string) => "/" + id,
  invoke: vi.fn(async (name: string, args?: any) => {
    backend.calls.push(name);
    if (name === "get_settings") return structuredClone(backend.settings);
    if (name === "timer_info") return timer;
    if (name === "get_platform") return "macos";
    if (name === "preview_schedule") return backend.preview?.(args.mode);
    if (name === "schedule_period_capacities") return backend.capacities;
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
  backend.capacities = [];
  backend.preview = null;
});
describe("设置主要交互", () => {
  it("草稿显示提醒时段，当前进度显示已开始的休息倒计时", async () => {
    const date = "2026-11-11";
    const period = { id: "p1", start: "22:00", end: "24:00" };
    const round = {
      number: 2,
      work_start: 82800,
      break_start: 86100,
      break_end: 86400,
      work_seconds: 3300,
      break_seconds: 300,
      content: "核心训练",
      guidance: null,
    };
    const schedule = {
      id: "s1",
      name: "工作日",
      enabled: true,
      repeat: { kind: "weekly", weekdays: [3] },
      date,
      applies_today: true,
      gaps: [],
    };
    backend.preview = (mode) => {
      const progress = mode === "progress";
      return {
        current: progress
          ? {
              state: {
                ...timer,
                mode: "break",
                remaining: 120,
                round: { schedule_id: "s1", period_id: "p1", date, number: 1 },
              },
              break_start: 82500,
              break_end: 82800,
            }
          : null,
        schedules: [
          {
            ...schedule,
            uses_progress: progress,
            periods: [
              {
                ...period,
                rounds: progress
                  ? [{
                      ...round,
                      number: 2,
                      work_start: 82800,
                      content: "本轮休息内容",
                    }]
                  : [round],
              },
            ],
          },
        ],
      };
    };

    const user = await setup();
    await user.click(
      screen.getByRole("button", { name: "日程安排", exact: true }),
    );
    const openPreview = screen.getByRole("button", { name: "预览日程" });
    await user.click(openPreview);
    const dialog = within(screen.getByRole("dialog"));
    await dialog.findByText("核心训练");
    expect(dialog.getByText("休息提醒 23:55–24:00")).toBeTruthy();
    dialog.getByRole("radio", { name: "草稿计划" }).focus();
    await user.keyboard("{ArrowRight}");
    await dialog.findByText("工作 23:00–23:55");
    expect(dialog.getByText("剩余 2:00")).toBeTruthy();
    expect(dialog.getByText("本轮休息内容")).toBeTruthy();

    await user.keyboard("{Escape}");
    await waitFor(() => expect(document.activeElement).toBe(openPreview));
    await user.click(openPreview);
    await screen.findByText("核心训练");
    expect(backend.calls).not.toContain("save_settings");
  });
  it("时段排满后点击添加立即提示，并保留已有轮次", async () => {
    const period = backend.settings.schedules[0].periods[0];
    period.end = "12:00";
    period.work_minutes = 55;
    period.overrides = [{ round: 1 }, { round: 2 }, { round: 3 }];
    backend.capacities = [
      { complete_rounds: 3, last_adjustable_round: 3, remaining_minutes: 0 },
    ];
    const user = await setup();
    await user.click(
      screen.getByRole("button", { name: "日程安排", exact: true }),
    );
    await user.click(screen.getByRole("button", { name: /单轮调整（可选）/ }));
    await user.click(await screen.findByRole("button", { name: "添加单轮调整" }));
    await screen.findByText(/第 4 轮放不进此时段/);
    expect(screen.getAllByLabelText("轮次序号")).toHaveLength(3);
    expect(backend.calls).not.toContain("save_settings");
    expect(backend.calls).not.toContain("preview_schedule");
    await user.click(screen.getByRole("button", { name: "保存设置" }));
    await screen.findByText("已保存");
    expect(backend.settings.schedules[0].periods[0].overrides).toHaveLength(3);
  });
  it("时段有余量时添加可缩短的轮次并立即显示可用时间", async () => {
    const period = backend.settings.schedules[0].periods[0];
    period.end = "10:05";
    period.overrides = [{ round: 1 }];
    backend.capacities = [
      { complete_rounds: 1, last_adjustable_round: 2, remaining_minutes: 15 },
    ];
    const user = await setup();
    await user.click(
      screen.getByRole("button", { name: "日程安排", exact: true }),
    );
    const editor = within(screen.getByRole("region", { name: "日程安排" }));
    await user.click(editor.getByRole("button", { name: /单轮调整（可选）/ }));
    await user.click(await editor.findByRole("button", { name: "添加单轮调整" }));
    await editor.findByText(/第 2 轮可用 15 分钟/);
    expect(editor.getAllByLabelText("轮次序号")).toHaveLength(2);
    expect(backend.calls).not.toContain("save_settings");

    backend.capacities = [
      { complete_rounds: 2, last_adjustable_round: 2, remaining_minutes: 0 },
    ];
    await user.type(editor.getAllByLabelText("工作时长", { exact: true })[1], "10");
    await waitFor(() => expect(editor.queryByText(/第 2 轮可用/)).toBeNull());
    await user.click(screen.getByRole("button", { name: "保存设置" }));
    await screen.findByText("已保存");
    expect(backend.settings.schedules[0].periods[0].overrides[1]).toEqual({
      round: 2,
      work_minutes: 10,
    });
  });
  it("切换计时方式及编辑日程保留草稿，导航不改变所选方式", async () => {
    const user = await setup();
    await user.clear(screen.getByLabelText("工作时长"));
    await user.type(screen.getByLabelText("工作时长"), "30");
    await user.click(screen.getByText("日程计时", { exact: true }));
    expect(screen.queryByRole("textbox", { name: "工作时长" })).toBeNull();
    expect(
      screen.getByRole("region", { name: "日程概览" }).textContent,
    ).toContain("09:00–24:00");
    await user.click(
      screen.getByRole("button", { name: "编辑日程", exact: true }),
    );
    expect(
      screen.getByRole("heading", { name: "日程安排", exact: true }),
    ).toBeTruthy();
    await user.click(
      screen.getByRole("button", { name: "计时与休息", exact: true }),
    );
    expect(
      (screen.getByRole("radio", { name: "日程计时" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    await user.click(screen.getByText("简单计时", { exact: true }));
    expect((screen.getByLabelText("工作时长") as HTMLInputElement).value).toBe(
      "30",
    );
    expect(
      (
        screen.getByRole("radio", {
          name: "日程计时",
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
  it("已有日程模式直接显示概览；模式切换保留日程修改并可保存", async () => {
    backend.settings.use_schedule = true;
    const user = await setup();
    expect(screen.queryByRole("textbox", { name: "工作时长" })).toBeNull();
    await user.click(
      screen.getByRole("button", { name: "编辑日程", exact: true }),
    );
    const hours = screen.getByLabelText("开始时间 · 小时");
    await user.click(hours);
    await user.keyboard("10");
    await user.click(
      screen.getByRole("button", { name: "计时与休息", exact: true }),
    );
    await user.click(screen.getByText("简单计时", { exact: true }));
    await user.click(screen.getByText("日程计时", { exact: true }));
    await user.click(
      screen.getByRole("button", { name: "保存设置", exact: true }),
    );
    await screen.findByText("已保存");
    expect(backend.settings.use_schedule).toBe(true);
    expect(backend.settings.schedules[0].periods[0].start).toBe("10:00");
    expect(
      screen.getByRole("region", { name: "日程概览" }).textContent,
    ).toContain("10:00–24:00");
    expect(screen.getByRole("switch", { name: "提示音" })).toBeTruthy();
    expect(screen.queryByRole("switch", { name: "离开自动重置" })).toBeNull();
  });
  it("日程空状态可以直接添加日程，并保留选中的计时方式", async () => {
    backend.settings.schedules = [];
    const user = await setup();
    await user.click(screen.getByText("日程计时", { exact: true }));
    expect(
      screen.getByRole("region", { name: "日程概览" }).textContent,
    ).toContain("还没有日程");
    await user.click(
      screen.getByRole("button", { name: "添加日程", exact: true }),
    );
    const name = screen.getByLabelText("日程名称");
    expect(document.activeElement).toBe(name);
    await user.clear(name);
    await user.type(name, "晨间安排");
    await user.click(
      screen.getByRole("button", { name: "计时与休息", exact: true }),
    );
    expect(
      screen.getByRole("region", { name: "日程概览" }).textContent,
    ).toContain("晨间安排");
    expect(
      (screen.getByRole("radio", { name: "日程计时" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    await user.click(
      screen.getByRole("button", { name: "保存设置", exact: true }),
    );
    await screen.findByText("已保存");
    expect(backend.settings.schedules[0].name).toBe("晨间安排");
    expect(backend.settings.use_schedule).toBe(true);
  });
  it("隐藏的简单计时时长有误时，提供可聚焦的修正入口，不静默改变模式", async () => {
    const user = await setup();
    await user.clear(screen.getByLabelText("工作时长"));
    await user.click(screen.getByText("日程计时", { exact: true }));
    backend.calls = [];
    await user.click(
      screen.getByRole("button", { name: "保存设置", exact: true }),
    );
    expect(backend.calls).not.toContain("save_settings");
    const correct = screen.getByRole("button", {
      name: "修正简单计时设置",
      exact: true,
    });
    await waitFor(() => expect(document.activeElement).toBe(correct));
    expect(
      (screen.getByRole("radio", { name: "日程计时" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    await user.click(correct);
    const work = screen.getByLabelText("工作时长");
    await waitFor(() => expect(document.activeElement).toBe(work));
    expect((work as HTMLInputElement).value).toBe("");
    await user.type(work, "30");
    await user.click(screen.getByText("日程计时", { exact: true }));
    await user.click(
      screen.getByRole("button", { name: "保存设置", exact: true }),
    );
    await screen.findByText("已保存");
    expect(backend.settings.use_schedule).toBe(true);
    expect(backend.settings.work_minutes).toBe(30);
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
      screen.getByRole("button", { name: "计时与休息", exact: true }),
    );
    await user.click(screen.getByText("日程计时", { exact: true }));
    expect(
      screen.getByRole("region", { name: "日程概览" }).textContent,
    ).toContain("—–24:00");
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
