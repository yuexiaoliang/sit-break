import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, writeFile, rename } from 'node:fs/promises';
import { join } from 'node:path';

/** 原生 WebView 验收的可控 HTTP 样例；不代理外部站点或改变站点限制。 */
export async function createOnlineGuidanceFixture() {
  const video = await readFile(new URL('../src/assets/guidance/squat-demo.mp4', import.meta.url));
  const requests = [];
  let mode = 'error';
  const server = createServer((request, response) => {
    requests.push({ url: request.url, destination: request.headers['sec-fetch-dest'], range: request.headers.range });
    if (request.url !== '/demo.mp4') { response.writeHead(404).end('Not found'); return; }
    if (mode === 'hang') return;
    if (mode === 'error') { response.writeHead(503, { 'Content-Type': 'text/plain', 'Cache-Control': 'no-store' }).end('Try again'); return; }
    const headers = { 'Content-Type': 'video/mp4', 'Cache-Control': 'no-store', 'Accept-Ranges': 'bytes', 'X-Content-Type-Options': 'nosniff' };
    const match = /^bytes=(\d+)-(\d*)$/.exec(request.headers.range ?? '');
    let start = 0;
    let end = video.length - 1;
    if (match) {
      start = Number(match[1]);
      end = match[2] ? Math.min(Number(match[2]), end) : end;
      if (start > end) { response.writeHead(416, { 'Content-Range': `bytes */${video.length}` }).end(); return; }
      headers['Content-Range'] = `bytes ${start}-${end}/${video.length}`;
    }
    headers['Content-Length'] = String(end - start + 1);
    response.writeHead(match ? 206 : 200, headers);
    response.end(request.method === 'HEAD' ? undefined : video.subarray(start, end + 1));
  });
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  return {
    url: `http://127.0.0.1:${server.address().port}/demo.mp4`, requests,
    setMode(value) { assert.ok(['error', 'hang', 'success'].includes(value)); mode = value; },
    async close() { server.closeAllConnections(); await new Promise((done, reject) => server.close((error) => error ? reject(error) : done())); },
  };
}

/** 调用者提供真实 Tauri exe/CDP 会话及既有生命周期；禁止普通浏览器/mock timer 替代。 */
export async function checkOnlineVideo(context) {
  const { launch, quit, dataDirectory, waitFor, inspectLayout } = context;
  const checks = context.checks ?? context.report.checks;
  let app = context.app;
  const previousSettings = await app.settings.invoke('get_settings');
  const fixture = await createOnlineGuidanceFixture();
  const delay = (ms) => new Promise((done) => setTimeout(done, ms));
  try {
    async function startRest() {
      const settings = await app.settings.invoke('get_settings');
      await app.settings.invoke('save_settings', { settings: { ...settings, language: 'en', idle_reset: false, sound: false, use_schedule: false,
        tips: [{ id: 'native-online-video', text: 'Online video native check', guidance: { kind: 'video', source: { type: 'builtin', id: 'squat-demo' } } }],
        schedules: [{ id: 'native-online-schedule', name: '在线视频原生验收', enabled: true,
          repeat: { kind: 'weekly', weekdays: [1, 2, 3, 4, 5, 6, 7] },
          periods: [{ id: 'native-online-period', start: '00:00', end: '24:00', work_minutes: 1, break_minutes: 1, overrides: [] }] }],
      } });
      await app.settings.invoke('open_settings');
      await waitFor('在线视频选择入口', () => app.settings.evaluate('document.querySelector(".reminder-entry .guidance-picker select")?.options.length >= 3'));
      await app.settings.evaluate(`(() => {
        const picker = document.querySelector('.reminder-entry .guidance-picker');
        const select = picker.querySelector('select'); select.value = 'online-video'; select.dispatchEvent(new Event('change', { bubbles:true }));
        const input = picker.querySelector('input[type=url]'); input.value = ${JSON.stringify(fixture.url)}; input.dispatchEvent(new Event('input', { bubbles:true }));
        document.getElementById('btnSave').click();
      })()`);
      await waitFor('真实设置表单保存视频地址', async () => (await app.settings.invoke('get_settings')).tips[0].guidance?.source.url === fixture.url);
      await waitFor('保存后选择器重载完成', () => app.settings.evaluate(`document.querySelector('.reminder-entry .guidance-picker input[type=url]')?.value === ${JSON.stringify(fixture.url)} && !document.getElementById('btnSave').disabled`));
      const valid = await app.settings.invoke('get_settings');
      await app.settings.evaluate(`(() => {
        const input = document.querySelector('.reminder-entry .guidance-picker input[type=url]'); input.value = 'https://user:secret@example.com/video.mp4'; input.dispatchEvent(new Event('input', { bubbles:true }));
        document.getElementById('btnSave').click();
      })()`);
      assert.equal(await app.settings.evaluate('document.querySelector(".reminder-entry .guidance-picker input[type=url]").checkValidity()'), false);
      assert.deepEqual(await app.settings.invoke('get_settings'), valid, '无效草稿不能替换已保存地址');
      await app.settings.invoke('hide_settings');
      // 在配置完成后启动新日程，避免运行中编辑按约定保留先前的内置指引。
      await app.settings.invoke('save_settings', { settings: { ...valid, use_schedule: true } });
      await app.settings.invoke('reset_timer');
      await quit(app.settings);
      const path = join(dataDirectory, 'progress.json');
      const progress = JSON.parse(await readFile(path, 'utf8'));
      assert.equal(progress.info.mode, 'work');
      assert.equal(progress.info.guidance.source.url, fixture.url);
      progress.info.remaining = 1;
      await writeFile(`${path}.online-check`, JSON.stringify(progress), 'utf8');
      await rename(`${path}.online-check`, path);
      app = await launch();
      context.app = app;
      context.onApp?.(app);
      await waitFor('在线视频所在当前轮提醒', async () => (await app.reminder.invoke('timer_info')).mode === 'remind');
      await app.reminder.evaluate('document.getElementById("btnBreak").click()');
      await waitFor('进入真实视频休息', async () => (await app.reminder.invoke('timer_info')).mode === 'break');
    }

    fixture.setMode('error');
    await startRest();
    await waitFor('503 视频错误与重试', () => app.reminder.evaluate('!document.querySelector(".guidance-retry").hidden && document.querySelector(".guidance-status").textContent.includes("cannot play")'));
    const beforeError = await app.reminder.invoke('timer_info');
    await delay(2200);
    assert.ok((await app.reminder.invoke('timer_info')).remaining < beforeError.remaining, '加载失败不能阻塞休息计时');
    await inspectLayout(app.reminder, 'btnEnd');
    await app.reminder.screenshot('online-video-error');
    fixture.setMode('success');
    await app.reminder.evaluate('document.querySelector(".guidance-retry").click()');
    await waitFor('受控 HTTP 视频重试后实际播放', () => app.reminder.evaluate('(() => { const v=document.querySelector("video"); return v?.readyState>=2 && v.videoWidth>0 && v.currentTime>0; })()'));
    const first = await app.reminder.evaluate('(() => { const v=document.querySelector("video"); window.__nativeVideo=v; return { time:v.currentTime, muted:v.muted, loop:v.loop, controls:v.controls }; })()');
    assert.ok(first.muted && first.loop && first.controls);
    await delay(1000);
    assert.ok(await app.reminder.evaluate(`document.querySelector('video').currentTime !== ${first.time}`), '真实媒体播放进度应前进');
    await app.reminder.evaluate('window.__nativeVideo.pause(); window.__nativeVideo.currentTime = 1');
    const beforePause = await app.reminder.invoke('timer_info');
    const preferences = await app.settings.invoke('get_settings');
    await app.settings.invoke('save_settings', { settings: { ...preferences, sound: !preferences.sound } });
    await delay(2200);
    assert.equal(await app.reminder.evaluate('document.querySelector("video") === window.__nativeVideo && window.__nativeVideo.paused && Math.abs(window.__nativeVideo.currentTime - 1) < 0.2'), true, '普通刷新和保存偏好保留播放器实例及手动进度');
    assert.ok((await app.reminder.invoke('timer_info')).remaining < beforePause.remaining, '视频暂停不能暂停休息倒计时');
    await assert.rejects(app.reminder.invoke('open_guidance_url', { url: 'javascript:alert(1)' }));
    await app.reminder.screenshot('online-video-playing');
    checks.push({ name: '在线视频表单保存、无效地址保留配置、503重试、真实播放及独立计时', first, requests: [...fixture.requests] });
    await app.reminder.evaluate('document.getElementById("btnEnd").click()');
    await waitFor('提前结束释放旧媒体', () => app.reminder.evaluate('!window.__nativeVideo.isConnected && window.__nativeVideo.paused && !window.__nativeVideo.getAttribute("src")'));

    fixture.setMode('hang');
    await startRest();
    const beforeTimeout = await app.reminder.invoke('timer_info');
    await waitFor('网络挂起显示有限等待与恢复操作', () => app.reminder.evaluate('document.querySelector(".guidance-status").textContent.includes("taking too long") && !document.querySelector(".guidance-retry").hidden && !!document.querySelector(".guidance-external")'), 18_000);
    assert.ok((await app.reminder.invoke('timer_info')).remaining < beforeTimeout.remaining, '网络挂起也继续休息计时');
    await inspectLayout(app.reminder, 'btnEnd');
    await app.reminder.screenshot('online-video-timeout');
    fixture.setMode('success');
    await app.reminder.evaluate('document.querySelector(".guidance-retry").click()');
    await waitFor('网络恢复后重试可播', () => app.reminder.evaluate('document.querySelector("video")?.currentTime>0'));
    checks.push({ name: '受控 HTTP 挂起超时、持续计时和网络恢复重试' });
    await app.reminder.evaluate('document.getElementById("btnEnd").click()');
    await waitFor('线上验收结束本轮后恢复原设置', async () => (await app.reminder.invoke('timer_info')).mode !== 'break');
    await app.settings.invoke('save_settings', { settings: previousSettings });
    await app.settings.invoke('reset_timer');
    return app;
  } finally {
    await fixture.close();
  }
}
