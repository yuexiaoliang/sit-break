import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

/** 用各平台实际存在的资源构造攻击页面；不提供 IPC 或文件访问特权。 */
export async function renderCapabilityFixture({ baseUrl, knownFileUrl, validAssetUrl, validGuidanceUrl = null, attackSettings }) {
  assert.ok(new URL(baseUrl).protocol.startsWith('http'));
  assert.equal(new URL(knownFileUrl).protocol, 'file:');
  assert.ok(typeof validAssetUrl === 'string' && validAssetUrl.length > 0);
  assert.ok(attackSettings && typeof attackSettings.sound === 'boolean', '必须提供合法的完整设置，建议仅反转 sound');
  const values = { FIXTURE_BASE_URL: baseUrl, KNOWN_FILE_URL: knownFileUrl,
    VALID_ASSET_URL: validAssetUrl, VALID_GUIDANCE_URL: validGuidanceUrl, ATTACK_SETTINGS: attackSettings };
  const template = await readFile(new URL('./native-fixtures/capability-attacks.html', import.meta.url), 'utf8');
  return template.replace(/__(\w+)_JSON__/g, (_, name) => {
    assert.ok(Object.hasOwn(values, name), `未知样例占位符 ${name}`);
    return JSON.stringify(values[name]).replaceAll('<', '\\u003c');
  });
}
