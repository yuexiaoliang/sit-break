import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { createHtmlGuidanceView } from "./html-guidance";
import { openGuidanceAuthoring } from "./guidance-authoring";

/** 指引只保存应用导入副本的引用；不接收网址、文件路径或 HTML 源码。 */
export interface Guidance {
  kind: "video" | "html";
  source: { type: "local"; id: string };
}

async function checkImportedVideo(guidance: Guidance): Promise<void> {
  const player = document.createElement("video");
  player.muted = true;
  player.preload = "auto";
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    await new Promise<void>((resolve, reject) => {
      player.addEventListener("loadeddata", () => player.videoWidth > 0 ? resolve() : reject("unsupported-video"), { once: true });
      player.addEventListener("error", () => reject(player.error?.code === 4 ? "unsupported-video" : "import-failed"), { once: true });
      timeout = setTimeout(() => reject("import-failed"), 12_000);
      player.src = convertFileSrc(guidance.source.id, "guidance");
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
  const group = document.createElement("div");
  group.className = "guidance-picker";
  const label = document.createElement("label");
  const caption = document.createElement("span");
  caption.textContent = strings.guidance;
  const select = document.createElement("select");
  for (const [id, name] of [["", strings.noGuidance], ["local-video", strings.localVideo], ["local-html", strings.localHtml]]) {
    const option = document.createElement("option");
    option.value = id;
    option.textContent = name;
    select.append(option);
  }
  const hint = document.createElement("p");
  hint.className = "guidance-hint";
  const choose = document.createElement("button");
  choose.type = "button";
  choose.className = "text-button guidance-import";
  const status = document.createElement("p");
  status.className = "guidance-hint guidance-import-status";
  status.setAttribute("role", "status");
  status.hidden = true;
  function render() {
    const html = select.value === "local-html";
    hint.hidden = choose.hidden = !select.value;
    hint.textContent = html ? strings.localHtmlHint : strings.localVideoHint;
    choose.textContent = html ? strings.chooseLocalHtml : strings.chooseLocalVideo;
  }
  async function importFile() {
    if (importing) return;
    const html = select.value === "local-html";
    importing = true;
    importingSelection = select.value;
    choose.disabled = true;
    select.setCustomValidity(html ? strings.importingHtml : strings.importingVideo);
    status.hidden = false;
    status.textContent = html ? strings.importingHtml : strings.importingVideo;
    try {
      const imported = await invoke<Guidance | null>(html ? "import_guidance_html" : "import_guidance_video");
      if (imported) {
        if (!html) await checkImportedVideo(imported);
        current = imported;
        onChange(imported);
      }
      status.hidden = true;
    } catch (error) {
      status.textContent = html ? error === "unsupported-html" ? strings.unsupportedHtml : strings.importHtmlFailed : error === "unsupported-video" ? strings.unsupportedVideo : strings.importVideoFailed;
    } finally {
      importing = false;
      choose.disabled = false;
      select.setCustomValidity("");
      select.value = current ? `local-${current.kind}` : "";
      render();
    }
  }
  select.value = current ? `local-${current.kind}` : "";
  select.addEventListener("change", () => {
    if (importing) { select.value = importingSelection; return; }
    status.hidden = true;
    render();
    if (select.value) void importFile();
    else { current = null; onChange(null); }
  });
  choose.addEventListener("click", () => { void importFile(); });
  label.append(caption, select);
  const authoring = document.createElement("button");
  authoring.type = "button";
  authoring.className = "text-button guidance-authoring-trigger";
  authoring.textContent = strings.htmlAuthoring;
  authoring.setAttribute("aria-haspopup", "dialog");
  authoring.addEventListener("click", () => openGuidanceAuthoring(strings, authoring));
  group.append(label, hint, choose, status, authoring);
  render();
  return group;
}

/** tick 仅更新文案；切换、重试或结束休息时才替换播放器。 */
export function createGuidanceView(container: HTMLElement) {
  const htmlView = createHtmlGuidanceView(container);
  let key = "";
  let video: HTMLVideoElement | null = null;
  let status: HTMLParagraphElement | null = null;
  let retry: HTMLButtonElement | null = null;
  let failure: "load" | "autoplay" | "timeout" | null = null;
  let loading = false;
  let loadTimer: ReturnType<typeof setTimeout> | undefined;
  let strings: Record<string, string>;
  function stopWaiting() { clearTimeout(loadTimer); loadTimer = undefined; }
  function clear() {
    htmlView.clear();
    key = "";
    stopWaiting();
    if (video) {
      const previous = video;
      video = null;
      previous.pause();
      previous.removeAttribute("src");
      previous.load();
    }
    container.replaceChildren();
    container.hidden = true;
    failure = null;
    loading = false;
    status = null;
    retry = null;
  }
  function updateLabels() {
    if (video) video.setAttribute("aria-label", strings.videoGuidance);
    if (status && retry) {
      status.textContent = failure === "autoplay" ? strings.videoAutoplay : failure === "timeout" ? strings.videoTimeout : failure ? strings.videoFailed : strings.videoLoading;
      status.hidden = failure === null && !loading;
      retry.textContent = failure === "autoplay" ? strings.playVideo : strings.retryGuidance;
      retry.hidden = failure === null;
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
    player.autoplay = player.muted = player.defaultMuted = player.loop = player.controls = player.playsInline = true;
    player.preload = "metadata";
    status = document.createElement("p");
    status.className = "guidance-status";
    status.setAttribute("role", "status");
    retry = document.createElement("button");
    retry.type = "button";
    retry.className = "guidance-retry";
    retry.addEventListener("click", () => {
      if (failure === "autoplay") {
        failure = null;
        waitForPlayback(player);
        void player.play().catch(() => { if (video === player) { stopWaiting(); loading = false; failure = "autoplay"; updateLabels(); } });
      } else { key = ""; show(guidance, strings); }
    });
    player.addEventListener("error", () => {
      if (video !== player) return;
      stopWaiting(); loading = false; failure = "load"; updateLabels();
    });
    for (const event of ["loadeddata", "playing"]) player.addEventListener(event, () => {
      if (video !== player) return;
      stopWaiting(); loading = false; failure = null; updateLabels();
    });
    for (const event of ["waiting", "stalled"]) player.addEventListener(event, () => {
      if (video === player && !player.paused) waitForPlayback(player);
    });
    container.append(player, status, retry);
    if (guidance.source.type === "local" && /^local-[a-f0-9]{32}$/i.test(guidance.source.id)) {
      waitForPlayback(player);
      player.src = convertFileSrc(guidance.source.id, "guidance");
      void player.play().catch((error: unknown) => {
        if (video !== player || failure === "load" || failure === "timeout") return;
        if (error instanceof DOMException && error.name === "NotAllowedError") {
          stopWaiting(); loading = false; failure = "autoplay"; updateLabels();
        }
      });
    } else { failure = "load"; updateLabels(); }
  }
  return { show, clear };
}
