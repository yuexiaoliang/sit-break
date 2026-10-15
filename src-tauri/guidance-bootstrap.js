(() => {
  if (window === window.top || !/^sit-break-guidance-[a-f0-9]{32}$/i.test(window.name) || location.href !== 'about:srcdoc') return;
  if (window.__sitBreakGuidanceBridge) return;
  Object.defineProperty(window, '__sitBreakGuidanceBridge', { value: true });
  document.addEventListener('click', event => {
    const element = event.target instanceof Element ? event.target.closest('a[href]') : null;
    if (!element) return;
    event.preventDefault();
    const href = element.getAttribute('href') || '';
    if (!href.startsWith('#') || element.hasAttribute('download')) return;
    try {
      const id = decodeURIComponent(href.slice(1));
      if (!id) window.scrollTo(0, 0);
      else (document.getElementById(id) || document.getElementsByName(id)[0])?.scrollIntoView();
    } catch { /* 无效锚点保持当前页面。 */ }
  }, true);
  window.open = () => null;
  const ready = () => parent.postMessage({ source:'sit-break-guidance',session:window.name,type:'ready',documentUrl:location.href }, '*');
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', ready, {once:true});
  else ready();
})();
