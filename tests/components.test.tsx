import { beforeEach, describe, expect, it, vi } from "vitest";

const startDragging = vi.hoisted(() => vi.fn(async () => {}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging }),
}));

import { dragWindow } from "../src/components";

function mouseDown(interactive = false) {
  return {
    button: 0,
    preventDefault: vi.fn(),
    target: {
      closest: vi.fn(() => (interactive ? {} : null)),
    },
  } as any;
}

describe("无边框窗口拖动", () => {
  beforeEach(() => startDragging.mockClear());

  it("从空白区域开始拖动窗口", () => {
    const event = mouseDown();
    dragWindow(event);
    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(startDragging).toHaveBeenCalledOnce();
  });

  it("不拦截按钮、视频和滑块等交互控件", () => {
    const event = mouseDown(true);
    dragWindow(event);
    expect(event.preventDefault).not.toHaveBeenCalled();
    expect(startDragging).not.toHaveBeenCalled();
  });
});
