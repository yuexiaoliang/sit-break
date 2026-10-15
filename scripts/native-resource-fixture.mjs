import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { mkdir, readFile, writeFile, rename } from 'node:fs/promises';
import { join } from 'node:path';

/** 已导入副本的明确测试 fixture；不调用或冒充原生文件选择／导入流程。 */
export async function preimportedVideo(dataDirectory, bytes) {
  const id = `local-${randomBytes(16).toString('hex')}`;
  const directory = join(dataDirectory, 'guidance');
  await mkdir(directory, { recursive: true });
  await writeFile(join(directory, `${id}.mp4`), bytes, { flag: 'wx' });
  return { kind: 'video', source: { type: 'local', id } };
}

/** 从真实应用保存的当前工作进度缩短等待，再由产品恢复入口自然提醒。 */
export async function restartWorkForBreak(context) {
  await context.quit(context.app.settings);
  const path = join(context.dataDirectory, 'progress.json');
  const progress = JSON.parse(await readFile(path, 'utf8'));
  assert.equal(progress.info.mode, 'work');
  progress.info.remaining = 1;
  await writeFile(`${path}.resource-check`, JSON.stringify(progress), 'utf8');
  await rename(`${path}.resource-check`, path);
  const app = await context.launch();
  context.app = app;
  context.onApp?.(app);
  await context.waitFor('恢复已配置本地指引后自然提醒', async () => (await app.reminder.invoke('timer_info')).mode === 'remind');
  await app.reminder.evaluate('document.getElementById("btnBreak").click()');
  await context.waitFor('实际按钮开始本地指引休息', async () => (await app.reminder.invoke('timer_info')).mode === 'break');
  return app;
}
