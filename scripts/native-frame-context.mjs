import assert from 'node:assert/strict';

/** 仅测试调试连接使用；定位真实 iframe 默认脚本上下文，不改变页面来源权限。 */
export async function createFrameInspector(connection) {
  const contexts = new Map();
  const disposers = [];
  const attached = new Set();
  const attachmentErrors = new Map();
  const key = (sessionId, id) => `${sessionId ?? ''}:${id}`;
  disposers.push(connection.on('Runtime.executionContextCreated', ({ context }, sessionId) => {
    contexts.set(key(sessionId, context.id), { ...context, sessionId });
  }));
  disposers.push(connection.on('Runtime.executionContextDestroyed', ({ executionContextId }, sessionId) => {
    contexts.delete(key(sessionId, executionContextId));
  }));
  disposers.push(connection.on('Runtime.executionContextsCleared', (_, sessionId) => {
    for (const [entryKey, context] of contexts) if (context.sessionId === sessionId) contexts.delete(entryKey);
  }));
  disposers.push(connection.on('Target.detachedFromTarget', ({ sessionId }) => {
    attached.delete(sessionId);
    attachmentErrors.delete(sessionId);
    for (const [entryKey, context] of contexts) if (context.sessionId === sessionId) contexts.delete(entryKey);
  }));
  disposers.push(connection.on('Target.attachedToTarget', ({ sessionId, targetInfo }) => {
    if (targetInfo.type !== 'iframe') return;
    attached.add(sessionId);
    void connection.call('Runtime.enable', {}, sessionId).catch((error) => {
      if (attached.has(sessionId)) attachmentErrors.set(sessionId, error);
    });
  }));
  await connection.call('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
  // 已有连接可能启用了 Runtime；重开调试域取得当前上下文，不重载或修改页面。
  await connection.call('Runtime.disable');
  await connection.call('Runtime.enable');

  async function frameInfo(namePrefix = 'sit-break-guidance-') {
    const { frameTree } = await connection.call('Page.getFrameTree');
    const frames = [];
    const visit = (tree) => { frames.push(tree.frame); for (const child of tree.childFrames ?? []) visit(child); };
    visit(frameTree);
    const frame = frames.find((item) => item.parentId && item.name?.startsWith(namePrefix));
    if (!frame) return null;
    const context = [...contexts.values()].find((item) => item.auxData?.frameId === frame.id && item.auxData?.isDefault);
    if (!context) {
      if (attachmentErrors.size) throw attachmentErrors.values().next().value;
      return null;
    }
    return { frameId: frame.id, name: frame.name, url: frame.url, contextId: context.id, sessionId: context.sessionId };
  }

  return {
    frameInfo,
    async evaluate(expression, namePrefix) {
      const frame = await frameInfo(namePrefix);
      assert.ok(frame, '没有找到实际指引 iframe 的默认脚本上下文');
      const result = await connection.call('Runtime.evaluate', { expression, contextId: frame.contextId,
        awaitPromise: true, returnByValue: true }, frame.sessionId);
      assert.ok(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
      return result.result.value;
    },
    async dispose() {
      for (const dispose of disposers) dispose();
      await connection.call('Target.setAutoAttach', { autoAttach: false, waitForDebuggerOnStart: false, flatten: true });
      contexts.clear();
      attached.clear();
      attachmentErrors.clear();
    },
  };
}
