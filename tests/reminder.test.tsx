import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ReminderApp from "../src/ReminderApp";
import { STR } from "../src/i18n";
import { initializeRuntime } from "../src/runtime";
import { AppTheme } from "../src/theme";
import type { TimerState } from "../src/timer-state";
import { settings, timer } from "./fixtures";

const { invoke, startDragging } = vi.hoisted(() => ({
  invoke: vi.fn(),
  startDragging: vi.fn(async () => {}),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (id: string) => "/" + id,
  invoke,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: async () => () => {},
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging }),
}));

async function showReminder(state: Partial<TimerState>) {
  invoke.mockClear();
  startDragging.mockClear();
  invoke.mockImplementation(async (command: string) => {
    if (command === "get_settings") return settings();
    if (command === "timer_info") return { ...timer, ...state };
    if (command === "get_platform") return "macos";
  });
  await initializeRuntime();
  return render(
    <AppTheme>
      <ReminderApp />
    </AppTheme>,
  );
}

describe("休息浮窗拖动", () => {
  it("能从提醒标题拖动，提醒小字和再等等按钮保留鼠标操作", async () => {
    const tip = "喝一杯水";
    await showReminder({ mode: "remind", tip });
    const user = userEvent.setup();

    fireEvent.mouseDown(
      screen.getByRole("heading", { name: STR.zh.timeToMove }),
      { button: 0 },
    );
    expect(startDragging).toHaveBeenCalledOnce();

    expect(fireEvent.mouseDown(screen.getByText(tip), { button: 0 })).toBe(true);
    await user.click(screen.getByRole("button", { name: STR.zh.snooze }));
    expect(invoke).toHaveBeenCalledWith("postpone");
    expect(startDragging).toHaveBeenCalledOnce();
  });

  it("能从倒计时顶栏拖动，点击休息内容后焦点留在滚动区", async () => {
    const tip = "跟着下面的说明伸展";
    const { container } = await showReminder({
      mode: "break",
      remaining: 90,
      tip,
    });

    fireEvent.mouseDown(screen.getByRole("timer"), { button: 0 });
    expect(startDragging).toHaveBeenCalledOnce();

    await userEvent.setup().click(screen.getByText(tip));
    expect(document.activeElement).toBe(container.querySelector("#stretchText"));
    expect(startDragging).toHaveBeenCalledOnce();
  });

  it("按住视频进度轨道只调整进度", async () => {
    vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => {});
    vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
    const { container } = await showReminder({
      mode: "break",
      remaining: 90,
      guidance: {
        kind: "video",
        source: { type: "local", id: "local-00000000000000000000000000000001" },
      },
    });
    const video = container.querySelector("video")!;
    Object.defineProperty(video, "duration", { value: 30 });
    fireEvent(video, new Event("loadedmetadata"));
    fireEvent(video, new Event("loadeddata"));

    // Mantine 把 slider 角色放在手柄上，轨道需要单独操作。
    const track = container.querySelector(".video-seek .mantine-Slider-track")!;
    vi.spyOn(track.parentElement!, "getBoundingClientRect").mockReturnValue(
      new DOMRect(0, 0, 100, 12),
    );
    fireEvent.mouseDown(track, { button: 0, clientX: 25, clientY: 6 });
    expect(startDragging).not.toHaveBeenCalled();
    fireEvent.mouseMove(document, { clientX: 75, clientY: 6 });
    await waitFor(() =>
      expect(screen.getByRole("slider").getAttribute("aria-valuenow")).toBe(
        "22.5",
      ),
    );
    fireEvent.mouseUp(track, { button: 0, clientX: 75, clientY: 6 });
    await waitFor(() => expect(video.currentTime).toBe(22.5));
  });
});
