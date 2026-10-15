import assert from 'node:assert/strict';
import { createFrameInspector } from './native-frame-context.mjs';
import { restartWorkForBreak } from './native-resource-fixture.mjs';

/** 随包内置 HTML 与作者材料；不点击原生保存对话框或写入系统剪贴板。 */
export async function checkBuiltinHtml(context) {
  let app = context.app;
  let inspector;
  const { report, waitFor, delay, inspectLayout } = context;
  report.builtinHtml = { status: 'running', stage: '真实表单选择和作者材料',
    notCovered: '作者材料保存对话框、实际导出文件与系统剪贴板写入未操作。' };
  const previous = await app.settings.invoke('get_settings');
  await app.settings.invoke('save_settings', { settings: { ...previous, use_schedule: false,
    work_minutes: 1, break_minutes: 2, language: 'en', sound: false, idle_reset: false,
    tips: [{ id: 'native-builtin-html', text: '', guidance: { kind: 'video', source: { type: 'builtin', id: 'squat-demo' } } }],
    schedules: [{ id: 'native-builtin-html-schedule', name: '内置 HTML 原生验证', enabled: true,
      repeat: { kind: 'weekly', weekdays: [1, 2, 3, 4, 5, 6, 7] },
      periods: [{ id: 'native-builtin-html-period', start: '00:00', end: '24:00', work_minutes: 1, break_minutes: 2, overrides: [] }] }],
  } });
  await app.settings.invoke('open_settings');
  const row = '.reminder-entry[data-reminder-id=native-builtin-html]';
  await waitFor('真实设置载入内置 HTML 选择入口', () => app.settings.evaluate(`document.querySelector('${row} .guidance-picker select')?.value==='squat-demo'`));
  await app.settings.evaluate(`(() => { const select=document.querySelector('${row} .guidance-picker select');
    select.value='stretch-demo'; select.dispatchEvent(new Event('change',{bubbles:true})); document.getElementById('btnSave').click(); })()`);
  await waitFor('真实表单保存内置 HTML 选择', async () => (await app.settings.invoke('get_settings')).tips[0].guidance?.source.id === 'stretch-demo');
  await waitFor('保存后作者材料按钮可用', () => app.settings.evaluate(`document.querySelector('${row} .guidance-picker select')?.value==='stretch-demo' && !document.getElementById('btnSave').disabled`));
  await app.settings.evaluate(`document.querySelector('${row} .guidance-authoring-trigger').click()`);
  const authoring = await app.settings.evaluate(`(() => {
    const dialog=document.getElementById('guidanceAuthoring'), prompt=document.getElementById('guidancePrompt');
    const r=dialog.getBoundingClientRect();
    return {open:dialog.open,prompt:prompt.value,readOnly:prompt.readOnly,
      palette:Array.from(dialog.querySelectorAll('table tr')).slice(1).map(row=>Array.from(row.cells).map(cell=>cell.textContent)),
      controls:['copyGuidancePrompt','saveGuidanceDemo','saveGuidanceTemplate'].map(id=>({id,enabled:!document.getElementById(id).disabled})),
      bounds:{x:r.x,y:r.y,right:r.right,bottom:r.bottom},width:innerWidth,height:innerHeight};
  })()`);
  assert.ok(authoring.open && authoring.readOnly && authoring.prompt.length > 4000);
  for (const text of ['<!doctype html>', '--sb-background', '--sb-on-accent', 'prefers-color-scheme', '240', 'UTF-8']) assert.ok(authoring.prompt.includes(text), `完整提示词必须包含 ${text}`);
  assert.ok(authoring.palette.length >= 10 && authoring.palette.every(row => row.length === 3 && row.every(Boolean)));
  assert.ok(authoring.palette.some(row => row[1] !== row[2]), '作者面板实际提供亮暗配色');
  assert.ok(authoring.controls.every(control => control.enabled));
  assert.ok(authoring.bounds.x >= 0 && authoring.bounds.y >= 0 && authoring.bounds.right <= authoring.width + 1 && authoring.bounds.bottom <= authoring.height + 1);
  const selection = await app.settings.evaluate(`(() => { const text=document.getElementById('guidancePrompt');
    text.closest('details').open=true; document.getElementById('selectGuidancePrompt').click();
    return {start:text.selectionStart,end:text.selectionEnd,length:text.value.length,focused:document.activeElement===text}; })()`);
  assert.equal(selection.start, 0); assert.equal(selection.end, selection.length); assert.ok(selection.focused);
  await app.settings.screenshot('html-authoring-prompt');
  // 不将完整材料重复写入运行报告；原文已由随包 textarea 实际取得并验证。
  report.checks.push({ name: '真实作者材料入口：完整提示词与模板、亮暗配色表和手动全选',
    ...authoring, prompt: undefined, promptLength: authoring.prompt.length, selection,
    notCovered: report.builtinHtml.notCovered });
  await app.settings.evaluate('document.querySelector("#guidanceAuthoring .close-button").click()');
  await waitFor('作者材料关闭并移除弹窗', () => app.settings.evaluate('!document.getElementById("guidanceAuthoring")'));
  const selected = await app.settings.invoke('get_settings');
  await app.settings.invoke('hide_settings');
  await app.settings.invoke('save_settings', { settings: { ...selected, use_schedule: true } });
  await app.settings.invoke('reset_timer');
  app = await restartWorkForBreak(context);
  try {
    report.builtinHtml.stage = '内置 HTML 真实渲染和系统主题';
    inspector = await createFrameInspector(app.reminder);
    await waitFor('内置 HTML 的真实 srcdoc 页面就绪', () => app.reminder.evaluate('document.querySelector("iframe.guidance-frame")?.dataset.state === "ready"'));
    await waitFor('内置 HTML 随包脚本和正文实际载入', () => inspector.evaluate('document.querySelector("h1")?.textContent === "站起来，舒展一下" && !!document.getElementById("read")'));
    assert.deepEqual((await app.reminder.invoke('timer_info')).guidance, { kind: 'html', source: { type: 'builtin', id: 'stretch-demo' } });
    assert.ok(await app.reminder.evaluate('document.getElementById("stretchText").hidden'), '仅内置 HTML 不补通用休息文字');
    const frame = await inspector.frameInfo();
    await app.reminder.screenshot('html-builtin-initial');
    await inspector.evaluate('document.getElementById("read").scrollIntoView({block:"center"}); document.getElementById("read").click()');
    const before = await app.reminder.invoke('timer_info');
    await delay(1200);
    assert.ok((await app.reminder.invoke('timer_info')).remaining < before.remaining);
    const themes = [];
    for (const [theme, value] of [['light', 1], ['dark', 0]]) {
      await context.setTheme(value);
      await waitFor(`内置 HTML 系统 ${theme} 配色`, () => inspector.evaluate(`matchMedia('(prefers-color-scheme: dark)').matches===${value === 0}`));
      const state = await inspector.evaluate(`({pressed:document.getElementById('read').getAttribute('aria-pressed'),
        confirmation:document.getElementById('confirmation').textContent,
        background:getComputedStyle(document.documentElement).backgroundColor,
        externalSources:Array.from(document.querySelectorAll('[src],link[href]')).map(e=>e.getAttribute('src')||e.getAttribute('href')).filter(Boolean)})`);
      assert.equal(state.pressed, 'true'); assert.ok(state.confirmation);
      assert.deepEqual(state.externalSources, [], '随包示范不用本地旁文件或在线资源');
      assert.equal((await inspector.frameInfo()).frameId, frame.frameId);
      const layout = await inspectLayout(app.reminder, 'btnEnd');
      await app.reminder.screenshot(`html-builtin-${theme}`);
      themes.push({ theme, state, ...layout });
    }
    assert.notEqual(themes[0].state.background, themes[1].state.background);
    await app.reminder.evaluate('document.getElementById("btnEnd").click()');
    await waitFor('结束内置 HTML 销毁 frame', async () => !(await inspector.frameInfo()));
    report.checks.push({ name: '真实表单选定内置 HTML：脚本交互、无外部资源、计时独立、主题保留和结束销毁', themes });
    await app.settings.invoke('save_settings', { settings: previous });
    await app.settings.invoke('reset_timer');
    report.builtinHtml.status = 'passed';
    return app;
  } finally {
    try { await inspector?.dispose(); } catch { /* 保留原生失败。 */ }
  }
}
