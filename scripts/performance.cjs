const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { execFileSync } = require('node:child_process');
const assert = require('node:assert/strict');
const { CDP, delay, waitFor, targets, findUi } = require('./cdp.cjs');
const root = path.resolve(__dirname, '..');
let requests = 0, ui;
const pages = [];
const server = http.createServer((req, res) => {
  if (req.url === '/favicon.ico') { res.writeHead(404); res.end(); return; }
  requests++;
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(`<!doctype html><title>Performance fixture ${req.url}</title><input id="draft"><script>window.fixture=new Uint8Array(8*1024*1024);fixture.fill(1);window.ticks=0;setInterval(()=>ticks++,100)</script>`);
});
function metrics() {
  return JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-File', path.join(__dirname, 'process-metrics.ps1'), '-BrowserPid', process.env.AZY_BENCH_PID], { encoding: 'utf8' }));
}
async function state() { return ui.evaluate('JSON.parse(JSON.stringify(state))'); }
async function command(cmd, data={}) { await ui.evaluate(`send(${JSON.stringify(cmd)},${JSON.stringify(data)})`); }
(async () => {
  await new Promise(r => server.listen(3138, '127.0.0.1', r));
  ui = await findUi();
  await waitFor(async () => (await state()).tabs.length === 1, 'startup');
  await delay(1800);
  const startup = metrics(); console.log('startup', startup);
  for (let i=0;i<12;i++) await command('new');
  await waitFor(async () => (await state()).tabs.length === 13, 'blank tabs');
  await delay(1800);
  const blanks = metrics();
  blanks.pageTargets = (await targets()).filter(t=>t.type==='page').length;
  console.log('13 blank tabs', blanks);
  for (let i=0;i<4;i++) {
    await command('new-url', { value:`http://127.0.0.1:3138/bench${i}` });
    await waitFor(async () => (await state()).tabs.some(t=>t.url.endsWith(`/bench${i}`)&&!t.loading&&t.title.startsWith('Performance')), 'fixture load');
    const target = (await targets()).find(t=>t.url.endsWith(`/bench${i}`));
    const c = new CDP(target.webSocketDebuggerUrl);
    await c.evaluate('document.getElementById("draft").value="unsaved draft survives"');
    c.close();
  }
  await delay(2000);
  const loaded = metrics(); console.log('4 loaded pages', loaded);
  const requestsBeforeSleep = requests;
  console.log('Waiting 65 seconds for background idle policy');
  await delay(65000);
  const idle = metrics();
  idle.suspendedTabs = (await state()).tabs.filter(t=>t.suspended).length;
  console.log('background idle', idle);
  const allTabs = (await state()).tabs;
  const first = allTabs.find(t=>t.url.endsWith('/bench0'));
  await command('switch', { id: first.id });
  const c = new CDP((await targets()).find(t=>t.url.endsWith('/bench0')).webSocketDebuggerUrl);
  pages.push(c);
  assert.equal(await c.evaluate('document.getElementById("draft").value'), 'unsaved draft survives');
  const ticks = await c.evaluate('window.ticks');
  await delay(350);
  assert.ok(await c.evaluate('window.ticks') > ticks);
  assert.equal(requests, requestsBeforeSleep, 'Resuming must not reload pages');
  const report = { label: process.env.AZY_BENCH_LABEL, at: new Date().toISOString(), startup, blanks, loaded, idle, resumePreservedDraft: true };
  fs.writeFileSync(path.join(root,'.cache',`performance-${report.label}.json`), JSON.stringify(report,null,2));
  console.log('PASS Resume preserves form contents without reload');
  await command('quit');
})().catch(e=>{console.error(e);process.exitCode=1}).finally(()=>{ui?.close();pages.forEach(p=>p.close());server.close()});
