/** 一份动作指引归属一条提醒或单轮休息；资源描述不依赖文字。 */
export interface Guidance {
  kind: "video";
  source: { type: "builtin"; id: string };
}

export function guidancePicker(value: Guidance | null | undefined, strings: Record<string, string>, onChange: (value: Guidance | null) => void): HTMLElement {
  const label = document.createElement("label");
  label.className = "guidance-picker";
  const caption = document.createElement("span");
  caption.textContent = strings.guidance;
  const select = document.createElement("select");
  for (const [id, name] of [["", strings.noGuidance], ["squat-demo", strings.squatDemo]]) {
    const option = document.createElement("option");
    option.value = id;
    option.textContent = name;
    select.append(option);
  }
  select.value = value?.source.id ?? "";
  select.addEventListener("change", () => onChange(select.value ? { kind: "video", source: { type: "builtin", id: select.value } } : null));
  label.append(caption, select);
  return label;
}

const squatDemoUrl = new URL("./assets/guidance/squat-demo.mp4", import.meta.url).href;

/** 计时刷新只更新文字；只有指引变化和离开休息时才替换媒体实例。 */
export function createGuidanceView(container: HTMLElement) {
  let key = "";
  let video: HTMLVideoElement | null = null;
  let status: HTMLParagraphElement | null = null;
  let retry: HTMLButtonElement | null = null;
  let failure: "load" | "autoplay" | null = null;
  let strings: Record<string, string>;

  function clear() {
    key = "";
    if (video) {
      video.pause();
      video.removeAttribute("src");
      video.load();
      video = null;
    }
    container.replaceChildren();
    container.hidden = true;
    failure = null;
    status = null;
    retry = null;
  }

  function updateLabels() {
    if (video) video.setAttribute("aria-label", strings.videoGuidance);
    if (status && retry) {
      status.textContent = failure === "autoplay" ? strings.videoAutoplay : strings.videoFailed;
      retry.textContent = failure === "autoplay" ? strings.playVideo : strings.retryGuidance;
      status.hidden = retry.hidden = failure === null;
    }
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
    retry = document.createElement("button");
    retry.type = "button";
    retry.className = "guidance-retry";
    retry.addEventListener("click", () => {
      if (failure === "autoplay") {
        void player.play().then(() => { failure = null; updateLabels(); }).catch(() => { failure = "autoplay"; updateLabels(); });
      } else {
        key = "";
        show(guidance, strings);
      }
    });
    player.addEventListener("error", () => {
      if (video !== player) return;
      failure = "load";
      updateLabels();
    });
    player.addEventListener("playing", () => {
      if (video !== player) return;
      failure = null;
      updateLabels();
    });
    container.append(player, status, retry);
    updateLabels();
    if (guidance.kind === "video" && guidance.source.type === "builtin" && guidance.source.id === "squat-demo") {
      player.src = squatDemoUrl;
      void player.play().catch(() => {
        if (video !== player || failure === "load") return;
        failure = "autoplay";
        updateLabels();
      });
    } else {
      failure = "load";
      updateLabels();
    }
  }
  return { show, clear };
}
