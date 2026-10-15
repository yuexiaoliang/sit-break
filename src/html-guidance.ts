import { invoke } from "@tauri-apps/api/core";
import bootstrap from "../src-tauri/guidance-bootstrap.js?raw";
import type { Guidance } from "./guidance";

// 单文件离线内容；外部资源、网络请求、嵌套页面和表单均不加载。
const policy = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";

export function createHtmlGuidanceView(container: HTMLElement) {
  let key = "";
  let frame: HTMLIFrameElement | null = null;
  let status: HTMLParagraphElement | null = null;
  let retry: HTMLButtonElement | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let loading = false;
  let failed = false;
  let strings: Record<string, string>;
  function updateLabels() {
    if (frame) frame.title = strings.htmlGuidance;
    if (status) { status.hidden = !loading && !failed; status.textContent = failed ? strings.htmlFailed : strings.htmlLoading; }
    if (retry) { retry.hidden = !failed; retry.textContent = strings.retryGuidance; }
  }
  function clear() {
    if (!key) return;
    clearTimeout(timer);
    const previous = frame;
    frame = null;
    previous?.remove();
    if (previous) { previous.removeAttribute("srcdoc"); previous.src = "about:blank"; }
    container.replaceChildren();
    container.hidden = true;
    key = "";
    status = retry = null;
    loading = failed = false;
  }
  window.addEventListener("message", (event: MessageEvent<unknown>) => {
    const current = frame;
    if (!current || event.source !== current.contentWindow || event.origin !== "null") return;
    const message = event.data as Record<string, unknown> | null;
    if (!message || message.source !== "sit-break-guidance" || message.session !== current.name || message.type !== "ready" || message.documentUrl !== "about:srcdoc") return;
    clearTimeout(timer);
    loading = failed = false;
    current.dataset.state = "ready";
    updateLabels();
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
    const page = document.createElement("iframe");
    frame = page;
    page.className = "guidance-frame";
    page.name = `sit-break-guidance-${Array.from(bytes, byte => byte.toString(16).padStart(2, "0")).join("")}`;
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
    const fail = () => {
      if (frame !== page) return;
      clearTimeout(timer);
      loading = false; failed = true; page.dataset.state = "failed"; updateLabels();
    };
    timer = setTimeout(fail, 15_000);
    void invoke<string>("read_guidance_html", { guidance, session: page.name }).then(content => {
      if (frame !== page) return;
      page.srcdoc = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="${policy}"><script>${bootstrap}</script>${content}`;
    }).catch(fail);
  }
  return { show, clear };
}
