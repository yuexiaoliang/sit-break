import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});
globalThis.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
};
Element.prototype.scrollIntoView = vi.fn();
Object.defineProperty(window, "scrollTo", { value: vi.fn() });
CSS.escape ??= (value: string) => value.replaceAll('"', '\\"');
vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => {});
vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(async () => {});
vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
afterEach(cleanup);
