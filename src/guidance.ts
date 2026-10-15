import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/** 一份动作指引归属一条提醒或单轮休息；资源描述不依赖文字。 */
export interface Guidance {
  kind: "video";
  source: { type: "builtin"; id: string } | { type: "online"; url: string } | { type: "local"; id: string };
}

/** UI 的即时校验与后端再次校验使用相同的 HTTP(S) 约定。 */
function validOnlineUrl(value: string): boolean {
  if (!/^https?:\/\//i.test(value) || /[\s\x00-\x1f\x7f-\x9f\\]/.test(value)) return false;
  if (/%(?![0-9a-f]{2})|%(?:0[0-9a-f]|1[0-9a-f]|7f)/i.test(value)) return false;
  try {
    const url = new URL(value);
    const authority = value.slice(value.indexOf("://") + 3).split(/[/?#]/, 1)[0];
    return ["http:", "https:"].includes(url.protocol) && !!url.hostname && !url.username && !url.password && !authority.includes("@");
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
  const commit = (next: Guidance | null) => { current = next; onChange(next); };
  const group = document.createElement("div");
  group.className = "guidance-picker";
  const label = document.createElement("label");
  const caption = document.createElement("span");
  caption.textContent = strings.guidance;
  const select = document.createElement("select");
  for (const [id, name] of [["", strings.noGuidance], ["squat-demo", strings.squatDemo], ["local-video", strings.localVideo], ["online-video", strings.onlineVideo]]) {
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
    if (valid) commit({ kind: "video", source: { type: "online", url: input.value } });
  };
  function selectCurrent() {
    const source = current?.source;
    select.value = source?.type === "local" ? "local-video" : source?.type === "online" ? "online-video" : source?.type === "builtin" ? source.id : "";
    if (source?.type === "online") input.value = source.url;
  }
  function renderSource() {
    const online = select.value === "online-video";
    const local = select.value === "local-video";
    input.hidden = !online;
    input.disabled = !online;
    input.required = online;
    input.setCustomValidity("");
    hint.hidden = !online && !local;
    hint.textContent = online ? strings.videoUrlHint : strings.localVideoHint;
    choose.hidden = !local;
    if (online) validateUrl();
  }
  async function importVideo() {
    if (importing) return;
    importing = true;
    choose.disabled = true;
    select.setCustomValidity(strings.importingVideo);
    group.dataset.importing = "true";
    status.hidden = false;
    status.textContent = strings.importingVideo;
    try {
      const imported = await invoke<Guidance | null>("import_guidance_video");
      if (imported) {
        await checkImportedVideo(imported);
        commit(imported);
      }
      status.hidden = true;
    } catch (error) {
      status.textContent = error === "unsupported-video" ? strings.unsupportedVideo : strings.importVideoFailed;
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
    if (importing) { select.value = "local-video"; return; }
    status.hidden = true;
    if (select.value === "local-video") {
      renderSource();
      void importVideo();
      return;
    }
    if (select.value !== "online-video") commit(select.value ? { kind: "video", source: { type: "builtin", id: select.value } } : null);
    renderSource();
    if (select.value === "online-video") input.focus();
  });
  choose.addEventListener("click", () => { void importVideo(); });
  input.addEventListener("input", validateUrl);
  label.append(caption, select);
  group.append(label, input, hint, choose, status);
  renderSource();
  return group;
}

const squatDemoUrl = new URL("./assets/guidance/squat-demo.mp4", import.meta.url).href;

/** 计时刷新只更新文案；只有指引变化、主动重试和离开休息时才替换媒体实例。 */
export function createGuidanceView(container: HTMLElement) {
  let key = "";
  let video: HTMLVideoElement | null = null;
  let status: HTMLParagraphElement | null = null;
  let retry: HTMLButtonElement | null = null;
  let external: HTMLButtonElement | null = null;
  let externalStatus: HTMLParagraphElement | null = null;
  let failure: "load" | "autoplay" | "timeout" | null = null;
  let loading = false;
  let openFailed = false;
  let loadTimer: ReturnType<typeof setTimeout> | undefined;
  let strings: Record<string, string>;

  function stopWaiting() {
    clearTimeout(loadTimer);
    loadTimer = undefined;
  }

  function clear() {
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
      player.src = url;
      void player.play().catch((error: unknown) => {
        if (video !== player || failure === "load" || failure === "timeout") return;
        // 格式或网络错误由 error/超时处理；仅浏览器拒绝自动播放时要求手动播放。
        if (error instanceof DOMException && error.name === "NotAllowedError") {
          stopWaiting();
          loading = false;
          failure = "autoplay";
          updateLabels();
        }
      });
    } else {
      failure = "load";
      updateLabels();
    }
  }
  return { show, clear };
}
