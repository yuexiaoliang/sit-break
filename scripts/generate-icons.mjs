import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const tauriDir = join(root, "src-tauri");
const cli = join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
const config = JSON.parse(readFileSync(join(tauriDir, "tauri.conf.json"), "utf8"));
const temporary = mkdtempSync(join(tmpdir(), "sit-break-icons-"));

function generate(source, output, extra = []) {
  const result = spawnSync(process.execPath, [cli, "icon", source, "--output", output, ...extra], {
    cwd: root,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Icon generation failed: ${source}`);
}

try {
  const application = join(temporary, "application");
  const tray = join(temporary, "tray");
  generate(join(tauriDir, "icons", "app-icon.svg"), application);
  generate(join(tauriDir, "icons", "tray-icon.svg"), tray, ["--png", "40"]);

  // Copy configured desktop icons; mobile/store assets stay in the temporary directory.
  for (const icon of config.bundle.icon) {
    copyFileSync(join(application, basename(icon)), join(tauriDir, icon));
  }
  copyFileSync(join(application, "icon.png"), join(tauriDir, "icons", "icon.png"));
  copyFileSync(join(tray, "40x40.png"), join(tauriDir, "icons", "tray-icon.png"));
  console.log("Updated desktop application icons and the monochrome tray template.");
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
