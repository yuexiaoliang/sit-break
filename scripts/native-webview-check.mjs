import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { createWriteStream } from 'node:fs';
import { access, mkdir, readFile, writeFile, rename } from 'node:fs/promises';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const usage = 'node scripts/native-webview-check.mjs <debug-exe> <output-directory>';
if (process.argv.includes('--help')) {
  console.log(`${usage}\n只运行真实 Windows WebView2；使用 com.sitbreak.nativecheck 独立构建。`);
  process.exit(0);
}
assert.equal(process.platform, 'win32', '本验收需要 Windows 图形会话与真实 WebView2，不使用浏览器替代');
assert.ok(process.argv[2] && process.argv[3], usage);
const executable = resolve(process.argv[2]);
const output = resolve(process.argv[3]);
const identifier = 'com.sitbreak.nativecheck';
const dataDirectory = join(process.env.APPDATA, identifier);
const nativeProbe = fileURLToPath(new URL('./native-window.ps1', import.meta.url));
const runFile = promisify(execFile);
const delay = (milliseconds) => new Promise((done) => setTimeout(done, milliseconds));
const report = { startedAt: new Date().toISOString(), executable, identifier, runs: [], checks: [], video: { status: 'not_started' }, notCovered: ['HTML、导入副本、在线来源与外链场景尚未执行'] };
const connections = [];
let application;
let originalTheme;
let runNumber = 0;
let applicationWasValidated = false;
let launchStartedAt;

async function waitFor(description, check, timeout = 20_000) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const value = await check();
      if (value) return value;
    } catch (error) { lastError = error; }
    if (application && application.exitCode !== null) throw new Error(`应用已退出：${application.exitCode}；${description}`);
    await delay(200);
  }
  throw new Error(`${description}超时${lastError ? `：${lastError.message}` : ''}`);
}

async function powershell(action = 'Snapshot', lightTheme = -1) {
  const { stdout } = await runFile('pwsh', ['-NoProfile', '-NonInteractive', '-File', nativeProbe,
    '-AppProcessId', String(application.pid), '-Action', action, '-LightTheme', String(lightTheme)], { timeout: 20_000 });
  return JSON.parse(stdout.trim());
}

class CDP {
  constructor(socket) {
    this.socket = socket;
    this.nextId = 1;
    this.pending = new Map();
    socket.addEventListener('message', ({ data }) => {
      const message = JSON.parse(data);
      const request = this.pending.get(message.id);
      if (!request) return;
      clearTimeout(request.timer);
      this.pending.delete(message.id);
      if (message.error) request.reject(new Error(JSON.stringify(message.error)));
      else request.resolve(message.result);
    });
    socket.addEventListener('close', () => {
      for (const request of this.pending.values()) {
        clearTimeout(request.timer);
        request.reject(new Error('真实 WebView2 CDP 连接已关闭'));
      }
      this.pending.clear();
    });
  }
  static async connect(url) {
    const socket = new WebSocket(url);
    await new Promise((done, reject) => {
      const timer = setTimeout(() => { socket.close(); reject(new Error('连接 WebView2 超时')); }, 10_000);
      socket.addEventListener('open', () => { clearTimeout(timer); done(); }, { once: true });
      socket.addEventListener('error', () => { clearTimeout(timer); reject(new Error('连接 WebView2 失败')); }, { once: true });
    });
    const connection = new CDP(socket);
    connections.push(connection);
    return connection;
  }
  call(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolveRequest, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`CDP ${method} 超时`)); }, 10_000);
      this.pending.set(id, { resolve: resolveRequest, reject, timer });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }
  async evaluate(expression) {
    const result = await this.call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    assert.ok(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  }
  invoke(command, payload = {}) {
    return this.evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${JSON.stringify(payload)})`);
  }
  async screenshot(name) {
    const { data } = await this.call('Page.captureScreenshot', { format: 'png' });
    await writeFile(join(output, `${name}.png`), Buffer.from(data, 'base64'));
  }
}

async function freePort() {
  const server = createServer();
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  const port = server.address().port;
  await new Promise((done) => server.close(done));
  return port;
}

async function launch() {
  const port = await freePort();
  runNumber += 1;
  applicationWasValidated = false;
  launchStartedAt = Date.now();
  application = spawn(executable, [], { env: { ...process.env,
    SIT_BREAK_NATIVE_CHECK_PORT: String(port),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  }, stdio: ['ignore', 'pipe', 'pipe'] });
  const log = createWriteStream(join(output, `application-${runNumber}.log`));
  application.stdout.pipe(log, { end: false });
  application.stderr.pipe(log, { end: false });
  application.once('close', () => log.end());
  application.once('error', (error) => { report.launchError = error.message; });
  const targets = await waitFor('真实应用 WebView2 调试端口（需 debug 构建及图形会话）', async () => {
    const response = await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(2000) });
    const pages = await response.json();
    return ['settings.html', 'reminder.html'].every((name) => pages.some((page) => page.type === 'page' && new URL(page.url).pathname.endsWith(`/${name}`))) && pages;
  }, 45_000);
  const page = async (name) => {
    const target = targets.find((item) => item.type === 'page' && new URL(item.url).pathname.endsWith(`/${name}.html`));
    const connection = await CDP.connect(target.webSocketDebuggerUrl);
    connection.label = name;
    await connection.call('Runtime.enable');
    await connection.call('Page.enable');
    await waitFor(`${name} 真实 IPC 初始化`, () => connection.evaluate('typeof window.__TAURI_INTERNALS__?.invoke === "function"'));
    return connection;
  };
  const settings = await page('settings');
  const reminder = await page('reminder');
  assert.equal(await settings.invoke('plugin:app|identifier'), identifier, '仅允许专用 com.sitbreak.nativecheck 构建，避免修改用户设置');
  applicationWasValidated = true;
  assert.equal(await settings.invoke('get_platform'), 'windows');
  report.runs.push({ number: runNumber, pid: application.pid, port, targets: targets.map(({ type, url }) => ({ type, url })) });
  return { settings, reminder };
}

async function quit(connection) {
  const exiting = new Promise((done, reject) => {
    const timer = setTimeout(() => reject(new Error('真实应用正常退出超时')), 10_000);
    application.once('exit', () => { clearTimeout(timer); done(); });
  });
  await connection.invoke('quit_app').catch(() => {});
  await exiting;
  for (const connection of connections.splice(0)) connection.socket.close();
}

async function inspectLayout(connection, buttonId, checkText = true) {
  const layout = await connection.evaluate(`(() => {
    const ids = ${JSON.stringify([buttonId, ...(checkText ? [buttonId === 'btnBreak' ? 'tipText' : 'breakTime'] : [])])};
    const rect = (element) => { const r = element.getBoundingClientRect(); return { x:r.x, y:r.y, width:r.width, height:r.height, right:r.right, bottom:r.bottom }; };
    return { width:innerWidth, height:innerHeight, ratio:devicePixelRatio,
      dark:matchMedia('(prefers-color-scheme: dark)').matches,
      background:getComputedStyle(document.documentElement).getPropertyValue('--sb-background').trim(),
      controls:ids.map((id) => { const e = document.getElementById(id); return { id, visible:!!e && !!e.getClientRects().length && getComputedStyle(e).visibility !== 'hidden', ...(e ? rect(e) : {}) }; }) };
  })()`);
  for (const element of layout.controls) {
    assert.ok(element.visible && element.width > 0 && element.height > 0, `${element.id} 必须在真实页面可见`);
    assert.ok(element.x >= 0 && element.y >= 0 && element.right <= layout.width + 1 && element.bottom <= layout.height + 1, `${element.id} 不能被内容挤出视口`);
  }
  const native = await powershell();
  const window = native.windows.find((item) => item.visible && /Reminder|提醒/.test(item.title));
  assert.ok(window, '必须找到真实可见的原生提醒 HWND');
  assert.ok(window.outer.width > 0 && window.outer.height > 0);
  assert.ok(window.outer.width * window.outer.height < window.workArea.width * window.workArea.height / 2, '浮窗不能占据半屏');
  assert.ok(Math.abs(window.client.width - layout.width * layout.ratio) <= 4, 'Win32 客户区与真实 WebView 宽度一致');
  assert.ok(Math.abs(window.client.height - layout.height * layout.ratio) <= 4, 'Win32 客户区与真实 WebView 高度一致');
  return { layout, window };
}

async function videoSample(connection, objectId) {
  const result = await connection.call('Runtime.callFunctionOn', { objectId, returnByValue: true,
    functionDeclaration: `function() {
      const r = this.getBoundingClientRect();
      return { connected:this.isConnected, same:this === document.querySelector('#guidanceContent video'),
        readyState:this.readyState, width:this.videoWidth, height:this.videoHeight, currentTime:this.currentTime,
        duration:this.duration, muted:this.muted, defaultMuted:this.defaultMuted, loop:this.loop, controls:this.controls, paused:this.paused,
        source:this.getAttribute('src'), currentSrc:this.currentSrc,
        rect:{x:r.x,y:r.y,right:r.right,bottom:r.bottom},
        endButtonTop:document.getElementById('btnEnd').getBoundingClientRect().top };
    }` });
  assert.ok(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
  return result.result.value;
}

async function checkBuiltinVideo(app) {
  report.video = { status: 'running', stage: '保存真实内置视频配置' };
  const guidance = { kind: 'video', source: { type: 'builtin', id: 'squat-demo' } };
  const current = await app.settings.invoke('get_settings');
  await app.settings.invoke('save_settings', { settings: { ...current,
    use_schedule: false, work_minutes: 1, break_minutes: 2,
    tips: [{ id: 'native-check-video-only', text: '', guidance }],
  } });
  await app.settings.invoke('reset_timer');
  assert.equal((await app.settings.invoke('timer_info')).mode, 'work');
  // 简单计时走真实一分钟；不注入计时器，也不改写工作状态。
  report.video.stage = '简单计时实际等待一分钟';
  await waitFor('简单计时自然到达视频提醒（一分钟）', async () => (await app.reminder.invoke('timer_info')).mode === 'remind', 80_000);
  const reminded = await app.reminder.invoke('timer_info');
  assert.equal(reminded.tip, '');
  assert.deepEqual(reminded.guidance, guidance);
  await waitFor('仅指引提醒页隐藏空文字且尚未创建播放器', () => app.reminder.evaluate(`!document.getElementById('cardRemind').hidden && document.getElementById('tipText').parentElement.hidden && !document.querySelector('video')`));
  await inspectLayout(app.reminder, 'btnBreak', false);
  await app.reminder.screenshot('04-video-reminder');
  await app.reminder.evaluate('document.getElementById("btnBreak").click()');
  report.video.stage = '原生视频解码与自动播放';
  await waitFor('内置视频取得原生解码画面', () => app.reminder.evaluate(`(() => { const v=document.querySelector('#guidanceContent video'); return !!v && v.readyState >= 2 && v.videoWidth > 0 && v.duration > 0 && Number.isFinite(v.duration); })()`));
  const remote = await app.reminder.call('Runtime.evaluate', { expression: 'document.querySelector("#guidanceContent video")', returnByValue: false });
  const objectId = remote.result.objectId;
  assert.ok(objectId, '必须持有真实 video 元素引用');
  try {
    const initial = await videoSample(app.reminder, objectId);
    report.video.initial = initial;
    assert.ok(initial.muted && initial.defaultMuted && initial.loop && initial.controls && !initial.paused, '内置视频自动静音循环并带播放控制');
    assert.ok(initial.width > 0 && initial.height > 0 && initial.readyState >= 2);
    assert.ok(initial.rect.x >= 0 && initial.rect.y >= 0 && initial.rect.bottom <= initial.endButtonTop, '视频内容不能遮挡结束按钮');
    assert.equal(await app.reminder.evaluate('document.getElementById("stretchText").hidden'), true, '仅指引休息省略文字区域');
    await delay(1100);
    const playing = await videoSample(app.reminder, objectId);
    const advanced = (playing.currentTime - initial.currentTime + initial.duration) % initial.duration;
    assert.ok(advanced > 0.3 && advanced < 2.5, '真实 WebView2 视频时间必须前进');
    assert.ok(playing.same, '计时 tick 不得重建播放器');
    report.video.stage = '真实暂停和拖动进度';
    await app.reminder.evaluate('document.querySelector("#guidanceContent video").pause()');
    const beforePause = await app.reminder.invoke('timer_info');
    const paused = await videoSample(app.reminder, objectId);
    await delay(2200);
    const afterPause = await app.reminder.invoke('timer_info');
    const stillPaused = await videoSample(app.reminder, objectId);
    assert.equal(afterPause.mode, 'break');
    assert.ok(afterPause.remaining < beforePause.remaining && afterPause.remaining >= beforePause.remaining - 4, '暂停视频后真实休息计时仍按实际经过时间推进');
    assert.ok(stillPaused.same && stillPaused.paused && Math.abs(stillPaused.currentTime - paused.currentTime) < 0.1, 'tick 保留同一视频与暂停位置');
    const seekTo = initial.duration / 5;
    await app.reminder.evaluate(`document.querySelector('#guidanceContent video').currentTime = ${seekTo}`);
    await waitFor('视频拖动进度生效', async () => Math.abs((await videoSample(app.reminder, objectId)).currentTime - seekTo) < 0.15);
    const afterSeek = await app.reminder.invoke('timer_info');
    assert.ok(afterSeek.remaining <= afterPause.remaining && afterSeek.remaining >= afterPause.remaining - 3 && afterSeek.remaining > 0, '拖动视频不跳转休息计时');
    report.video.stage = '真实系统主题切换与播放器保留';
    const themes = [];
    for (const [theme, value] of [['light', 1], ['dark', 0]]) {
      await powershell('Theme', value);
      await waitFor(`视频浮窗系统 ${theme} 主题`, () => app.reminder.evaluate(`matchMedia('(prefers-color-scheme: dark)').matches === ${value === 0}`));
      const video = await videoSample(app.reminder, objectId);
      assert.ok(video.same && video.paused && Math.abs(video.currentTime - seekTo) < 0.15, '真实主题切换保留播放器和已选择的暂停进度');
      const state = await app.reminder.invoke('timer_info');
      assert.equal(state.mode, 'break');
      assert.ok(state.remaining <= afterSeek.remaining && state.remaining > 0, '主题切换不重置视频休息倒计时');
      const layout = await inspectLayout(app.reminder, 'btnEnd');
      await app.reminder.screenshot(`05-video-${theme}`);
      themes.push({ theme, video, state, ...layout });
    }
    await app.reminder.evaluate('document.querySelector("#guidanceContent video").play()');
    report.video.stage = '真实循环播放';
    let previousTime = (await videoSample(app.reminder, objectId)).currentTime;
    const looped = await waitFor('原生视频播完后实际循环', async () => {
      const sample = await videoSample(app.reminder, objectId);
      assert.ok(sample.same && !sample.paused, '实际循环仍使用同一播放器');
      if (sample.currentTime < previousTime - 0.5) return sample;
      previousTime = sample.currentTime;
      return false;
    }, 15_000);
    report.video.stage = '结束休息释放播放器';
    await app.reminder.evaluate('document.getElementById("btnEnd").click()');
    const released = await waitFor('结束休息后停止并释放视频', async () => {
      const sample = await videoSample(app.reminder, objectId);
      return !sample.connected && sample.paused && sample.source === null && sample.readyState === 0 && sample;
    });
    assert.equal(await app.reminder.evaluate('document.querySelectorAll("#guidanceContent video").length'), 0);
    assert.equal((await app.reminder.invoke('timer_info')).mode, 'work');
    report.checks.push({ name: '真实内置视频：静音循环、控制、计时独立、主题保留与结束释放', reminded, initial, playing,
      beforePause, afterPause, paused, stillPaused, afterSeek, themes, looped, released });
    report.video.status = 'passed';
  } finally {
    try { await app.reminder.call('Runtime.releaseObject', { objectId }); }
    catch { /* 崩溃或关闭连接时保留前面的实际失败原因。 */ }
  }
}

try {
  await access(executable);
  await mkdir(output, { recursive: true });
  // 专用构建必须使用新目录；绝不删除已有设置来让测试通过。
  await assert.rejects(access(dataDirectory), { code: 'ENOENT' }, `${dataDirectory} 已存在，请使用干净的测试用户/runner`);
  let app = await launch();
  originalTheme = (await powershell()).lightTheme;
  const settings = await app.settings.invoke('get_settings');
  assert.equal(settings.autostart, false, '独立验收构建不应启用开机启动');
  const marker = '原生验收：站立活动\n用户文字应完整保留';
  await app.settings.invoke('save_settings', { settings: { ...settings,
    use_schedule: true, sound: false, idle_reset: false, show_widget: false, language: 'en',
    tips: [{ id: 'native-check-tip', text: marker }],
    schedules: [{ id: 'native-check-schedule', name: '原生验收日程', enabled: true,
      repeat: { kind: 'weekly', weekdays: [1, 2, 3, 4, 5, 6, 7] },
      periods: [{ id: 'native-check-period', start: '00:00', end: '24:00', work_minutes: 1, break_minutes: 1, overrides: [] }] }],
  } });
  await app.settings.invoke('reset_timer');
  const working = await app.settings.invoke('timer_info');
  assert.equal(working.mode, 'work');
  assert.equal(working.round.number, 1);
  assert.equal(working.tip, marker);
  await quit(app.settings);

  // 从真实应用保存的快照缩短等待；保留其余字段，随后仍由真实恢复入口读取。
  const progressPath = join(dataDirectory, 'progress.json');
  const progress = JSON.parse(await readFile(progressPath, 'utf8'));
  assert.equal(progress.info.mode, 'work');
  progress.info.remaining = 1;
  await writeFile(`${progressPath}.check`, JSON.stringify(progress), 'utf8');
  await rename(`${progressPath}.check`, progressPath);
  app = await launch();
  await waitFor('恢复当前轮后触发真实提醒', async () => (await app.reminder.invoke('timer_info')).mode === 'remind');
  await waitFor('真实提醒显示用户文字', () => app.reminder.evaluate(`document.getElementById('tipText')?.textContent === ${JSON.stringify(marker)} && !document.getElementById('cardRemind').hidden`));
  const remind = await inspectLayout(app.reminder, 'btnBreak');
  await app.reminder.screenshot('01-reminder');
  report.checks.push({ name: '保存设置、退出恢复及真实提醒', ...remind });

  await app.reminder.evaluate('document.getElementById("btnBreak").click()');
  await waitFor('实际休息按钮启动休息', async () => (await app.reminder.invoke('timer_info')).mode === 'break');
  const before = await app.reminder.invoke('timer_info');
  await delay(2200);
  const after = await app.reminder.invoke('timer_info');
  assert.ok(after.remaining < before.remaining && after.remaining > 0, '真实休息倒计时必须推进');
  assert.equal(after.tip, marker);
  const resting = await inspectLayout(app.reminder, 'btnEnd');
  report.checks.push({ name: '休息倒计时与始终可见的提前结束入口', before, after, ...resting });

  const themes = [];
  for (const [theme, value] of [['light', 1], ['dark', 0]]) {
    await powershell('Theme', value);
    await waitFor(`真实系统 ${theme} 主题生效`, () => app.reminder.evaluate(`matchMedia('(prefers-color-scheme: dark)').matches === ${value === 0}`));
    const state = await app.reminder.invoke('timer_info');
    assert.equal(state.mode, 'break');
    assert.ok(state.remaining <= after.remaining && state.remaining > 0, '主题切换不得重新启动休息');
    const layout = await inspectLayout(app.reminder, 'btnEnd');
    await app.reminder.screenshot(`02-break-${theme}`);
    themes.push({ theme, state, ...layout });
  }
  assert.notEqual(themes[0].layout.background, themes[1].layout.background, '系统主题切换必须改变浮窗配色');
  report.checks.push({ name: '真实系统亮暗主题切换', themes });
  const saved = await app.reminder.invoke('timer_info');
  await quit(app.reminder);
  app = await launch();
  const restored = await app.reminder.invoke('timer_info');
  assert.equal(restored.mode, 'break');
  assert.deepEqual(restored.round, saved.round);
  assert.equal(restored.tip, marker);
  const startupSeconds = Math.ceil((Date.now() - launchStartedAt) / 1000);
  assert.ok(restored.remaining <= saved.remaining && restored.remaining >= saved.remaining - startupSeconds - 3, '重新打开保留已保存的休息进度');
  await inspectLayout(app.reminder, 'btnEnd');
  await app.reminder.screenshot('03-restored-break');
  await app.reminder.evaluate('document.getElementById("btnEnd").click()');
  await waitFor('实际提前结束按钮完成本轮', async () => {
    const state = await app.reminder.invoke('timer_info');
    return state.mode === 'work' && state.round.number === 2;
  });
  report.checks.push({ name: '真实退出重开及提前结束操作', saved, restored });
  await checkBuiltinVideo(app);
  report.status = 'passed';
} catch (error) {
  report.status = 'failed';
  if (report.video.status === 'running') report.video.status = 'failed';
  report.error = error.stack;
  for (const connection of connections) {
    try { if (connection.socket.readyState === WebSocket.OPEN) await connection.screenshot(`failure-${connection.label}`); }
    catch { /* 原始失败信息保留；崩溃后的 WebView 可能无法再截图。 */ }
  }
  process.exitCode = 1;
} finally {
  if (originalTheme !== undefined) {
    try { await powershell('Theme', originalTheme ?? -1); }
    catch (error) { report.themeRestoreError = error.message; process.exitCode = 1; report.status = 'failed'; }
  }
  if (application?.exitCode === null) {
    const connection = connections.find((item) => item.socket.readyState === WebSocket.OPEN);
    try { if (connection && applicationWasValidated) await quit(connection); }
    catch (error) { report.quitError = error.message; }
    if (application.exitCode === null) application.kill();
  }
  for (const connection of connections) connection.socket.close();
  report.finishedAt = new Date().toISOString();
  await mkdir(output, { recursive: true });
  await writeFile(join(output, 'report.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ status: report.status, checks: report.checks.map((item) => item.name), report: join(output, 'report.json'), error: report.error }, null, 2));
}
