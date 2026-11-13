import { randomBytes } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

/** 已导入副本的明确测试 fixture；不调用或冒充原生文件选择／导入流程。 */
export async function preimportedVideo(dataDirectory, bytes) {
  const id = `local-${randomBytes(16).toString('hex')}`;
  const directory = join(dataDirectory, 'guidance');
  await mkdir(directory, { recursive: true });
  await writeFile(join(directory, `${id}.mp4`), bytes, { flag: 'wx' });
  return { kind: 'video', source: { type: 'local', id } };
}
