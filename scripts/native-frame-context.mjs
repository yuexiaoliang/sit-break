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
    void Promise.all([
      connection.call('Runtime.enable', {}, sessionId),
      connection.call('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true }, sessionId),
    ]).catch((error) => {
      if (attached.has(sessionId)) attachmentErrors.set(sessionId, error);
    });
  }));
  await connection.call('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
  // 已有连接可能启用了 Runtime；重开调试域取得当前上下文，不重载或修改页面。
  await connection.call('Runtime.disable');
  await connection.call('Runtime.enable');

  async function frameTree() {
    const root = await connection.call('Page.getFrameTree');
    const children = await Promise.allSettled([...attached].map(sessionId => connection.call('Page.getFrameTree', {}, sessionId)));
    const frames = new Map();
    const visit = (tree) => { frames.set(tree.frame.id, { ...frames.get(tree.frame.id), ...tree.frame }); for (const child of tree.childFrames ?? []) visit(child); };
    visit(root.frameTree);
    for (const child of children) if (child.status === 'fulfilled') visit(child.value.frameTree);
    return [...frames.values()];
  }

  async function frameInfo(selector = 'sit-break-guidance-') {
    const frames = await frameTree();
    const frame = frames.find((item) => typeof selector === 'string'
      ? item.parentId && item.name?.startsWith(selector) : item.id === selector.frameId);
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
    /** 由实际 iframe DOM owner 取得无 name 子帧 ID，避免把请求地址当作加载成功。 */
    async elementFrameInfo(expression, parentSelector) {
      const parent = await frameInfo(parentSelector);
      assert.ok(parent, '未找到 iframe owner 所在的真实上下文');
      const remote = await connection.call('Runtime.evaluate', { expression, contextId: parent.contextId,
        returnByValue: false }, parent.sessionId);
      assert.ok(!remote.exceptionDetails && remote.result.objectId, '未取得实际 iframe 元素');
      try {
        const { node } = await connection.call('DOM.describeNode', { objectId: remote.result.objectId }, parent.sessionId);
        assert.ok(node.frameId, '实际 iframe owner 尚无 frame ID');
        const frame = (await frameTree()).find(item => item.id === node.frameId);
        return { frameId: node.frameId, url: frame?.url ?? null, name: frame?.name ?? '',
          context: await frameInfo({ frameId: node.frameId }) };
      } finally {
        await connection.call('Runtime.releaseObject', { objectId: remote.result.objectId }, parent.sessionId);
      }
    },
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
