import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { createHtmlGuidanceView } from "./html-guidance";
import { openGuidanceAuthoring } from "./guidance-authoring";

/** 一份动作指引归属一条提醒或单轮休息；资源描述不依赖文字。 */
export interface Guidance {
  kind: "video" | "html";
  source: { type: "builtin"; id: string } | { type: "online"; url: string } | { type: "local"; id: string };
}

/** UI 的即时校验与后端再次校验使用相同的 HTTP(S) 约定。 */
function validOnlineUrl(value: string): boolean {
  if (!/^https?:\/\//i.test(value) || /[\s\x00-\x1f\x7f-\x9f\\]/.test(value)) return false;
  if (/%(?![0-9a-f]{2})|%(?:0[0-9a-f]|1[0-9a-f]|7f)/i.test(value)) return false;
  try {
    const url = new URL(value);
    const authority = value.slice(value.indexOf("://") + 3).split(/[/?#]/, 1)[0];
    return ["http:", "https:"].includes(url.protocol) && !!url.hostname && !url.hostname.toLowerCase().replace(/\.$/, "").endsWith(".localhost") && !url.username && !url.password && !authority.includes("@");
  } catch { return false; }
}

/** 供视频、HTML 的恢复操作共用；后端只接受 HTTP(S)，不会解释 shell。 */
export async function openExternalGuidance(url: string): Promise<void> {
  await invoke("open_guidance_url", { url });
}

/** 导入只在当前 WebView 能读取并解码首帧后生效，失败时保留旧选择。 */
async function checkImportedVideo(guidance: Guidance): Promise<void> {
  if (guidance.source.type !== "local") throw "import-failed";
  const id = guidance.source.id;
  const player = document.createElement("video");
  player.muted = true;
  player.preload = "auto";
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    await new Promise<void>((resolve, reject) => {
      player.addEventListener("loadeddata", () => player.videoWidth > 0 ? resolve() : reject("unsupported-video"), { once: true });
      player.addEventListener("error", () => reject(player.error?.code === 4 ? "unsupported-video" : "import-failed"), { once: true });
      timeout = setTimeout(() => reject("import-failed"), 12_000);
      player.src = convertFileSrc(id, "guidance");
      player.load();
    });
  } finally {
    clearTimeout(timeout);
    player.pause();
    player.removeAttribute("src");
    player.load();
  }
}

export function guidancePicker(value: Guidance | null | undefined, strings: Record<string, string>, onChange: (value: Guidance | null) => void): HTMLElement {
  let current = value ?? null;
  let importing = false;
  let importingSelection = "";
  const commit = (next: Guidance | null) => { current = next; onChange(next); };
  const group = document.createElement("div");
  group.className = "guidance-picker";
  const label = document.createElement("label");
  const caption = document.createElement("span");
  caption.textContent = strings.guidance;
  const select = document.createElement("select");
  for (const [id, name] of [["", strings.noGuidance], ["squat-demo", strings.squatDemo], ["stretch-demo", strings.stretchDemo], ["local-video", strings.localVideo], ["local-html", strings.localHtml], ["online-video", strings.onlineVideo], ["online-html", strings.onlineHtml]]) {
    const option = document.createElement("option");
    option.value = id;
    option.textContent = name;
    select.append(option);
  }
  const input = document.createElement("input");
  input.type = "url";
  input.className = "guidance-url";
  input.placeholder = "https://example.com/video.mp4";
  input.setAttribute("aria-label", strings.videoUrl);
  input.autocomplete = "off";
  input.spellcheck = false;
  const hint = document.createElement("p");
  hint.className = "guidance-hint";
  const choose = document.createElement("button");
  choose.type = "button";
  choose.className = "text-button guidance-import";
  choose.textContent = strings.chooseLocalVideo;
  const status = document.createElement("p");
  status.className = "guidance-hint guidance-import-status";
  status.setAttribute("role", "status");
  status.hidden = true;
  const validateUrl = () => {
    const valid = validOnlineUrl(input.value);
    input.setCustomValidity(valid ? "" : strings.guidanceUrlInvalid);
    if (valid) commit({ kind: select.value === "online-html" ? "html" : "video", source: { type: "online", url: input.value } });
  };
  function selectCurrent() {
    const source = current?.source;
    select.value = source?.type === "local" ? `local-${current!.kind}` : source?.type === "online" ? `online-${current!.kind}` : source?.type === "builtin" ? source.id : "";
    if (source?.type === "online") input.value = source.url;
  }
  function renderSource() {
    const online = select.value.startsWith("online-");
    const html = select.value.endsWith("html");
    const local = select.value.startsWith("local-");
    input.hidden = !online;
    input.disabled = !online;
    input.required = online;
    input.setCustomValidity("");
    hint.hidden = !online && !local;
    input.placeholder = html ? "https://example.com/guide.html" : "https://example.com/video.mp4";
    input.setAttribute("aria-label", html ? strings.htmlUrl : strings.videoUrl);
    hint.textContent = online ? html ? strings.htmlUrlHint : strings.videoUrlHint : html ? strings.localHtmlHint : strings.localVideoHint;
    choose.hidden = !local;
    choose.textContent = html ? strings.chooseLocalHtml : strings.chooseLocalVideo;
    if (online) validateUrl();
  }
  async function importFile() {
    if (importing) return;
    const html = select.value === "local-html";
    importing = true;
    importingSelection = select.value;
    choose.disabled = true;
    select.setCustomValidity(html ? strings.importingHtml : strings.importingVideo);
    group.dataset.importing = "true";
    status.hidden = false;
    status.textContent = html ? strings.importingHtml : strings.importingVideo;
    try {
      const imported = await invoke<Guidance | null>(html ? "import_guidance_html" : "import_guidance_video");
      if (imported) {
        if (!html) await checkImportedVideo(imported);
        commit(imported);
      }
      status.hidden = true;
    } catch (error) {
      status.textContent = html ? error === "unsupported-html" ? strings.unsupportedHtml : strings.importHtmlFailed : error === "unsupported-video" ? strings.unsupportedVideo : strings.importVideoFailed;
    } finally {
      importing = false;
      choose.disabled = false;
      select.setCustomValidity("");
      delete group.dataset.importing;
      selectCurrent();
      renderSource();
    }
  }
  selectCurrent();
  select.addEventListener("change", () => {
    if (importing) { select.value = importingSelection; return; }
    status.hidden = true;
    if (select.value === "local-video" || select.value === "local-html") {
      renderSource();
      void importFile();
      return;
    }
    if (!select.value.startsWith("online-")) commit(select.value ? { kind: select.value === "stretch-demo" ? "html" : "video", source: { type: "builtin", id: select.value } } : null);
    renderSource();
    if (select.value.startsWith("online-")) input.focus();
  });
  choose.addEventListener("click", () => { void importFile(); });
  input.addEventListener("input", validateUrl);
  label.append(caption, select);
  const authoring = document.createElement("button");
  authoring.type = "button";
  authoring.className = "text-button guidance-authoring-trigger";
  authoring.textContent = strings.htmlAuthoring;
  authoring.setAttribute("aria-haspopup", "dialog");
  authoring.addEventListener("click", () => openGuidanceAuthoring(strings, authoring));
  group.append(label, input, hint, choose, status, authoring);
  renderSource();
  return group;
}

const squatDemoUrl = new URL("./assets/guidance/squat-demo.mp4", import.meta.url).href;

/** 计时刷新只更新文案；只有指引变化、主动重试和离开休息时才替换媒体实例。 */
export function createGuidanceView(container: HTMLElement) {
  const htmlView = createHtmlGuidanceView(container, validOnlineUrl, openExternalGuidance);
  let key = "";
  let video: HTMLVideoElement | null = null;
  let status: HTMLParagraphElement | null = null;
  let retry: HTMLButtonElement | null = null;
  let external: HTMLButtonElement | null = null;
  let externalStatus: HTMLParagraphElement | null = null;
  let failure: "load" | "autoplay" | "timeout" | null = null;
  let loading = false;
  let openFailed = false;
  let builtinFetch: AbortController | null = null;
  let builtinObjectUrl: string | null = null;
  let loadTimer: ReturnType<typeof setTimeout> | undefined;
  let strings: Record<string, string>;

  function stopWaiting() {
    clearTimeout(loadTimer);
    loadTimer = undefined;
  }

  function clear() {
    htmlView.clear();
    key = "";
    stopWaiting();
    builtinFetch?.abort();
    builtinFetch = null;
    if (video) {
      const previous = video;
      video = null;
      previous.pause();
      previous.removeAttribute("src");
      previous.load();
    }
    if (builtinObjectUrl) URL.revokeObjectURL(builtinObjectUrl);
    builtinObjectUrl = null;
    container.replaceChildren();
    container.hidden = true;
    failure = null;
    loading = false;
    openFailed = false;
    status = null;
    retry = null;
    external = null;
    externalStatus = null;
  }

  function updateLabels() {
    if (video) video.setAttribute("aria-label", strings.videoGuidance);
    if (status && retry) {
      status.textContent = failure === "autoplay" ? strings.videoAutoplay : failure === "timeout" ? strings.videoTimeout : failure ? strings.videoFailed : strings.videoLoading;
      status.hidden = failure === null && !loading;
      retry.textContent = failure === "autoplay" ? strings.playVideo : strings.retryGuidance;
      retry.hidden = failure === null;
    }
    if (external) external.textContent = strings.openInBrowser;
    if (externalStatus) {
      externalStatus.textContent = strings.openBrowserFailed;
      externalStatus.hidden = !openFailed;
    }
  }

  function waitForPlayback(player: HTMLVideoElement) {
    if (loadTimer !== undefined) return;
    loading = true;
    updateLabels();
    loadTimer = setTimeout(() => {
      if (video !== player) return;
      loading = false;
      failure = "timeout";
      player.pause();
      updateLabels();
    }, 12_000);
  }

  function show(guidance: Guidance | null | undefined, labels: Record<string, string>) {
    strings = labels;
    if (!guidance) { clear(); return; }
    if (guidance.kind === "html") {
      if (key) clear();
      htmlView.show(guidance, labels);
      return;
    }
    htmlView.clear();
    const nextKey = JSON.stringify(guidance);
    if (key === nextKey) { updateLabels(); return; }
    clear();
    key = nextKey;
    container.hidden = false;
    const player = document.createElement("video");
    video = player;
    player.autoplay = true;
    player.muted = true;
    player.defaultMuted = true;
    player.loop = true;
    player.controls = true;
    player.playsInline = true;
    player.preload = "metadata";
    status = document.createElement("p");
    status.className = "guidance-status";
    status.setAttribute("role", "status");
    const actions = document.createElement("div");
    actions.className = "guidance-actions";
    retry = document.createElement("button");
    retry.type = "button";
    retry.className = "guidance-retry";
    retry.addEventListener("click", () => {
      if (failure === "autoplay") {
        void player.play().then(() => {
          if (video !== player) return;
          failure = null;
          updateLabels();
        }).catch(() => {
          if (video !== player) return;
          failure = "autoplay";
          updateLabels();
        });
      } else {
        key = "";
        show(guidance, strings);
      }
    });
    actions.append(retry);
    externalStatus = document.createElement("p");
    externalStatus.className = "guidance-status";
    externalStatus.setAttribute("role", "status");
    if (guidance.source.type === "online") {
      const url = guidance.source.url;
      external = document.createElement("button");
      external.type = "button";
      external.className = "guidance-external guidance-retry";
      const button = external;
      button.addEventListener("click", async () => {
        button.disabled = true;
        try {
          await openExternalGuidance(url);
          if (video === player) openFailed = false;
        } catch {
          if (video === player) openFailed = true;
        } finally {
          button.disabled = false;
          if (video === player) updateLabels();
        }
      });
      actions.append(button);
    }
    player.addEventListener("error", () => {
      if (video !== player) return;
      stopWaiting();
      loading = false;
      failure = "load";
      updateLabels();
    });
    for (const event of ["loadeddata", "playing"]) player.addEventListener(event, () => {
      if (video !== player) return;
      stopWaiting();
      loading = false;
      failure = null;
      updateLabels();
    });
    // 只监控实际缺少数据的等待；用户主动暂停不会被当作网络超时。
    for (const event of ["waiting", "stalled"]) player.addEventListener(event, () => {
      if (video === player && !player.paused) waitForPlayback(player);
    });
    container.append(player, status, actions, externalStatus);
    const source = guidance.source;
    const url = source.type === "builtin" && source.id === "squat-demo" ? squatDemoUrl : source.type === "online" && validOnlineUrl(source.url) ? source.url : source.type === "local" && /^local-[a-f0-9]{32}$/i.test(source.id) ? convertFileSrc(source.id, "guidance") : null;
    if (url) {
      waitForPlayback(player);
      const play = () => void player.play().catch((error: unknown) => {
        if (video !== player || failure === "load" || failure === "timeout") return;
        // 格式或网络错误由 error/超时处理；仅浏览器拒绝自动播放时要求手动播放。
        if (error instanceof DOMException && error.name === "NotAllowedError") {
          stopWaiting();
          loading = false;
          failure = "autoplay";
          updateLabels();
        }
      });
      if (source.type === "builtin") {
        // 打包的 tauri 资源响应不提供字节 Range；小型内置视频转为 Blob 后可在 WebView2 拖动。
        const controller = new AbortController();
        builtinFetch = controller;
        void fetch(url, { signal: controller.signal }).then(async (response) => {
          if (!response.ok) throw new Error("Builtin video unavailable");
          const blob = await response.blob();
          if (video !== player) return;
          builtinObjectUrl = URL.createObjectURL(blob);
          player.src = builtinObjectUrl;
          play();
        }).catch(() => {
          if (video !== player) return;
          stopWaiting();
          loading = false;
          failure = "load";
          updateLabels();
        }).finally(() => {
          if (builtinFetch === controller) builtinFetch = null;
        });
      } else {
        player.src = url;
        play();
      }
    } else {
      failure = "load";
      updateLabels();
    }
  }
  return { show, clear };
}
