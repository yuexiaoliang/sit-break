import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createFrameInspector } from './native-frame-context.mjs';
import { renderCapabilityFixture } from './native-capability-fixture.mjs';
import { preimportedVideo, restartWorkForBreak } from './native-resource-fixture.mjs';

/** 在线页直接由真实 WebView 请求，保留这些响应头；不下载重写站点来绕过内嵌限制。 */
export async function createOnlineHtmlFixture() {
  let mode = 'success';
  let attackHtml = '';
  const requests = [];
  const page = `<!doctype html><html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Online movement guidance</title><link rel="stylesheet" href="/page.css">
<h1>Online guidance</h1><button id="increment">Record a stretch</button> <output id="count">0</output>
<div id="motion"></div><a id="jump" href="#last-step">Last step</a>
<div id="scroll-content"><p>Keep the native break controls visible.</p><p>Scroll within this page.</p><p>Shared HTML keeps its own colors.</p><p id="last-step">Last step</p></div>
<a id="external-link" href="/external-marker?from=link">External reference</a>
<button id="script-navigation">Script navigation</button><button id="popup-navigation">Open reference</button>
<script>document.getElementById('increment').onclick=()=>{const n=document.getElementById('count');n.textContent=String(Number(n.textContent)+1)};
document.getElementById('script-navigation').onclick=()=>{location.href='/external-marker?from=script'};
document.getElementById('popup-navigation').onclick=()=>window.open('/external-marker?from=popup');
document.documentElement.dataset.ready='true';</script></html>`;
  const server = createServer((request, response) => {
    const path = new URL(request.url, 'http://fixture.invalid').pathname;
    requests.push({ url: request.url, destination: request.headers['sec-fetch-dest'], origin: request.headers.origin });
    if (path === '/page.css') {
      response.writeHead(200, { 'Content-Type': 'text/css', 'Cache-Control': 'no-store' }).end(`
*{box-sizing:border-box}body{margin:0;padding:12px;font:13px/1.5 system-ui;background:#f7edff;color:#38204f;color-scheme:light}
h1{margin:0 0 8px;font-size:17px}button{padding:6px;font:inherit}a{color:#5d237f}#scroll-content{height:110px;overflow:auto;border:1px solid #9267ad;padding:8px;margin:10px 0}
#scroll-content p{margin:0 0 35px}#motion{width:10px;height:10px;margin:8px;background:#7f3daa;animation:move 1s linear infinite alternate}@keyframes move{to{transform:translateX(25px)}}`);
      return;
    }
    if (path === '/attacks.html') { response.writeHead(200, { 'Content-Type': 'text/html', 'Cache-Control': 'no-store' }).end(attackHtml); return; }
    if (path === '/external-marker') { response.writeHead(200, { 'Content-Type': 'text/html' }).end('<!doctype html><title>Guidance external reference</title><h1>Opened external reference</h1>'); return; }
    if (!['/guide.html', '/redirected.html'].includes(path)) { response.writeHead(404).end('Not found'); return; }
    if (path === '/guide.html') {
      if (mode === 'hang') return;
      if (mode === 'disconnect') { response.destroy(); return; }
      if (mode === 'error') { response.writeHead(503, { 'Content-Type': 'text/plain', 'Cache-Control': 'no-store' }).end('Unavailable'); return; }
      if (mode === 'redirect') { response.writeHead(302, { Location: '/redirected.html?shared=1', 'Cache-Control': 'no-store' }).end(); return; }
    }
    const headers = { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' };
    if (mode === 'deny-xfo') headers['X-Frame-Options'] = 'DENY';
    if (mode === 'deny-csp') headers['Content-Security-Policy'] = "frame-ancestors 'none'";
    response.writeHead(200, headers).end(page);
  });
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  const baseUrl = `http://127.0.0.1:${server.address().port}`;
  return {
    baseUrl, url: `${baseUrl}/guide.html?shared=1`, attackUrl: `${baseUrl}/attacks.html`, requests,
    setMode(value) { assert.ok(['success', 'hang', 'disconnect', 'error', 'redirect', 'deny-xfo', 'deny-csp'].includes(value)); mode = value; },
    setAttackHtml(value) { attackHtml = value; },
    async close() { server.closeAllConnections(); await new Promise((done) => server.close(done)); },
  };
}

/** 由主脚本提供真实 Tauri exe/CDP；网页内上下文仅从外部调试连接读取。 */
export async function checkOnlineHtml(context) {
  const { report, waitFor, inspectLayout, delay } = context;
  let app = context.app;
  let inspector;
  const previousSettings = await app.settings.invoke('get_settings');
  const fixture = await createOnlineHtmlFixture();
  const markerDirectory = await mkdtemp(join(tmpdir(), 'sit-break-online-html-'));
  const markerPath = join(markerDirectory, 'known-file.txt');
  const marker = `native-online-${Date.now()}`;
  await writeFile(markerPath, marker, { flag: 'wx' });
  assert.equal(await readFile(markerPath, 'utf8'), marker);
  const retainedVideo = await preimportedVideo(context.dataDirectory, await readFile(new URL('../src/assets/guidance/squat-demo.mp4', import.meta.url)));
  report.onlineHtml = { status: 'running', stage: '在线地址真实设置入口', notCovered: '系统浏览器实际打开由单独原生交互验收；本模块不操作外部浏览器。' };
  const guideFor = (url) => ({ kind: 'html', source: { type: 'online', url } });
  const settingsFor = (url, id) => ({ ...previousSettings, use_schedule: true, language: 'en', sound: false, idle_reset: false,
    tips: [{ id: 'native-online-html-entry', text: '', guidance: guideFor(url) }, { id: 'native-online-retained-video', text: '', guidance: retainedVideo }],
    schedules: [{ id, name: '在线网页原生验证', enabled: true,
      repeat: { kind: 'weekly', weekdays: [1, 2, 3, 4, 5, 6, 7] },
      periods: [{ id: `${id}-period`, start: '00:00', end: '24:00', work_minutes: 1, break_minutes: 3,
        overrides: [{ round: 1, guidance: guideFor(url) }] }] }],
  });
  async function begin(url, id) {
    await app.settings.invoke('save_settings', { settings: settingsFor(url, id) });
    await app.settings.invoke('reset_timer');
    app = await restartWorkForBreak(context);
    inspector = await createFrameInspector(app.reminder);
    assert.deepEqual((await app.reminder.invoke('timer_info')).guidance, guideFor(url));
    assert.equal((await app.reminder.invoke('timer_info')).tip, '');
    assert.ok(await app.reminder.evaluate('document.getElementById("stretchText").hidden'), '仅在线网页不补通用休息文字');
    assert.equal(await app.reminder.evaluate('document.querySelector("iframe.guidance-frame").getAttribute("sandbox")'), 'allow-scripts');
  }
  async function end() {
    await app.reminder.evaluate('window.__onlineOldFrame=document.querySelector("iframe.guidance-frame");document.getElementById("btnEnd").click()');
    await waitFor('结束在线网页销毁旧 browsing context', () => app.reminder.evaluate('!document.querySelector("iframe.guidance-frame") && !window.__onlineOldFrame.isConnected'));
    await inspector?.dispose(); inspector = null;
    await waitFor('结束后再配置下一轮在线网页', async () => (await app.reminder.invoke('timer_info')).mode !== 'break');
  }
  const ready = () => app.reminder.evaluate('document.querySelector("iframe.guidance-frame")?.dataset.state === "ready"');
  const sample = () => inspector.evaluate(`(() => ({ count:document.getElementById('count').textContent,
    scroll:document.getElementById('scroll-content').scrollTop, background:getComputedStyle(document.body).backgroundColor,
    animation:document.getElementById('motion').getAnimations()[0]?.currentTime, url:location.href }))()`);
  try {
    // 从实际提醒表单选择网页，空文字、地址校验及保存重载沿同一产品入口。
    await app.settings.invoke('save_settings', { settings: { ...settingsFor(fixture.url, 'native-online-form'), use_schedule: false } });
    await app.settings.invoke('open_settings');
    await waitFor('在线网页表单重载', () => app.settings.evaluate('document.querySelector(".reminder-entry .guidance-picker select")?.value === "online-html"'));
    const configured = await app.settings.invoke('get_settings');
    await app.settings.evaluate(`(() => { const input=document.querySelector('.reminder-entry .guidance-url');
      input.value='http://tauri.localhost/reminder.html';input.dispatchEvent(new Event('input',{bubbles:true}));document.getElementById('btnSave').click(); })()`);
    assert.equal(await app.settings.evaluate('document.querySelector(".reminder-entry .guidance-url").checkValidity()'), false);
    assert.deepEqual(await app.settings.invoke('get_settings'), configured);
    await app.settings.evaluate(`(() => { const row=document.querySelector('.reminder-entry'); const select=row.querySelector('select');
      select.value='';select.dispatchEvent(new Event('change',{bubbles:true}));select.value='online-html';select.dispatchEvent(new Event('change',{bubbles:true}));
      const input=row.querySelector('.guidance-url');input.value=${JSON.stringify(fixture.url)};input.dispatchEvent(new Event('input',{bubbles:true}));
      row.querySelector('textarea').value='';row.querySelector('textarea').dispatchEvent(new Event('input',{bubbles:true}));document.getElementById('btnSave').click(); })()`);
    await waitFor('网页表单真实保存空文字和原地址', async () => { const saved=await app.settings.invoke('get_settings'); return saved.tips[0].text === '' && saved.tips[0].guidance?.kind === 'html' && saved.tips[0].guidance.source.url === fixture.url; });
    await app.settings.invoke('hide_settings');

    fixture.setMode('deny-xfo');
    await begin(fixture.url, 'native-online-denied');
    report.onlineHtml.stage = '拒绝内嵌、诚实反馈与真实重试';
    const before = await app.reminder.invoke('timer_info');
    await waitFor('X-Frame-Options原样生效后给出恢复操作', () => app.reminder.evaluate('document.querySelector("iframe.guidance-frame")?.dataset.state === "failed" && !document.querySelector(".guidance-retry").hidden && !!document.querySelector(".guidance-external")'), 19_000);
    assert.match(await app.reminder.evaluate('document.querySelector(".guidance-status").textContent'), /may not be able to display/);
    assert.ok((await app.reminder.invoke('timer_info')).remaining < before.remaining);
    await inspectLayout(app.reminder, 'btnEnd');
    await app.reminder.screenshot('online-html-denied');
    fixture.setMode('success');
    await app.reminder.evaluate('window.__failedFrame=document.querySelector("iframe.guidance-frame");document.querySelector(".guidance-retry").click()');
    await waitFor('真实网络恢复后重新加载网页并握手', ready);
    assert.equal(await app.reminder.evaluate('window.__failedFrame.isConnected'), false);
    assert.equal(await app.reminder.evaluate('!document.querySelector(".guidance-retry").hidden && !document.querySelector(".guidance-external").hidden'), true, '可运行的站点错误页也保留重试和浏览器恢复入口');
    const frame = await inspector.frameInfo();
    await app.reminder.evaluate('window.__nativeOnlineFrame=document.querySelector("iframe.guidance-frame")');
    await inspector.evaluate('document.getElementById("increment").click();document.getElementById("scroll-content").scrollTop=75');
    const first = await sample();
    assert.equal(first.background, 'rgb(247, 237, 255)', '远程网页的外部样式原样呈现');
    const timerBefore = await app.reminder.invoke('timer_info');
    await delay(1200);
    const second = await sample();
    assert.equal(second.count, '1'); assert.equal(second.scroll, 75); assert.ok(second.animation > first.animation + 500);
    assert.ok((await app.reminder.invoke('timer_info')).remaining < timerBefore.remaining);
    if (context.setTheme) for (const [theme, value] of [['light', 1], ['dark', 0]]) {
      await context.setTheme(value); await delay(500);
      assert.equal((await inspector.frameInfo()).frameId, frame.frameId);
      assert.ok(await app.reminder.evaluate('window.__nativeOnlineFrame===document.querySelector("iframe.guidance-frame")'));
      const themed = await sample(); assert.equal(themed.count, '1'); assert.equal(themed.scroll, 75);
      assert.equal(themed.background, first.background, '系统主题不强改远程页面配色');
      await app.reminder.screenshot(`online-html-${theme}`);
    }
    await inspector.evaluate('document.getElementById("jump").click()');
    assert.ok(await inspector.evaluate('document.getElementById("scroll-content").scrollTop>75'));
    await inspectLayout(app.reminder, 'btnEnd');
    report.checks.push({ name: '在线网页真实表单、无文字、XFO不绕过、恢复重试、外部CSS、滚动脚本动画和主题保留', first, second });
    await end();

    for (const mode of ['hang', 'disconnect', 'deny-csp', 'error']) {
      fixture.setMode(mode); report.onlineHtml.stage = `网络与拒绝展示：${mode}`;
      await begin(fixture.url, `native-online-${mode}`);
      const initial = await app.reminder.invoke('timer_info');
      await waitFor(`${mode}保留有限等待与浏览器恢复`, () => app.reminder.evaluate('document.querySelector("iframe.guidance-frame")?.dataset.state === "failed" && !document.querySelector(".guidance-retry").hidden && !!document.querySelector(".guidance-external")'), 19_000);
      assert.ok((await app.reminder.invoke('timer_info')).remaining < initial.remaining);
      await inspectLayout(app.reminder, 'btnEnd');
      await app.reminder.screenshot(`online-html-${mode}`);
      fixture.setMode('success');
      await app.reminder.evaluate('document.querySelector(".guidance-retry").click()'); await waitFor(`${mode}恢复后重试真正运行HTML`, ready);
      assert.equal(await inspector.evaluate('document.documentElement.dataset.ready'), 'true');
      report.checks.push({ name: `在线网页${mode}：诚实失败反馈、继续计时与恢复重试` });
      await end();
    }

    report.onlineHtml.stage = 'HTTP重定向与原生能力隔离';
    fixture.setMode('redirect');
    await begin(fixture.url, 'native-online-redirect'); await waitFor('同一次HTTP加载的重定向可就绪', ready);
    assert.match((await inspector.frameInfo()).url, /\/redirected\.html\?shared=1$/);
    report.checks.push({ name: '在线网页只在所选初始加载中跟随HTTP重定向', frame: await inspector.frameInfo() });
    await end();
    fixture.setMode('success');
    const attackSettings = { ...settingsFor(fixture.attackUrl, 'native-online-attacks'), sound: true };
    const hostUrl = await app.reminder.evaluate('location.href');
    const nestedAppUrl = new URL('settings.html', hostUrl).href;
    const localVideoUrl = `http://guidance.localhost/${retainedVideo.source.id}`;
    const knownResources = await app.reminder.evaluate(`(async()=>{const a=await fetch(${JSON.stringify(hostUrl)});const v=await fetch(${JSON.stringify(localVideoUrl)},{method:'HEAD'});
      const settings=await fetch(${JSON.stringify(nestedAppUrl)});return{asset:a.status,video:v.status,settings:settings.status,settingsForm:(await settings.text()).includes('id="settingsForm"')};})()`);
    assert.deepEqual(knownResources, { asset: 200, video: 200, settings: 200, settingsForm: true });
    fixture.setAttackHtml(await renderCapabilityFixture({ baseUrl: fixture.baseUrl, knownFileUrl: pathToFileURL(markerPath).href,
      validAssetUrl: hostUrl, validGuidanceUrl: localVideoUrl, nestedAppUrl, attackSettings }));
    await begin(fixture.attackUrl, 'native-online-attacks'); await waitFor('在线攻击页面真实就绪', ready);
    const baseline = await app.settings.invoke('get_settings');
    await app.settings.invoke('save_settings', { settings: attackSettings });
    assert.equal((await app.settings.invoke('get_settings')).sound, true);
    await app.settings.invoke('save_settings', { settings: baseline });
    const timer = await app.reminder.invoke('timer_info');
    await inspector.evaluate('document.getElementById("run-attacks").click()');
    await waitFor('在线页面完成实际能力攻击', () => inspector.evaluate('document.getElementById("attack-status").textContent === "done"'), 15_000);
    const results = await inspector.evaluate('JSON.parse(document.getElementById("results").textContent)');
    const afterSettings = await app.settings.invoke('get_settings');
    const afterAttack = await app.reminder.invoke('timer_info');
    report.onlineHtml.capability = { results, settingsUnchanged: JSON.stringify(afterSettings) === JSON.stringify(baseline),
      beforeTimer: timer, afterTimer: afterAttack, hostUrl: await app.reminder.evaluate('location.href') };
    for (const name of ['parentDom', 'parentNativeCommand']) assert.ok(results[name]?.error, `${name}必须隔离`);
    for (const name of ['ownNativeCommand', 'ownReadSettings', 'ownSaveSettings']) assert.ok(results[name] === 'unavailable' || results[name] === 'pending' || results[name]?.error, `${name}不可执行`);
    for (const name of ['fetchIpc', 'fetchFile', 'fetchAsset', 'fetchLocalVideo']) assert.ok(results[name]?.error, `${name}不可读取`);
    assert.deepEqual(afterSettings, baseline);
    assert.equal(afterAttack.mode, 'break');
    assert.ok(afterAttack.remaining <= timer.remaining && afterAttack.remaining >= timer.remaining - 15, '攻击期间只按正常墙钟计时');
    for (const key of ['round', 'guidance', 'work_seconds', 'break_seconds', 'paused']) assert.deepEqual(afterAttack[key], timer[key]);
    assert.equal(await app.reminder.evaluate('location.href'), hostUrl);
    await app.reminder.screenshot('online-html-capability-attacks');
    report.checks.push({ name: '在线opaque网页实际IPC、设置、文件和受控资源攻击被拒绝', knownResources, results, settingsUnchanged: true });

    report.onlineHtml.stage = '无 name 应用子帧的实际 RPC 隔离';
    const nested = { requestedUrl: nestedAppUrl, payloadAcceptedByHost: true, commands: {} };
    report.onlineHtml.nestedApp = nested;
    let previousUrl;
    let stableSince = Date.now();
    await waitFor('定位无 name 子帧及其实际默认脚本上下文', async () => {
      nested.owner = await inspector.evaluate(`(() => { const f=document.getElementById('nested-app-attempt');
        return f && {src:f.src,name:f.name,hasName:f.hasAttribute('name'),event:f.dataset.event}; })()`);
      nested.frame = await inspector.elementFrameInfo('document.getElementById("nested-app-attempt")');
      if (nested.frame.url !== previousUrl) { previousUrl = nested.frame.url; stableSince = Date.now(); }
      if (!nested.frame.context) return false;
      // 应用模块可能因真实 IPC 未决而阻止 load；不把这个事件当作文档或权限证据。
      return Date.now() - stableSince >= 1000;
    }, 12_000);
    assert.equal(nested.owner.hasName, false); assert.equal(nested.owner.name, '');
    const selector = { frameId: nested.frame.frameId };
    nested.document = await inspector.evaluate(`({url:location.href,title:document.title,
      readyState:document.readyState,settingsForm:!!document.getElementById('settingsForm')})`, selector);
    nested.appDocumentLoaded = nested.document.url === nestedAppUrl && nested.document.settingsForm;
    const nestedTimer = await app.reminder.invoke('timer_info');
    for (const [command, payload] of [['get_settings', {}], ['save_settings', { settings: attackSettings }], ['end_break', {}]]) {
      try {
        nested.commands[command] = await inspector.evaluate(`(async()=>{
          if(typeof window.__TAURI_INTERNALS__?.invoke!=='function')return 'unavailable';
          try{return await Promise.race([window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)},${JSON.stringify(payload)}).then(()=>'resolved'),
            new Promise(done=>setTimeout(()=>done('pending'),800))]);}catch(error){return {error:error?.name||typeof error};}
        })()`, selector);
      } catch (error) { nested.commands[command] = { debugError: error.message }; }
    }
    const nestedAfterSettings = await app.settings.invoke('get_settings');
    nested.afterTimer = await app.reminder.invoke('timer_info');
    nested.hostUrl = await app.reminder.evaluate('location.href');
    nested.settingsUnchanged = JSON.stringify(nestedAfterSettings) === JSON.stringify(baseline);
    for (const [command, outcome] of Object.entries(nested.commands)) {
      assert.ok(outcome === 'unavailable' || outcome === 'pending' || outcome?.error, `无name子帧 ${command} 不得执行成功`);
    }
    assert.deepEqual(nestedAfterSettings, baseline, '嵌套应用页不能通过真实 save_settings 修改宿主');
    assert.equal(nested.afterTimer.mode, 'break');
    assert.ok(nested.afterTimer.remaining <= nestedTimer.remaining && nested.afterTimer.remaining >= nestedTimer.remaining - 7);
    for (const key of ['round', 'guidance', 'work_seconds', 'break_seconds', 'paused']) assert.deepEqual(nested.afterTimer[key], nestedTimer[key]);
    assert.equal(nested.hostUrl, hostUrl);
    await app.reminder.screenshot('online-html-nested-app-capability');
    report.checks.push({ name: '在线网页创建无name应用子帧：实际文档加载与真实RPC权限分别核对', ...nested, beforeTimer: nestedTimer });
    await end();
    await app.settings.invoke('save_settings', { settings: previousSettings }); await app.settings.invoke('reset_timer');
    report.onlineHtml.requests = [...fixture.requests]; report.onlineHtml.status = 'passed';
    return app;
  } finally {
    try { await inspector?.dispose(); } catch { /* 失败时保留首个原生问题。 */ }
    await fixture.close(); await rm(markerDirectory, { recursive: true, force: true });
  }
}
