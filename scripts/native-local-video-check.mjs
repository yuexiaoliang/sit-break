import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { preimportedVideo, restartWorkForBreak } from './native-resource-fixture.mjs';

/** Windows 只验证已导入副本；不打开文件选择器，不发送任何原生按键。 */
export async function checkLocalVideo(context) {
  let app = context.app;
  const { report, waitFor, inspectLayout, videoSample } = context;
  report.localVideo = { status: 'running', fixtureMode: 'preimported-copy',
    notCovered: '未操作 Windows 原生文件选择器；不证明文件选择、导入命令或导入 UI。' };
  const previousSettings = await app.settings.invoke('get_settings');
  const bytes = await readFile(new URL('../src/assets/guidance/squat-demo.mp4', import.meta.url));
  const firstGuidance = await preimportedVideo(context.dataDirectory, bytes);
  report.localVideo.firstGuidance = firstGuidance;
  await app.settings.invoke('save_settings', { settings: { ...previousSettings,
    use_schedule: true, language: 'en', sound: false, idle_reset: false,
    tips: [{ id: 'native-local-video', text: 'Local resource fixture', guidance: firstGuidance }],
    schedules: [{ id: 'native-local-schedule', name: '本地副本原生验证', enabled: true,
      repeat: { kind: 'weekly', weekdays: [1, 2, 3, 4, 5, 6, 7] },
      periods: [{ id: 'native-local-period', start: '00:00', end: '24:00', work_minutes: 1, break_minutes: 2, overrides: [] }] }],
  } });
  await app.settings.invoke('reset_timer');
  app = await restartWorkForBreak(context);

  async function decoded(expected) {
    await waitFor('真实 WebView2 解码受控本地副本', () => app.reminder.evaluate(`(() => { const v=document.querySelector('#guidanceContent video'); return v?.readyState >= 2 && v.videoWidth > 0 && v.currentSrc.includes(${JSON.stringify(expected.source.id)}); })()`));
    const info = await app.reminder.invoke('timer_info');
    assert.deepEqual(info.guidance, expected);
    return info;
  }
  await decoded(firstGuidance);
  const remote = await app.reminder.call('Runtime.evaluate', { expression: 'document.querySelector("#guidanceContent video")', returnByValue: false });
  const objectId = remote.result.objectId;
  assert.ok(objectId);
  const sample = () => videoSample(app.reminder, objectId, report.localVideo);
  let first;
  let replacement;
  try {
    first = await sample();
    assert.equal(new URL(first.currentSrc).hostname, 'guidance.localhost', '实际本地媒体必须经过受控资源协议');
    assert.ok(first.muted && first.loop && first.controls && !first.paused);
    await context.delay(1100);
    const playing = await sample();
    const advanced = (playing.currentTime - first.currentTime + first.duration) % first.duration;
    assert.ok(advanced > 0.3 && advanced < 2.5 && playing.same, '真实本地视频持续播放且 tick 保留同一节点');
    const protocol = await app.reminder.evaluate(`(async () => {
      const url=document.querySelector('#guidanceContent video').currentSrc;
      const head=await fetch(url,{method:'HEAD'});
      const range=await fetch(url,{headers:{Range:'bytes=0-31'}});
      return {headStatus:head.status,length:Number(head.headers.get('content-length')),
        rangeStatus:range.status,contentRange:range.headers.get('content-range'),
        mime:range.headers.get('content-type'),received:(await range.arrayBuffer()).byteLength};
    })()`);
    assert.equal(protocol.headStatus, 200);
    assert.equal(protocol.length, bytes.length);
    assert.equal(protocol.rangeStatus, 206);
    assert.equal(protocol.contentRange, `bytes 0-31/${bytes.length}`);
    assert.equal(protocol.received, 32);
    assert.equal(protocol.mime, 'video/mp4');
    await app.reminder.evaluate('document.querySelector("#guidanceContent video").pause(); document.querySelector("#guidanceContent video").currentTime=1.5');
    await waitFor('本地资源协议支持实际 seek', async () => Math.abs((await sample()).currentTime - 1.5) < 0.15);
    await inspectLayout(app.reminder, 'btnEnd');
    await app.reminder.screenshot('local-video-fixture-playing');
    replacement = await preimportedVideo(context.dataDirectory, bytes);
    report.localVideo.replacement = replacement;
    const settings = await app.settings.invoke('get_settings');
    await app.settings.invoke('save_settings', { settings: { ...settings,
      tips: [{ id: 'native-local-video', text: 'Replacement local resource', guidance: replacement }],
    } });
    assert.deepEqual((await app.reminder.invoke('timer_info')).guidance, firstGuidance, '替换未来设置不改变当前本地指引');
    const kept = await sample();
    assert.ok(kept.same && kept.paused && Math.abs(kept.currentTime - 1.5) < 0.15, '保存替换设置保留同一播放器与进度');
    report.checks.push({ name: '已导入本地副本 fixture：真实解码、HEAD/Range、seek 与当前轮替换保留',
      fixtureMode: 'preimported-copy', first, playing, protocol, kept, firstGuidance, replacement });
  } finally {
    try { await app.reminder.call('Runtime.releaseObject', { objectId }); } catch { /* 连接退出时保留原始失败。 */ }
  }

  await context.quit(app.settings);
  app = await context.launch();
  context.app = app;
  context.onApp?.(app);
  await decoded(firstGuidance);
  await app.reminder.screenshot('local-video-fixture-restored');
  await app.reminder.evaluate('document.getElementById("btnEnd").click()');
  await waitFor('结束旧本地指引后下一轮选用新副本', async () => {
    const state = await app.reminder.invoke('timer_info');
    return state.mode === 'work' && state.guidance?.source.id === replacement.source.id;
  });
  app = await restartWorkForBreak(context);
  const nextRound = await decoded(replacement);
  await app.reminder.screenshot('local-video-fixture-replacement');
  await app.reminder.evaluate('window.__nativeLocalVideo = document.querySelector("#guidanceContent video"); document.getElementById("btnEnd").click()');
  await waitFor('结束本地指引清理旧媒体', () => app.reminder.evaluate('!window.__nativeLocalVideo.isConnected && window.__nativeLocalVideo.paused && !window.__nativeLocalVideo.getAttribute("src") && window.__nativeLocalVideo.readyState===0'));
  report.checks.push({ name: '已导入本地副本 fixture：替换后退出恢复旧引用、下一轮采用新引用与结束释放', fixtureMode: 'preimported-copy', firstGuidance, replacement, nextRound });
  await app.settings.invoke('save_settings', { settings: previousSettings });
  await app.settings.invoke('reset_timer');
  report.localVideo.status = 'passed';
  return app;
}
