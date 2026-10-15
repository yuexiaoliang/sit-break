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
  let external: HTMLButtonElement | null = null;
  let session = "";
  let onlineUrl = "";
  let documentUrl = "";
  let timer: ReturnType<typeof setTimeout> | undefined;
  let failure = false;
  let loading = false;
  let browserFailed = false;
  let strings: Record<string, string>;

  function updateLabels() {
    if (frame) frame.title = strings.htmlGuidance;
    if (status) {
      status.hidden = !loading && !failure && !browserFailed;
      status.textContent = browserFailed ? strings.openBrowserFailed : failure ? onlineUrl ? strings.onlineHtmlFailed : strings.htmlFailed : strings.htmlLoading;
    }
    if (retry) {
      // 网页自身的 HTTP 错误页也可能运行脚本；在线来源始终保留可主动重新加载的恢复入口。
      retry.hidden = loading || (!failure && !onlineUrl);
      retry.textContent = strings.retryGuidance;
    }
    if (external) external.textContent = strings.openInBrowser;
  }

  function clear() {
    if (!key) return;
    clearTimeout(timer);
    timer = undefined;
    const previous = frame;
    const previousSession = session;
    frame = null;
    previous?.remove();
    // 销毁整个 browsing context，停止用户页面中可能存在的媒体、动画和脚本。
    if (previous) { previous.removeAttribute("srcdoc"); previous.src = "about:blank"; }
    container.replaceChildren();
    container.hidden = true;
    key = session = onlineUrl = documentUrl = "";
    status = null;
    retry = null;
    external = null;
    loading = failure = browserFailed = false;
    void invoke("release_guidance_html", { session: previousSession }).catch(() => {});
  }

  window.addEventListener("message", (event: MessageEvent<unknown>) => {
    const current = frame;
    if (!current || event.source !== current.contentWindow || event.origin !== "null") return;
    const message = event.data as Record<string, unknown> | null;
    if (!message || typeof message !== "object" || message.source !== "sit-break-guidance" || message.session !== session) return;
    // 错误文档的初始化脚本不构成加载成功。在线重定向的最终地址还须由原生导航授权确认。
    if (typeof message.documentUrl !== "string") return;
    const reportedUrl = message.documentUrl;
    if (onlineUrl ? !validUrl(reportedUrl) : reportedUrl.split("#", 1)[0] !== "about:srcdoc") return;
    const ready = () => {
      if (frame !== current) return;
      clearTimeout(timer);
      timer = undefined;
      loading = failure = false;
      documentUrl = reportedUrl.split("#", 1)[0];
      current.dataset.state = "ready";
      updateLabels();
    };
    if (message.type === "ready") {
      if (onlineUrl) {
        void invoke("confirm_online_guidance", { session, documentUrl: reportedUrl }).then(ready).catch(() => {});
      } else ready();
    } else if (message.type === "external" && (!onlineUrl || documentUrl === reportedUrl.split("#", 1)[0]) && typeof message.url === "string" && validUrl(message.url)) {
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
    onlineUrl = guidance.source.type === "online" ? guidance.source.url : "";
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
    const actions = document.createElement("div");
    actions.className = "guidance-actions";
    actions.append(retry);
    if (onlineUrl) {
      const url = onlineUrl;
      external = document.createElement("button");
      external.type = "button";
      external.className = "guidance-external guidance-retry";
      const button = external;
      button.addEventListener("click", async () => {
        button.disabled = true;
        try { await openExternal(url); if (frame === page) browserFailed = false; }
        catch { if (frame === page) browserFailed = true; }
        finally { button.disabled = false; if (frame === page) updateLabels(); }
      });
      actions.append(button);
    }
    container.append(page, status, actions);
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
    // 不用 iframe load 事件宣称成功；网络错误、拒绝内嵌或无响应均保留明确且诚实的恢复入口。
    timer = setTimeout(failed, 15_000);
    if (onlineUrl) {
      const url = onlineUrl;
      // 先等原生 frame hook，随后只授权此次所选地址；站点响应头与样式按原样保留。
      void invoke("prepare_online_guidance", { guidance, session }).then(() => {
        if (frame === page) page.src = url;
        else void invoke("release_guidance_html", { session: page.name }).catch(() => {});
      }).catch(failed);
    } else {
      // read 同样验证原生隔离接线；用户内容永不作为应用协议页面执行。
      void invoke<string>("read_guidance_html", { guidance, session }).then((content) => {
        if (frame !== page) return;
        page.srcdoc = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="${policy}"><script>${bootstrap}</script>${content}`;
      }).catch(failed);
    }
  }
  return { show, clear };
}
