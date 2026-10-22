const fs=require('node:fs'),http=require('node:http'),path=require('node:path'),assert=require('node:assert/strict');
const {CDP,delay,waitFor,targets,findUi}=require('./cdp.cjs');
const root=path.resolve(__dirname,'..'),connections=[],checks=[];let ui;
const logo=fs.readFileSync(path.join(root,'logo-azy-core.png'));
const svg=Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="1356" height="1356"><rect width="1356" height="1356" fill="#0b8043"/><image x="178" y="178" width="1000" height="1000" href="data:image/png;base64,'+logo.toString('base64')+'"/></svg>');
const icoDirectory=fs.readdirSync(path.join(root,'target/release/build')).find(name=>name.startsWith('azy-core-')&&fs.existsSync(path.join(root,'target/release/build',name,'out/azy-core.ico')));
const ico=fs.readFileSync(path.join(root,'target/release/build',icoDirectory,'out/azy-core.ico'));
const check=label=>{checks.push(label);console.log('PASS',label)};
const server=http.createServer((req,res)=>{
 if(req.url.endsWith('.ico')||req.url.endsWith('.png')||req.url.endsWith('.svg')){
  if(req.url==='/favicon.ico'){res.writeHead(404);res.end();return}
  const ext=path.extname(req.url),data=ext==='.ico'?ico:ext==='.png'?logo:svg;
  setTimeout(()=>{if(!res.destroyed){res.writeHead(200,{'Content-Type':ext==='.ico'?'image/x-icon':ext==='.png'?'image/png':'image/svg+xml'});res.end(data)}},180);return;
 }
 res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'});
 res.end('<!doctype html><title>Water-style favicon fixture</title><link rel="icon" type="image/x-icon" href="../../assets/img/logo.ico"><link rel="icon" type="image/png" href="../assets/img/favicon/favicon-96x96.png" sizes="96x96"><link rel="icon" type="image/svg+xml" href="../assets/img/favicon/favicon.svg"><link rel="shortcut icon" href="../assets/img/favicon/favicon.ico"><h1>Multiple favicon declarations</h1><script>let n=0;const timer=setInterval(()=>{location.hash="water_saving_dma_"+n++;if(n===60)clearInterval(timer)},10)</script>');
});
async function command(cmd,data={}){await ui.evaluate(`send(${JSON.stringify(cmd)},${JSON.stringify(data)})`)}
async function active(){return ui.evaluate('state.tabs.find(t=>t.id===state.active)')}
async function iconReady(){await waitFor(async()=> (await active()).favicon?.length>0,'favicon data');await waitFor(()=>ui.evaluate('document.querySelector(".tab.active img.favicon")?.naturalWidth>0'),'rendered favicon')}
(async()=>{
 await new Promise(r=>server.listen(3151,'127.0.0.1',r));ui=await findUi();connections.push(ui);
 await command('navigate',{value:'http://127.0.0.1:3151/dashboard/'});await iconReady();await waitFor(async()=> (await active()).url.includes('#water_saving_dma_59'),'hash route');await iconReady();
 check('Multiple relative ICO/PNG/SVG declarations render while hash routes change');
 const target=await waitFor(async()=> (await targets()).find(t=>t.type==='page'&&t.url.includes('3151/dashboard/')),'fixture page');const content=new CDP(target.webSocketDebuggerUrl);connections.push(content);
 const previousIcon=(await active()).favicon;
 await content.evaluate('document.querySelectorAll("link").forEach(l=>l.remove());const l=document.createElement("link");l.rel="icon";l.type="image/svg+xml";l.href="/assets/img/favicon/large.svg";document.head.append(l)');await waitFor(async()=> (await active()).favicon!==previousIcon,'updated SVG favicon');await iconReady();
 check('Large SVG icon remains renderable with bounded thumbnail data');assert.ok((await active()).favicon.length<=1024*1024);
 const previousTab=(await active()).id;
 await command('new-url',{value:'http://127.0.0.1:3151/dashboard/'});await waitFor(async()=> (await active()).id!==previousTab,'new tab activation');await iconReady();await waitFor(async()=> !(await active()).loading,'cached document load');check('New tab obtains a cached website favicon after navigation completes');
 await command('reload');await waitFor(async()=> !(await active()).loading,'reload');await delay(300);await iconReady();check('Reload and hash-only navigation retain the website icon');
 const screenshot=await ui.call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});fs.writeFileSync(path.join(root,'.cache/AzyCore-favicon-fixed-preview.png'),Buffer.from(screenshot.data,'base64'));
 fs.writeFileSync(path.join(root,'.cache/favicon-results.json'),JSON.stringify({passed:checks.length,checks},null,2));await command('quit');
})().catch(e=>{console.error(e);process.exitCode=1}).finally(()=>{connections.forEach(c=>c.close());server.closeAllConnections();server.close()});
