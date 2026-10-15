import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createFrameInspector } from './native-frame-context.mjs';
import { renderCapabilityFixture } from './native-capability-fixture.mjs';
import { preimportedHtml, preimportedVideo, restartWorkForBreak } from './native-resource-fixture.mjs';

/** 真实 WebView2 的已导入 HTML 副本检查；不操作文件选择器或系统浏览器。 */
export async function checkHtml(context) {
  const { report, waitFor, inspectLayout, delay } = context;
  let app = context.app;
  let inspector;
  report.html = { status: 'running', stage: '准备单文件副本', fixtureMode: 'preimported-copy',
    notCovered: '未操作 Windows 文件选择器、导入命令或外部浏览器。' };
  const previousSettings = await app.settings.invoke('get_settings');
  const requests = { image: 0, nested: 0 };
  const server = createServer((request, response) => {
    if (request.url === '/pixel.svg') {
      requests.image += 1;
      response.writeHead(200, { 'Content-Type': 'image/svg+xml', 'Cache-Control': 'no-store' });
      response.end('<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18"><rect width="18" height="18" fill="#1d7a56"/></svg>');
    } else {
      if (request.url === '/nested-attempt') requests.nested += 1;
      response.writeHead(404); response.end();
    }
  });
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  const baseUrl = `http://127.0.0.1:${server.address().port}`;
  const markerDirectory = await mkdtemp(join(tmpdir(), 'sit-break-html-capability-'));
  const markerPath = join(markerDirectory, 'known-file.txt');
  const marker = `native-check-${Date.now()}`;
  await writeFile(markerPath, marker, { flag: 'wx' });
  assert.equal(await readFile(markerPath, 'utf8'), marker, '文件访问攻击使用本平台真实存在的测试文件');
  const video = await preimportedVideo(context.dataDirectory, await readFile(new URL('../src/assets/guidance/squat-demo.mp4', import.meta.url)));
  const settingsFor = (guidance, id) => ({ ...previousSettings, use_schedule: true, language: 'en', sound: false, idle_reset: false,
    // 视频引用既保证重启时保留已知资源，也供能力攻击使用实际可读的协议地址。
    tips: [{ id: 'native-html-retained-video', text: '', guidance: video }],
    schedules: [{ id, name: 'HTML 原生验证', enabled: true,
      repeat: { kind: 'weekly', weekdays: [1, 2, 3, 4, 5, 6, 7] },
      periods: [{ id: `${id}-period`, start: '00:00', end: '24:00', work_minutes: 1, break_minutes: 3,
        overrides: [{ round: 1, guidance }] }] }],
  });
  async function begin(guidance, id) {
    await app.settings.invoke('save_settings', { settings: settingsFor(guidance, id) });
    await app.settings.invoke('reset_timer');
    app = await restartWorkForBreak(context);
    inspector = await createFrameInspector(app.reminder);
    await waitFor('本地 HTML 原生登记与真实 srcdoc 握手', async () => {
      report.html.lastFrame = await inspector.frameInfo();
      return report.html.lastFrame?.url === 'about:srcdoc'
        && await app.reminder.evaluate('document.querySelector("iframe.guidance-frame")?.dataset.state === "ready"');
    });
    assert.deepEqual((await app.reminder.invoke('timer_info')).guidance, guidance);
    const attributes = await app.reminder.evaluate(`(() => { const f=document.querySelector('iframe.guidance-frame');
      return { sandbox:f.getAttribute('sandbox'), referrer:f.referrerPolicy, name:f.name }; })()`);
    assert.equal(attributes.sandbox, 'allow-scripts');
    assert.equal(attributes.referrer, 'no-referrer');
    assert.match(attributes.name, /^sit-break-guidance-[a-f0-9]{32}$/);
    return attributes;
  }
  const sample = () => inspector.evaluate(`(() => ({
    count:document.getElementById('count').textContent,
    scroll:document.getElementById('scroll-content').scrollTop,
    motion:getComputedStyle(document.getElementById('motion')).transform,
    animationTime:document.getElementById('motion').getAnimations()[0]?.currentTime,
    background:getComputedStyle(document.body).backgroundColor,
    dark:matchMedia('(prefers-color-scheme: dark)').matches,
    embedded:document.getElementById('embedded-image').naturalWidth,
    online:document.getElementById('online-image').naturalWidth,
    url:location.href
  }))()`);
  try {
    const source = await readFile(new URL('./native-fixtures/interactive-guidance.html', import.meta.url), 'utf8');
    const interactive = await preimportedHtml(context.dataDirectory, source.replaceAll('__FIXTURE_BASE_URL__', baseUrl));
    const attributes = await begin(interactive, 'native-html-interactive');
    report.html.stage = '交互、图片、动画与计时独立';
    await waitFor('单文件内嵌图和真实在线图片加载', async () => {
      const state = await sample(); return state.embedded === 18 && state.online === 18;
    });
    assert.ok(requests.image > 0, '在线图片必须产生真实服务器请求');
    const frame = await inspector.frameInfo();
    await app.reminder.evaluate('window.__nativeHtmlFrame=document.querySelector("iframe.guidance-frame")');
    await inspector.evaluate('document.getElementById("increment").click(); document.getElementById("scroll-content").scrollTop=80');
    const first = await sample();
    const before = await app.reminder.invoke('timer_info');
    await delay(1200);
    const after = await app.reminder.invoke('timer_info');
    const second = await sample();
    assert.equal(second.count, '1');
    assert.equal(second.scroll, 80);
    assert.ok(second.animationTime > first.animationTime + 500, '真实 CSS 动画持续推进');
    assert.equal(after.mode, 'break');
    assert.ok(after.remaining < before.remaining && after.remaining >= before.remaining - 3, 'iframe 交互和滚动不阻止实际倒计时');
    const themes = [];
    for (const [theme, value] of [['light', 1], ['dark', 0]]) {
      await context.setTheme(value);
      await waitFor(`真实 HTML 跟随系统 ${theme} 主题`, async () => (await sample()).dark === (value === 0));
      const state = await sample();
      assert.equal((await inspector.frameInfo()).frameId, frame.frameId, '系统主题和 tick 不重建 iframe');
      assert.ok(await app.reminder.evaluate('window.__nativeHtmlFrame===document.querySelector("iframe.guidance-frame")'));
      assert.equal(state.count, '1'); assert.equal(state.scroll, 80);
      const layout = await inspectLayout(app.reminder, 'btnEnd');
      const bounds = await app.reminder.evaluate(`(() => { const r=document.querySelector('iframe.guidance-frame').getBoundingClientRect();
        return { top:r.top,bottom:r.bottom,width:r.width,height:r.height,endTop:document.getElementById('btnEnd').getBoundingClientRect().top }; })()`);
      assert.ok(bounds.height > 0 && bounds.bottom <= bounds.endTop, 'HTML 保留宿主结束按钮的独立可见空间');
      await app.reminder.screenshot(`html-interactive-${theme}`);
      themes.push({ theme, state, bounds, ...layout });
    }
    assert.notEqual(themes[0].state.background, themes[1].state.background);
    report.checks.push({ name: '已导入 HTML fixture：真实脚本、CSS 动画、内嵌与在线图、滚动、倒计时和主题保留',
      fixtureMode: 'preimported-copy', interactive, attributes, first, second, before, after, themes });

    report.html.stage = '正常退出恢复与结束销毁';
    await inspector.dispose(); inspector = null;
    await context.quit(app.settings);
    app = await context.launch(); context.app = app; context.onApp?.(app);
    inspector = await createFrameInspector(app.reminder);
    await waitFor('恢复当前 HTML 描述后重新创建真实指引', () => app.reminder.evaluate('document.querySelector("iframe.guidance-frame")?.dataset.state === "ready"'));
    assert.deepEqual((await app.reminder.invoke('timer_info')).guidance, interactive);
    await waitFor('恢复后的单文件脚本实际运行', () => inspector.evaluate('document.documentElement.dataset.ready === "true"'));
    await app.reminder.screenshot('html-interactive-restored');
    await app.reminder.evaluate('window.__nativeHtmlFrame=document.querySelector("iframe.guidance-frame"); document.getElementById("btnEnd").click()');
    await waitFor('结束休息销毁 HTML browsing context', async () => {
      return await app.reminder.evaluate('!document.querySelector("iframe.guidance-frame") && !window.__nativeHtmlFrame.isConnected')
        && !(await inspector.frameInfo());
    });
    report.checks.push({ name: '已导入 HTML fixture：正常退出恢复当前指引与结束销毁 frame', fixtureMode: 'preimported-copy', interactive });
    await inspector.dispose(); inspector = null;

    report.html.stage = '真实能力攻击与宿主状态核对';
    const validAssetUrl = await app.reminder.evaluate('location.href');
    const validGuidanceUrl = `http://guidance.localhost/${video.source.id}`;
    const knownResources = await app.reminder.evaluate(`(async () => {
      const asset=await fetch(${JSON.stringify(validAssetUrl)}), media=await fetch(${JSON.stringify(validGuidanceUrl)},{method:'HEAD'});
      return {assetStatus:asset.status,assetBytes:(await asset.arrayBuffer()).byteLength,mediaStatus:media.status,mediaBytes:Number(media.headers.get('content-length'))};
    })()`);
    assert.equal(knownResources.assetStatus, 200); assert.ok(knownResources.assetBytes > 0);
    assert.equal(knownResources.mediaStatus, 200); assert.ok(knownResources.mediaBytes > 0);
    let attackSettings;
    const attack = await preimportedHtml(context.dataDirectory, (guidance) => {
      attackSettings = { ...settingsFor(guidance, 'native-html-attacks'), sound: true };
      return renderCapabilityFixture({ baseUrl, knownFileUrl: pathToFileURL(markerPath).href,
        validAssetUrl, validGuidanceUrl, attackSettings });
    });
    await begin(attack, 'native-html-attacks');
    const savedSettings = await app.settings.invoke('get_settings');
    // 先以受信宿主公开入口验证同一载荷有效，再恢复基线，排除无效设置造成的假阳性。
    await app.settings.invoke('save_settings', { settings: attackSettings });
    assert.equal((await app.settings.invoke('get_settings')).sound, true);
    await app.settings.invoke('save_settings', { settings: savedSettings });
    const savedTimer = await app.reminder.invoke('timer_info');
    const hostUrl = await app.reminder.evaluate('location.href');
    await inspector.evaluate('document.getElementById("run-attacks").click()');
    await waitFor('用户 HTML 自己完成能力攻击', () => inspector.evaluate('document.getElementById("attack-status").textContent === "done"'), 15_000);
    const results = await inspector.evaluate('JSON.parse(document.getElementById("results").textContent)');
    for (const name of ['parentDom', 'parentNativeCommand']) assert.ok(results[name]?.error, `${name} 不可读取宿主能力`);
    for (const name of ['ownNativeCommand', 'ownReadSettings', 'ownSaveSettings']) {
      assert.ok(results[name] === 'unavailable' || results[name] === 'pending' || results[name]?.error, `${name} 不得执行成功`);
    }
    for (const name of ['fetchIpc', 'fetchFile', 'fetchAsset', 'fetchLocalVideo']) assert.ok(results[name]?.error, `${name} 必须实际阻止读取`);
    assert.equal(results.popupFile, 'blocked'); assert.equal(requests.nested, 0);
    assert.deepEqual(await app.settings.invoke('get_settings'), savedSettings, '合法 save_settings 攻击不得改变设置');
    const timer = await app.reminder.invoke('timer_info');
    assert.equal(timer.mode, 'break');
    for (const key of ['round', 'guidance', 'work_seconds', 'break_seconds', 'paused']) assert.deepEqual(timer[key], savedTimer[key], `攻击不得改变 ${key}`);
    assert.ok(timer.remaining <= savedTimer.remaining && timer.remaining >= savedTimer.remaining - 15);
    assert.equal(await app.reminder.evaluate('location.href'), hostUrl, '用户页面不能顶层导航');
    await inspectLayout(app.reminder, 'btnEnd');
    await app.reminder.screenshot('html-capability-attacks');
    report.checks.push({ name: '已导入 HTML fixture：真实 IPC／设置／文件／应用资源／嵌套 frame／顶层导航攻击被隔离',
      fixtureMode: 'preimported-copy', attack, knownResources, results, savedTimer, timer,
      attackPayloadAcceptedByHost: true, settingsUnchanged: true, requests: { ...requests } });
    await app.reminder.evaluate('document.getElementById("btnEnd").click()');
    await waitFor('结束攻击指引销毁 frame', async () => !(await inspector.frameInfo()));
    await app.settings.invoke('save_settings', { settings: previousSettings });
    await app.settings.invoke('reset_timer');
    report.html.status = 'passed';
    return app;
  } finally {
    try { await inspector?.dispose(); } catch { /* 失败时保留原始原生证据。 */ }
    server.closeAllConnections();
    await new Promise((done) => server.close(done));
    await rm(markerDirectory, { recursive: true, force: true });
  }
}
