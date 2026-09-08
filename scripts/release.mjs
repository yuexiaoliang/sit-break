// Release automation: bump version across package.json / tauri.conf.json / Cargo.toml,
// sync Cargo.lock, commit, tag and push — then the Release workflow builds & publishes.
// Usage: npm run release -- patch|minor|major|x.y.z [--dry-run]
import { readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";

const DRY = process.argv.includes("--dry-run");
const args = process.argv.slice(2).filter((a) => a !== "--dry-run");
const bump = args[0];

function run(cmd, opts = {}) {
  console.log(`\x1b[36m> ${cmd}\x1b[0m`);
  execSync(cmd, { stdio: "inherit", ...opts });
}

function fail(msg) {
  console.error(`\x1b[31m✗ ${msg}\x1b[0m`);
  process.exit(1);
}

if (!bump) fail("Usage: npm run release -- patch|minor|major|x.y.z [--dry-run]");

const pkg = JSON.parse(readFileSync("package.json", "utf8"));
const current = pkg.version;
const [cmaj, cmin, cpat] = current.split(".").map(Number);

let version;
if (bump === "patch") version = `${cmaj}.${cmin}.${cpat + 1}`;
else if (bump === "minor") version = `${cmaj}.${cmin + 1}.0`;
else if (bump === "major") version = `${cmaj + 1}.0.0`;
else if (/^\d+\.\d+\.\d+$/.test(bump)) version = bump;
else fail(`Invalid version or bump type: ${bump}`);

if (version === current) fail(`Already at version ${version}`);

// 工作区必须干净
const dirty = execSync("git status --porcelain", { encoding: "utf8" }).trim();
if (dirty && !DRY) fail("Working tree is not clean. Commit or stash first.");

console.log(`\x1b[32m${current} → ${version}\x1b[0m\n`);

// 1. package.json
pkg.version = version;
writeFileSync("package.json", JSON.stringify(pkg, null, 2) + "\n");

// 2. src-tauri/tauri.conf.json
const confPath = "src-tauri/tauri.conf.json";
const conf = readFileSync(confPath, "utf8");
writeFileSync(confPath, conf.replace(/"version":\s*"[^"]+"/, `"version": "${version}"`));

// 3. src-tauri/Cargo.toml（[package] 段的 version）
const cargoPath = "src-tauri/Cargo.toml";
const cargo = readFileSync(cargoPath, "utf8");
writeFileSync(cargoPath, cargo.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`));

console.log(`✔ package.json / tauri.conf.json / Cargo.toml → ${version}`);

if (DRY) {
  console.log("\n\x1b[33m--dry-run: 未提交/推送。去掉 --dry-run 执行正式发布。\x1b[0m");
  process.exit(0);
}

// 4. 同步 Cargo.lock
run("cargo update -p sit-break", { cwd: "src-tauri" });

// 5. commit + tag + push
run("git add -A");
run(`git commit -m "chore(release): v${version}"`);
run(`git tag v${version}`);
run("git push origin main --follow-tags");

console.log(`\n\x1b[32m✓ v${version} 已推送，Release 流水线将自动构建并发布。\x1b[0m`);
