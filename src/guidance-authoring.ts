import { invoke } from "@tauri-apps/api/core";
import template from "./assets/guidance/guidance-template.html?raw";
import promptIntro from "./assets/guidance/prompt-intro.txt?raw";

// 复制和手动取得的是同一份完整材料；导出的模板也来自此静态文件。
const prompt = `${promptIntro}\n${template}`;

export function openGuidanceAuthoring(strings: Record<string, string>, trigger: HTMLButtonElement) {
  if (document.getElementById("guidanceAuthoring")) return;
  const dialog = document.createElement("dialog");
  dialog.id = "guidanceAuthoring";
  dialog.className = "guidance-authoring";
  dialog.setAttribute("aria-labelledby", "guidanceAuthoringTitle");
  const element = (tag: string, text: string, className = "") => {
    const node = document.createElement(tag);
    node.textContent = text;
    if (className) node.className = className;
    return node;
  };
  const head = element("header", "", "preview-head");
  const title = element("h2", strings.htmlAuthoring);
  title.id = "guidanceAuthoringTitle";
  const close = document.createElement("button");
  close.type = "button";
  close.className = "close-button";
  close.textContent = "×";
  close.setAttribute("aria-label", strings.closeAuthoring);
  close.addEventListener("click", () => dialog.close());
  head.append(title, close);
  const body = element("div", "", "guidance-authoring-scroll");
  body.append(element("p", strings.authoringIntro));
  const actions = element("div", "", "authoring-actions");
  const button = (id: string, text: string) => {
    const control = document.createElement("button");
    control.type = "button";
    control.id = id;
    control.className = "secondary-button";
    control.textContent = text;
    actions.append(control);
    return control;
  };
  const copy = button("copyGuidancePrompt", strings.copyGuidancePrompt);
  const saveDemo = button("saveGuidanceDemo", strings.saveGuidanceDemo);
  const saveTemplate = button("saveGuidanceTemplate", strings.saveGuidanceTemplate);
  const status = element("p", "", "authoring-status");
  status.id = "guidanceAuthoringStatus";
  status.setAttribute("role", "status");
  body.append(actions, status);

  const section = (caption: string) => {
    const details = document.createElement("details");
    details.append(element("summary", caption));
    body.append(details);
    return details;
  };
  const help = section(strings.authoringLayoutTitle);
  for (const text of [strings.authoringSteps, strings.authoringLayout, strings.authoringResources, strings.authoringScripts, strings.authoringTheme]) help.append(element("p", text));
  const palette = section(strings.authoringPalette);
  palette.append(element("p", strings.authoringPaletteHint));
  const tableScroll = element("div", "", "authoring-table-scroll");
  tableScroll.tabIndex = 0;
  tableScroll.setAttribute("role", "region");
  tableScroll.setAttribute("aria-label", strings.authoringPalette);
  const table = document.createElement("table");
  const header = document.createElement("tr");
  for (const text of [strings.authoringVariable, strings.authoringLight, strings.authoringDark]) header.append(element("th", text));
  table.append(header);
  // 表格直接展示离线模板中的配色，模板与界面说明不会各自保存一份色值。
  const declarations = [...template.matchAll(/(--sb-[a-z-]+):\s*([^;]+);/g)];
  for (const name of new Set(declarations.map(match => match[1]))) {
    const values = declarations.filter(match => match[1] === name).map(match => match[2]);
    const row = document.createElement("tr");
    const label = document.createElement("td");
    label.append(element("code", name));
    row.append(label, element("td", values[0]), element("td", values[1]));
    table.append(row);
  }
  tableScroll.append(table);
  palette.append(tableScroll);
  const fullPrompt = section(strings.authoringFullPrompt);
  fullPrompt.append(element("p", strings.authoringPromptHint));
  const promptText = document.createElement("textarea");
  promptText.id = "guidancePrompt";
  promptText.readOnly = true;
  promptText.spellcheck = false;
  promptText.value = prompt;
  promptText.setAttribute("aria-label", strings.authoringFullPrompt);
  const select = document.createElement("button");
  select.type = "button";
  select.id = "selectGuidancePrompt";
  select.className = "secondary-button";
  select.textContent = strings.selectGuidancePrompt;
  select.addEventListener("click", () => { promptText.focus(); promptText.select(); });
  fullPrompt.append(promptText, select);
  copy.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(prompt);
      status.textContent = strings.guidancePromptCopied;
    } catch {
      fullPrompt.open = true;
      promptText.focus();
      promptText.select();
      status.textContent = strings.guidancePromptCopyFailed;
    }
  });
  const save = async (material: "demo" | "template") => {
    saveDemo.disabled = saveTemplate.disabled = true;
    status.textContent = "";
    try {
      const saved = await invoke<boolean>("save_guidance_authoring_file", { material });
      status.textContent = saved ? strings.guidanceFileSaved : strings.guidanceFileCanceled;
    } catch {
      status.textContent = strings.guidanceFileSaveFailed;
    } finally {
      saveDemo.disabled = saveTemplate.disabled = false;
    }
  };
  saveDemo.addEventListener("click", () => { void save("demo"); });
  saveTemplate.addEventListener("click", () => { void save("template"); });
  dialog.append(head, body);
  dialog.addEventListener("click", event => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) dialog.close();
  });
  dialog.addEventListener("close", () => { dialog.remove(); trigger.focus(); });
  document.body.append(dialog);
  dialog.showModal();
}
