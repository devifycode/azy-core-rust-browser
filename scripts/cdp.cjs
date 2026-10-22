const delay = ms => new Promise(r => setTimeout(r, ms));
class CDP {
  constructor(url) {
    this.ws = new WebSocket(url); this.next = 0; this.pending = new Map();
    this.ready = new Promise((r, j) => { this.ws.onopen = r; this.ws.onerror = j; });
    this.ws.onmessage = e => {
      const m = JSON.parse(e.data), p = this.pending.get(m.id);
      if (!p && m.method && this.onEvent) this.onEvent(m);
      if (p) { this.pending.delete(m.id); clearTimeout(p.timer); m.error ? p.reject(new Error(JSON.stringify(m.error))) : p.resolve(m.result); }
    };
  }
  async call(method, params = {}) {
    await this.ready; const id = ++this.next;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 15000);
      this.pending.set(id, { resolve, reject, timer });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
  async evaluate(expression) {
    const r = await this.call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, userGesture: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  }
  close() { this.ws.close(); }
}
async function waitFor(fn, description, timeout = 25000) {
  const start = Date.now(); let lastError;
  while (Date.now() - start < timeout) { try { const v = await fn(); if (v) return v; } catch (e) { lastError = e; } await delay(150); }
  throw new Error(`Timeout: ${description}${lastError ? ` (${lastError.message})` : ''}`);
}
async function targets() {
  const fs=require('node:fs'),path=require('node:path'),ports=new Set(),root=process.env.AZY_CORE_DATA_DIR;
  if(root)for(const profile of ['WebView2','WebView2.UI'])for(const suffix of ['DevToolsActivePort','EBWebView/DevToolsActivePort']){
    try{const port=Number(fs.readFileSync(path.join(root,profile,suffix),'utf8').split(/\r?\n/)[0]);if(port>0)ports.add(port)}catch{}
  }
  if(!ports.size&&!process.env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS?.includes('--remote-debugging-port=0'))ports.add(9224);
  const results=await Promise.allSettled([...ports].map(async port=>{const r=await fetch(`http://127.0.0.1:${port}/json/list`,{signal:AbortSignal.timeout(1500)});return r.json()}));
  return results.flatMap(r=>r.status==='fulfilled'&&Array.isArray(r.value)?r.value:[]);
}
async function findUi() {
  return waitFor(async () => {
    for (const t of await targets()) {
      if (t.type !== 'page') continue;
      const c = new CDP(t.webSocketDebuggerUrl);
      if (await c.evaluate('typeof window.receive === "function"')) return c;
      c.close();
    }
  }, 'browser UI');
}
module.exports = { CDP, delay, waitFor, targets, findUi };
