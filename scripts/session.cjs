const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const { CDP, waitFor, targets, findUi } = require('./cdp.cjs');
let ui;
const requested=[];
const server=http.createServer((req,res)=>{
  if(req.url==='/favicon.ico'){res.writeHead(404);res.end();return;}
  requested.push(req.url);res.writeHead(200,{'Content-Type':'text/html'});
  res.end(`<!doctype html><title>Restored ${req.url}</title><h1>${req.url}</h1>`);
});
(async()=>{
  await new Promise(r=>server.listen(3139,'127.0.0.1',r));
  ui=await findUi();
  const state=()=>ui.evaluate('JSON.parse(JSON.stringify(state))');
  await waitFor(async()=>(await state()).tabs.length===3,'restored tab metadata');
  assert.equal((await targets()).filter(t=>t.type==='page').length,1,'Only the UI should be created for restored blank tab');
  assert.deepEqual(requested,[],'Restored background URLs must not load');
  assert.equal((await state()).bookmarks[0].title,'Saved bookmark');
  const first=(await state()).tabs[0].id;
  await ui.evaluate(`send('switch',{id:${first}})`);
  await waitFor(async()=>(await state()).tabs.some(t=>t.id===first&&t.title==='Restored /first'&&!t.loading),'activate deferred tab');
  assert.equal((await targets()).filter(t=>t.type==='page').length,2);
  assert.deepEqual(requested,['/first']);
  await ui.evaluate(`send('close',{id:${first}})`);
  await waitFor(async()=>(await state()).tabs.some(t=>t.title==='Restored /second'&&!t.loading),'close activates next deferred tab');
  assert.deepEqual(requested,['/first','/second']);
  const store=JSON.parse(fs.readFileSync(path.join(process.env.AZY_CORE_DATA_DIR,'browser.json'),'utf8'));
  assert.equal(store.bookmarks[0].title,'Saved bookmark');
  const result={passed:['Deferred session restore','Load on activation','Close activates deferred neighbor','Bookmarks preserved'],at:new Date().toISOString()};
  fs.writeFileSync(path.resolve(__dirname,'../.cache/session-results.json'),JSON.stringify(result,null,2));
  console.log('PASS',result.passed.join('; '));
  await ui.evaluate('send("quit")');
})().catch(e=>{console.error(e);process.exitCode=1}).finally(()=>{ui?.close();server.close()});
