(() => {
  // 这份脚本只桥接就绪和 HTTP(S) 外链，不向用户页面提供任何应用命令。
  if (window === window.top || !/^sit-break-guidance-[a-f0-9]{32}$/i.test(window.name)) return;
  if (location.href !== 'about:srcdoc' && !/^https?:\/\//i.test(location.href)) return;
  if (window.__sitBreakGuidanceBridge) return;
  Object.defineProperty(window, '__sitBreakGuidanceBridge', { value: true });
  const session = window.name;
  const send = (type, url) => parent.postMessage({ source: 'sit-break-guidance', session, type, url, documentUrl: location.href }, '*');
  const external = (value) => {
    try {
      const url = new URL(String(value), location.href);
      if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) return;
      send('external', url.href);
    } catch { /* 不打开无效或非 HTTP(S) 地址。 */ }
  };
  document.addEventListener('click', (event) => {
    const element = event.target instanceof Element ? event.target.closest('a[href]') : null;
    if (!element) return;
    const href = element.getAttribute('href') || '';
    event.preventDefault();
    // srcdoc 的 fallback base URL 来自宿主；不能让 #fragment 默认解析后导航到应用页面。
    let fragment = href.startsWith('#') ? href : null;
    if (fragment === null && /^https?:/.test(location.protocol)) {
      try {
        const destination = new URL(element.href);
        if (destination.href.split('#')[0] === location.href.split('#')[0] && destination.hash) fragment = destination.hash;
      } catch { /* 无效地址不进行页内导航。 */ }
    }
    if (fragment !== null && !element.hasAttribute('download')) {
      try {
        const id = decodeURIComponent(fragment.slice(1));
        if (!id) window.scrollTo(0, 0);
        else (document.getElementById(id) || document.getElementsByName(id)[0])?.scrollIntoView();
      } catch { /* 无效的 fragment 保留当前页面。 */ }
      return;
    }
    external(element.href);
  }, true);
  window.open = (url) => { if (url) external(url); return null; };
  const ready = () => {
    // 原生网络错误文档或纯文本错误响应不是已就绪的 HTML 指引。
    if (['text/html', 'application/xhtml+xml'].includes(document.contentType)) send('ready');
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', ready, { once: true });
  else ready();
})();
