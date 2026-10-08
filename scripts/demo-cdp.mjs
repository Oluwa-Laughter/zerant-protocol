// Local-only CDP helper for genuine browser demo capture. Never exports cookies.
export const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
export async function connect(endpoint, targetId) {
  const tabs = await fetch(`${endpoint}/json/list`).then((r) => r.json());
  const target = tabs.find((t) => t.type === 'page' && (!targetId || t.id === targetId));
  if (!target) throw new Error('No browser page available');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  let seq = 0;
  const pending = new Map();
  const listeners = new Map();
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.id) {
      const item = pending.get(message.id);
      if (!item) return;
      pending.delete(message.id); clearTimeout(item.timer);
      message.error ? item.reject(new Error(message.error.message)) : item.resolve(message.result);
    } else listeners.get(message.method)?.(message.params);
  };
  const call = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++seq;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, method === 'Page.navigate' ? 120000 : 30000);
    pending.set(id, { resolve, reject, timer });
    ws.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression) => {
    const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
    return result.result.value;
  };
  const waitFor = async (expression) => {
    const end = Date.now() + 120000;
    while (Date.now() < end) { if (await evaluate(expression)) return; await delay(200); }
    throw new Error(`Expected browser state did not appear: ${expression}`);
  };
  return { call, evaluate, waitFor, on: (name, handler) => listeners.set(name, handler), close: () => ws.close() };
}
