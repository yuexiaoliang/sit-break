import { icon } from "./prototype-icons";
// 临时原型控件，保留在原型分支。
export const variants = [
  { key: "A", name: "时段卡片", description: "先设默认值，再展开特殊轮次" },
  { key: "B", name: "一天时间线", description: "先看一天，再点时段编辑" },
  { key: "C", name: "分步设置", description: "一次只决定一件事" },
];

export function mountPrototypeSwitcher(onSwitch: (key: string) => void) {
  if (!import.meta.env.DEV) return;
  const bar = document.createElement("nav");
  bar.className = "prototype-switcher";
  bar.setAttribute("aria-label", "切换原型方案");
  const update = (key: string, notify = true) => {
    const item = variants.find((v) => v.key === key) ?? variants[0];
    const url = new URL(location.href);
    url.searchParams.set("variant", item.key);
    history.replaceState(null, "", url);
    bar.innerHTML = `<button data-cycle="-1" aria-label="上一个方案">${icon("left")}</button>
      <div><small>原型方案 · 左右方向键切换</small><strong>${item.key} · ${item.name}</strong></div>
      <div class="variant-dots">${variants.map((v) => `<button data-variant="${v.key}" aria-label="方案 ${v.key}：${v.name}" aria-pressed="${v.key === item.key}">${v.key}</button>`).join("")}</div>
      <button data-cycle="1" aria-label="下一个方案">${icon("right")}</button>`;
    if (notify) onSwitch(item.key);
  };
  const cycle = (direction: number) => {
    const current = new URLSearchParams(location.search).get("variant") ?? "A";
    update(variants[(variants.findIndex((v) => v.key === current) + direction + variants.length) % variants.length].key);
  };
  bar.addEventListener("click", (event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button");
    if (button?.dataset.variant) update(button.dataset.variant);
    else if (button?.dataset.cycle) cycle(Number(button.dataset.cycle));
  });
  document.addEventListener("keydown", (event) => {
    if (!document.querySelector(".prototype-tools[open]")) return;
    if ((event.target as HTMLElement).closest("input, textarea, select, [contenteditable]")) return;
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      cycle(event.key === "ArrowLeft" ? -1 : 1);
    }
  });
  document.body.append(bar);
  update(new URLSearchParams(location.search).get("variant") ?? "A", false);
}
