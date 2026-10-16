import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { MantineProvider } from "@mantine/core";
import { GuidanceVideo } from "../src/Guidance";
import { STR } from "../src/i18n";
vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (id: string) => "/" + id,
  invoke: vi.fn(),
}));
describe("视频播放生命周期", () => {
  it("读秒、语言和明暗切换保留节点、播放进度与暂停状态，结束时释放资源", async () => {
    const load = vi
      .spyOn(HTMLMediaElement.prototype, "load")
      .mockImplementation(() => {});
    vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(
      async () => {},
    );
    const pause = vi
      .spyOn(HTMLMediaElement.prototype, "pause")
      .mockImplementation(() => {});
    const guidance = {
      kind: "video" as const,
      source: {
        type: "local" as const,
        id: "local-00000000000000000000000000000001",
      },
    };
    const { container, rerender, unmount } = render(
      <MantineProvider forceColorScheme="light">
        <GuidanceVideo guidance={guidance} S={STR.zh} />
      </MantineProvider>,
    );
    const video = container.querySelector("video")!;
    video.currentTime = 1.5;
    const loads = load.mock.calls.length;
    rerender(
      <MantineProvider forceColorScheme="dark">
        <GuidanceVideo guidance={{ ...guidance }} S={STR.en} />
      </MantineProvider>,
    );
    expect(container.querySelector("video")).toBe(video);
    expect(video.currentTime).toBe(1.5);
    expect(load.mock.calls.length).toBe(loads);
    expect(pause).not.toHaveBeenCalled();
    await act(async () => video.dispatchEvent(new Event("loadeddata")));
    expect(
      screen.getByRole("slider", { name: STR.en.videoProgress }),
    ).toBeTruthy();
    expect(video.controls).toBe(false);
    expect(video.muted && video.loop).toBe(true);
    unmount();
    expect(pause).toHaveBeenCalled();
    expect(video.getAttribute("src")).toBeNull();
  });
});
