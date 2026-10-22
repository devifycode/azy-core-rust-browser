const fs=require('node:fs');
const path=require('node:path');
const http=require('node:http');
const assert=require('node:assert/strict');
const {CDP,waitFor,targets,findUi}=require('./cdp.cjs');
let ui,content;
const hits=[];
const server=http.createServer((req,res)=>{
 hits.push(req.url);
 res.setHeader('Cache-Control','no-store');res.setHeader('Access-Control-Allow-Origin','*');
 if(req.url==='/favicon.ico'){res.writeHead(404);res.end();return;}
 if(req.url==='/ad.js'||req.url==='/track.js'||req.url==='/normal.js'||req.url==='/worker-ad.js'){
  res.setHeader('Content-Type','text/javascript');res.end(req.url==='/normal.js'?'window.normalScript=true':req.url==='/ad.js'?'window.adScript=true':req.url==='/track.js'?'window.trackerScript=true':'self.adWorker=true');return;
 }
 if(req.url==='/worker.js'){
  res.setHeader('Content-Type','text/javascript');res.end('try{importScripts("http://ads.doubleclick.net:3140/worker-ad.js")}catch{};postMessage("worker-ready")');return;
 }
 if(req.url==='/frame'){res.setHeader('Content-Type','text/html');res.end('<p>ad iframe</p>');return;}
 res.setHeader('Content-Type','text/html');
 res.end('<!doctype html><title>Adblock fixture</title><h1>Normal content</h1><script src="/normal.js"></script><script src="http://ads.doubleclick.net:3140/ad.js"></script><script src="http://00px.net:3140/track.js"></script><iframe src="http://ads.doubleclick.net:3140/frame"></iframe><script>new Worker("/worker.js").onmessage=()=>window.workerReady=true</script>');
});
async function state(){return ui.evaluate('JSON.parse(JSON.stringify(state))')}
async function command(cmd,data={}){return ui.evaluate(`send(${JSON.stringify(cmd)},${JSON.stringify(data)})`)}
async function loaded(){await waitFor(async()=>{const s=await state();return s.tabs.find(t=>t.id===s.active)?.title==='Adblock fixture'&&!s.tabs.find(t=>t.id===s.active)?.loading},'fixture page loaded')}
const checks=[];
function pass(label){checks.push(label);console.log('PASS',label)}
(async()=>{
 await new Promise(r=>server.listen(3140,'0.0.0.0',r));
 ui=await findUi();
 await command('navigate',{value:'http://127.0.0.1:3140/page'});await loaded();
 content=new CDP((await targets()).find(t=>t.url==='http://127.0.0.1:3140/page').webSocketDebuggerUrl);
 content.onEvent=m=>{if(m.method==='Network.loadingFailed')console.log('Network probe:',m.params.errorText,m.params.blockedReason||'')};
 await content.call('Network.enable');
 await waitFor(()=>content.evaluate('window.workerReady===true'),'worker request filtering');
 await waitFor(async()=>{const s=await state();return s.tabs.find(t=>t.id===s.active)?.blocked>=4},'blocked request count');
 assert.equal(await content.evaluate('window.normalScript'),true);
 assert.equal(await content.evaluate('Boolean(window.adScript||window.trackerScript)'),false);
 for(const target of ['/ad.js','/track.js','/frame','/worker-ad.js'])assert.equal(hits.includes(target),false,`Blocked request must never reach server: ${target}`);
 pass('Ads, tracker, iframe and worker requests blocked before network; normal script loads');
 await command('adblock');
 await waitFor(()=>ui.evaluate('state.panel==="adblock" && document.querySelector("#panel .block-stat")?.textContent!=="0"'),'adblock control panel');
 await command('toggle-site-adblock');await loaded();
 await waitFor(()=>content.evaluate('window.adScript===true && window.trackerScript===true'),'site exception allows scripts');
 assert.ok(hits.includes('/ad.js')&&hits.includes('/frame')&&hits.includes('/worker-ad.js'));
 const store=JSON.parse(fs.readFileSync(path.join(process.env.AZY_CORE_DATA_DIR,'browser.json'),'utf8'));
 assert.ok(store.settings.adblock_allowed_sites.includes('127.0.0.1'));
 pass('Per-site exception reloads page, permits actual network requests and persists');
 await command('toggle-site-adblock');await loaded();
 await waitFor(async()=>{const s=await state();return s.tabs.find(t=>t.id===s.active)?.blocked>=3},'re-enable site protection');
 await command('settings',{value:{...(await state()).settings,adblock_enabled:false}});
 await command('reload');await loaded();
 await waitFor(()=>content.evaluate('window.adScript===true'),'global off');
 await command('settings',{value:{...(await state()).settings,adblock_enabled:true}});
 await command('private');await command('navigate',{value:'http://127.0.0.1:3140/private'});await loaded();
 await waitFor(async()=>{const s=await state();return s.tabs.some(t=>t.id===s.active&&t.private&&t.adblock&&t.blocked>=3)},'private tab filtering');
 pass('Global switch and private tab filtering');
 const shot=await ui.call('Page.captureScreenshot',{format:'png'});
 fs.writeFileSync(path.resolve(__dirname,'../.cache/AzyCore-adblock-preview.png'),Buffer.from(shot.data,'base64'));
 fs.writeFileSync(path.resolve(__dirname,'../.cache/adblock-results.json'),JSON.stringify({passed:checks,at:new Date().toISOString()},null,2));
 await command('quit');
})().catch(e=>{console.error(e);console.error('Fixture requests:',hits);process.exitCode=1}).finally(()=>{ui?.close();content?.close();server.closeAllConnections();server.close()});
