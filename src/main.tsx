import { createRoot } from "react-dom/client";
import { initializeRuntime } from "./runtime";
import { AppTheme } from "./theme";

async function start() {
  await initializeRuntime();
  const path = window.location.pathname;
  const { default: App } = path.endsWith("reminder.html")
    ? await import("./ReminderApp")
    : path.endsWith("panel.html")
      ? await import("./PanelApp")
      : path.endsWith("widget.html")
        ? await import("./WidgetApp")
        : await import("./SettingsApp");
  createRoot(document.getElementById("root")!).render(
    <AppTheme>
      <App />
    </AppTheme>,
  );
}
window.addEventListener("contextmenu", (event) => event.preventDefault());
void start().catch((error) => {
  console.error(error);
  const root = document.getElementById("root")!;
  const message = document.createElement("p");
  message.textContent = navigator.language.startsWith("zh")
    ? "暂时无法读取应用状态，请重新打开 Sit Break。"
    : "Could not load the app. Please reopen Sit Break.";
  root.replaceChildren(message);
});
