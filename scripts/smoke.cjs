// Tests the compiled application through its temporary local DevTools endpoint.
// No npm dependencies; test server, data, and screenshots stay in this project.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '..');
const delay = ms => new Promise(r => setTimeout(r, ms));
class CDP {
  constructor(url) {
    this.ws = new WebSocket(url); this.next = 0; this.pending = new Map();
    this.ready = new Promise((r, j) => { this.ws.onopen = r; this.ws.onerror = j; });
    this.ws.onmessage = e => {
      const m = JSON.parse(e.data), p = this.pending.get(m.id);
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
const connections = [];
async function targets() { return require('./cdp.cjs').targets(); }
async function connect(target) { const c = new CDP(target.webSocketDebuggerUrl); connections.push(c); await c.ready; return c; }
async function waitFor(fn, description, timeout = 20000) {
  const start = Date.now();
  let lastError;
  while (Date.now() - start < timeout) { try { const v = await fn(); if (v) return v; } catch (e) { lastError = e; } await delay(120); }
  if (lastError) console.error('Last probe error:', lastError);
  throw new Error(`Timeout: ${description}`);
}
let ui;
const checks = [];
const check = label => { checks.push(label); console.log('PASS', label); };
async function state() { return ui.evaluate('JSON.parse(JSON.stringify(state))'); }
async function command(cmd, data = {}) { await ui.evaluate(`send(${JSON.stringify(cmd)},${JSON.stringify(data)})`); }
async function pageWith(urlPart) {
  const target = await waitFor(async () => (await targets()).find(t => t.type === 'page' && t.url.includes(urlPart)), `page ${urlPart}`);
  return connect(target);
}
const server = http.createServer((req, res) => {
  if (req.url.startsWith('/favicon')) {
    res.writeHead(200, { 'Content-Type': 'image/svg+xml' });
    const color = req.url.includes('second') ? '#157c48' : '#ed6712';
    res.end(`<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" rx="6" fill="${color}"/><path d="M7 24 16 6l9 18h-5l-4-9-4 9z" fill="white"/></svg>`); return;
  }
  if (req.url === '/download') {
    res.writeHead(200, { 'Content-Type': 'text/plain', 'Content-Disposition': 'attachment; filename="azy-test.txt"' });
    res.end('Azy Core download verified.'); return;
  }
  if (req.url === '/cookie') {
    res.writeHead(200, { 'Content-Type': 'text/html', 'Set-Cookie': 'azy_normal=present; Path=/; SameSite=Lax' });
  } else res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
  const page = req.url === '/two' ? 'Two' : req.url === '/private' ? 'Private' : 'One';
  res.end(`<!doctype html><title>Azy Test ${page}</title><link rel="icon" type="image/svg+xml" href="/favicon.svg"><h1>Azy Test ${page}</h1><p>Searchable Azy browser content.</p><a id="next" href="/two">Next page</a> <a id="popup" target="_blank" href="/popup">New tab</a> <a id="download" href="/download">Download</a>`);
});
(async () => {
  await new Promise(r => server.listen(3137, '127.0.0.1', r));
  await waitFor(async () => {
    for (const t of await targets()) {
      if (t.type !== 'page') continue;
      const c = await connect(t);
      if (await c.evaluate('typeof window.receive === "function"')) { ui = c; return true; }
      c.close();
    }
  }, 'browser UI');
  await waitFor(async () => (await state()).tabs.length === 1, 'initial tab');
  assert.equal(await ui.evaluate('document.querySelector(".hero-logo").naturalWidth > 0'), true);
  check('Startup, new tab, embedded user logo');
  const screenshot = await ui.call('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  fs.writeFileSync(path.join(root, '.cache', 'AzyCore-preview.png'), Buffer.from(screenshot.data, 'base64'));
  await command('navigate', { value: 'http://127.0.0.1:3137/cookie' });
  await waitFor(async () => (await state()).tabs.some(t => t.title === 'Azy Test One' && !t.loading), 'page load');
  const content = await pageWith('/cookie');
  assert.equal(await content.evaluate('document.querySelector("h1").textContent'), 'Azy Test One');
  await content.evaluate('window.ipc?.postMessage(JSON.stringify({cmd:"settings",value:{dark:true,search:"bing",restore:false}}))');
  await delay(150);
  assert.equal((await state()).settings.dark, false);
  check('Real web rendering, title and address updates, no privileged website IPC');
  await waitFor(async () => ui.evaluate('!!document.querySelector(".tab.active img.favicon")?.naturalWidth'), 'website favicon rendered');
  const firstIcon = (await state()).tabs[0].favicon;
  await content.evaluate('document.querySelector("link[rel=icon]").href="/favicon-second.svg"');
  await waitFor(async () => (await state()).tabs[0].favicon && (await state()).tabs[0].favicon !== firstIcon, 'dynamic favicon update');
  await waitFor(async () => ui.evaluate('!!document.querySelector(".tab.active img.favicon")?.naturalWidth'), 'updated favicon rendered');
  const faviconPreview = await ui.call('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  fs.writeFileSync(path.join(root, '.cache', 'AzyCore-favicon-preview.png'), Buffer.from(faviconPreview.data, 'base64'));
  check('Native website favicon rendering and dynamic updates');
  await command('bookmark');
  await waitFor(async () => (await state()).bookmarks.length === 1, 'bookmark');
  await content.evaluate('document.querySelector("#next").click()');
  await waitFor(async () => (await state()).tabs.some(t => t.title === 'Azy Test Two' && t.back), 'navigation');
  await command('back');
  await waitFor(async () => (await state()).tabs.some(t => t.url.endsWith('/cookie') && t.forward), 'back');
  await command('forward');
  await waitFor(async () => (await state()).tabs.some(t => t.url.endsWith('/two')), 'forward');
  check('Bookmark, native back and forward, history');
  await command('zoom-in');
  await waitFor(async () => (await state()).tabs[0].zoom === 110, 'zoom');
  await command('zoom-reset');
  await content.evaluate('document.querySelector("#popup").click()');
  await waitFor(async () => (await state()).tabs.length === 2, 'popup tab');
  const popupId = (await state()).active;
  await command('close', { id: popupId });
  await waitFor(async () => (await state()).tabs.length === 1, 'close tab');
  await command('reopen');
  await waitFor(async () => (await state()).tabs.length === 2, 'reopen tab');
  check('Zoom, target=_blank tab, close and reopen tab');
  await command('private');
  await waitFor(async () => (await state()).tabs.some(t => t.private), 'private tab');
  await command('navigate', { value: 'http://127.0.0.1:3137/private' });
  const privatePage = await pageWith('/private');
  await waitFor(async () => (await state()).tabs.some(t => t.title === 'Azy Test Private' && !t.loading), 'private load');
  assert.equal(await privatePage.evaluate('document.cookie.includes("azy_normal")'), false);
  assert.equal((await state()).history.some(h => h.url.endsWith('/private')), false);
  check('Private cookie isolation and exclusion from history');
  const normalId = (await state()).tabs.find(t => !t.private).id;
  await command('switch', { id: normalId });
  await ui.evaluate('send("settings",{value:{...state.settings,download_ask:false}})');
  await waitFor(async()=> (await state()).settings.download_ask===false,'automatic download mode for smoke test');
  await content.evaluate('document.querySelector("#download").click()');
  await waitFor(async () => (await state()).downloads.some(d => d.status === 'complete'), 'download');
  const download = (await state()).downloads.find(d => d.status === 'complete');
  assert.equal(fs.readFileSync(download.path, 'utf8'), 'Azy Core download verified.');
  check('Actual file download, local path and completion tracking');
  await command('settings', { value: { dark: true, search: 'duckduckgo', restore: true } });
  await waitFor(async () => ui.evaluate('document.body.classList.contains("dark")'), 'dark theme');
  for (const p of ['history', 'bookmarks', 'downloads', 'settings-page', 'menu', 'about']) {
    await command(p); await waitFor(async () => ui.evaluate(`!document.getElementById('panel').classList.contains('hidden') && state.panel === ${JSON.stringify(p)}`), p);
  }
  check('Theme, search setting and all internal pages');
  const store = JSON.parse(fs.readFileSync(path.join(process.env.AZY_CORE_DATA_DIR || path.join(root, '.cache', 'smoke-profile'), 'browser.json'), 'utf8'));
  assert.equal(store.bookmarks.length, 1);
  assert.equal(store.settings.search, 'duckduckgo');
  assert.equal(store.session.some(u => u.endsWith('/private')), false);
  assert.equal(store.history.some(h => h.url.endsWith('/private')), false);
  check('Persisted settings/bookmarks/session and private-session exclusion');
  fs.writeFileSync(path.join(root, '.cache', 'smoke-results.json'), JSON.stringify({ passed: checks, at: new Date().toISOString() }, null, 2));
  await command('quit');
  console.log(`${checks.length} integration groups passed.`);
})().catch(e => { console.error(e); process.exitCode = 1; }).finally(() => { connections.forEach(c => c.close()); server.close(); });
