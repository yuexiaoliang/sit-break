import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import bootstrap from "../src-tauri/guidance-bootstrap.js?raw";
import type { Guidance } from "./guidance";

// 策略先于任何用户 markup 解析；用户自己添加 CSP 只能进一步收紧。
const policy = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob: http: https:; font-src data: http: https:; media-src data: blob: http: https:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";

export function createHtmlGuidanceView(container: HTMLElement, validUrl: (value: string) => boolean, openExternal: (url: string) => Promise<void>) {
  let key = "";
  let frame: HTMLIFrameElement | null = null;
  let status: HTMLParagraphElement | null = null;
  let retry: HTMLButtonElement | null = null;
  let session = "";
  let timer: ReturnType<typeof setTimeout> | undefined;
  let failure = false;
  let loading = false;
  let browserFailed = false;
  let strings: Record<string, string>;

  function updateLabels() {
    if (frame) frame.title = strings.htmlGuidance;
    if (status) {
      status.hidden = !loading && !failure && !browserFailed;
      status.textContent = browserFailed ? strings.openBrowserFailed : failure ? strings.htmlFailed : strings.htmlLoading;
    }
    if (retry) {
      retry.hidden = !failure;
      retry.textContent = strings.retryGuidance;
    }
  }

  function clear() {
    if (!key) return;
    clearTimeout(timer);
    timer = undefined;
    const previous = frame;
    frame = null;
    previous?.remove();
    // 销毁整个 browsing context，停止用户页面中可能存在的媒体、动画和脚本。
    if (previous) { previous.removeAttribute("srcdoc"); previous.src = "about:blank"; }
    container.replaceChildren();
    container.hidden = true;
    key = session = "";
    status = null;
    retry = null;
    loading = failure = browserFailed = false;
  }

  window.addEventListener("message", (event: MessageEvent<unknown>) => {
    const current = frame;
    if (!current || event.source !== current.contentWindow || event.origin !== "null") return;
    const message = event.data as Record<string, unknown> | null;
    if (!message || typeof message !== "object" || message.source !== "sit-break-guidance" || message.session !== session) return;
    // 本地页面只有 about:srcdoc 握手有效，错误文档的初始化脚本不构成加载成功。
    if (typeof message.documentUrl !== "string" || message.documentUrl.split("#", 1)[0] !== "about:srcdoc") return;
    if (message.type === "ready") {
      clearTimeout(timer);
      timer = undefined;
      loading = failure = false;
      current.dataset.state = "ready";
      updateLabels();
    } else if (message.type === "external" && typeof message.url === "string" && validUrl(message.url)) {
      void openExternal(message.url).then(() => {
        if (frame === current) { browserFailed = false; updateLabels(); }
      }).catch(() => {
        if (frame === current) { browserFailed = true; updateLabels(); }
      });
    }
  });
  void listen("guidance-browser-error", () => {
    if (frame) { browserFailed = true; updateLabels(); }
  });

  function show(guidance: Guidance, labels: Record<string, string>) {
    strings = labels;
    const nextKey = JSON.stringify(guidance);
    if (nextKey === key) { updateLabels(); return; }
    clear();
    key = nextKey;
    container.hidden = false;
    loading = true;
    const bytes = crypto.getRandomValues(new Uint8Array(16));
    session = `sit-break-guidance-${Array.from(bytes, byte => byte.toString(16).padStart(2, "0")).join("")}`;
    const page = document.createElement("iframe");
    frame = page;
    page.className = "guidance-frame";
    page.name = session;
    page.setAttribute("sandbox", "allow-scripts");
    page.setAttribute("allow", "camera 'none'; microphone 'none'; geolocation 'none'; autoplay 'none'");
    page.referrerPolicy = "no-referrer";
    page.src = "about:blank";
    page.dataset.state = "loading";
    status = document.createElement("p");
    status.className = "guidance-status";
    status.setAttribute("role", "status");
    retry = document.createElement("button");
    retry.type = "button";
    retry.className = "guidance-retry";
    retry.addEventListener("click", () => { clear(); show(guidance, strings); });
    container.append(page, status, retry);
    updateLabels();
    const failed = () => {
      if (frame !== page) return;
      clearTimeout(timer);
      timer = undefined;
      loading = false;
      failure = true;
      page.dataset.state = "failed";
      updateLabels();
    };
    // read 也验证原生 frame 导航钩子已安装；没有隔离保障时不把内容装进 frame。
    timer = setTimeout(failed, 15_000);
    void invoke<string>("read_guidance_html", { guidance, session }).then((content) => {
      if (frame !== page) return;
      page.srcdoc = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="${policy}"><script>${bootstrap}</script>${content}`;
    }).catch(failed);
  }
  return { show, clear };
}
