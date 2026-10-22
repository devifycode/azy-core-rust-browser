const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {CDP,delay,waitFor,targets,findUi}=require('./cdp.cjs');
const root=path.resolve(__dirname,'..'),connections=[],checks=[];let ui;
const check=label=>{checks.push(label);console.log('PASS',label)};
const server=http.createServer((req,res)=>{
 if(req.url.startsWith('/slow-image')){setTimeout(()=>{if(!res.destroyed){res.writeHead(200,{'Content-Type':'image/svg+xml'});res.end('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="24" height="24" fill="#f97316"/></svg>')}},2400);return}
 const slow=req.url.startsWith('/fallback');
 setTimeout(()=>{if(!res.destroyed){res.writeHead(200,{'Content-Type':'text/html; charset=utf-8','Cache-Control':'no-store'});res.end('<!doctype html><title>Presentation fixture</title><style>html,body{margin:0;background:#101010;color:#fff;font:18px Arial}h1{margin:30px}</style><h1>Halaman gelap siap</h1><img src="/slow-image?'+encodeURIComponent(req.url)+'"><input id="draft">')}},slow?5000:850);
});
async function command(cmd,data={}){return ui.evaluate(`send(${JSON.stringify(cmd)},${JSON.stringify(data)})`)}
async function state(){return ui.evaluate('JSON.parse(JSON.stringify(state))')}
async function active(){const s=await state();return s.tabs.find(t=>t.id===s.active)}
async function pending(){await waitFor(async()=> (await active()).presentation_pending,'pending presentation');assert.ok(await ui.evaluate('innerHeight>400'))}
async function revealed(){await waitFor(async()=> !(await active()).presentation_pending,'document revealed');await waitFor(()=>ui.evaluate('innerHeight===86'),'toolbar-only bounds')}
(async()=>{
 await new Promise(r=>server.listen(3147,'127.0.0.1',r));ui=await findUi();connections.push(ui);
 await command('navigate',{value:'http://127.0.0.1:3147/dark'});await pending();
 const screenshot=await ui.call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});fs.writeFileSync(path.join(root,'.cache/presentation-loading.png'),Buffer.from(screenshot.data,'base64'));
 check('First navigation retains full loading surface before document arrives');
 await revealed();assert.equal((await active()).loading,true);
 const target=await waitFor(async()=> (await targets()).find(t=>t.type==='page'&&t.url.endsWith('/dark')),'dark page');const content=new CDP(target.webSocketDebuggerUrl);connections.push(content);
 assert.equal(await content.evaluate('getComputedStyle(document.body).backgroundColor'),'rgb(16, 16, 16)');
 check('Dark page is revealed after DOM without waiting for slow images or changing site colors');
 await command('home');await waitFor(async()=> (await active()).url==='azy://newtab','home');
 await command('navigate',{value:'http://127.0.0.1:3147/reused'});await pending();await revealed();check('Reused view from new-tab page receives the same presentation guard');
 await command('settings',{value:{...(await state()).settings,dark:true}});await command('home');
 await command('navigate',{value:'http://127.0.0.1:3147/theme'});await pending();assert.equal(await ui.evaluate('getComputedStyle(document.body).backgroundColor'),'rgb(32, 33, 36)');await revealed();check('Dark application theme keeps matching loading background');
 await command('home');await command('navigate',{value:'http://127.0.0.1:3147/fallback'});await pending();const start=Date.now();await revealed();assert.ok(Date.now()-start<4200);assert.equal((await active()).loading,true);check('Three-second fallback prevents a slow response from trapping the loading surface');
 fs.writeFileSync(path.join(root,'.cache/presentation-results.json'),JSON.stringify({passed:checks.length,checks},null,2));
 await command('quit');
})().catch(async e=>{console.error(e);if(ui)try{console.error('State at failure:',await active())}catch{}process.exitCode=1}).finally(()=>{connections.forEach(c=>c.close());server.closeAllConnections();server.close()});
