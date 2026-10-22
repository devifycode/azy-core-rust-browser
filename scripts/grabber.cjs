const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {CDP,delay,waitFor,targets,findUi}=require('./cdp.cjs');
const source=process.argv[2],connections=[],checks=[];let ui;
const json=JSON.stringify({asset:{version:'2.0'},scene:0,scenes:[{nodes:[]}]});const chunk=Buffer.from(json.padEnd(Math.ceil(json.length/4)*4,' '));const bytes=Buffer.alloc(20+chunk.length);bytes.write('glTF');bytes.writeUInt32LE(2,4);bytes.writeUInt32LE(bytes.length,8);bytes.writeUInt32LE(chunk.length,12);bytes.write('JSON',16);chunk.copy(bytes,20);
const server=http.createServer((req,res)=>{if(req.url.endsWith('.glb')){res.writeHead(200,{'Content-Type':'model/gltf-binary','Content-Length':bytes.length});res.end(bytes)}else{res.writeHead(200,{'Content-Type':'text/html'});res.end('<title>GLB fixture</title><h1>GLB test</h1><a href="/azy-popup-test.glb">Model</a><script>fetch("/azy-popup-test.glb")</script>')}});
const check=s=>{checks.push(s);console.log('PASS',s)};
(async()=>{
 await new Promise(r=>server.listen(3153,'127.0.0.1',r));ui=await findUi();connections.push(ui);const cmd=(c,value)=>ui.evaluate(`send(${JSON.stringify(c)},{value:${JSON.stringify(value||'')}})`);
 if(process.argv.includes('--verify')){
  await waitFor(()=>ui.evaluate('state.extensionReady&&!state.extensionBusy'),'startup refresh without manager');assert.equal(await ui.evaluate('state.extensions.length'),1);assert.equal(await ui.evaluate('state.extensions[0].source.popup'),'popup.html');assert.equal(await ui.evaluate('!!state.extensions[0].source.icon'),true);assert.equal(await ui.evaluate('document.querySelectorAll("[data-extension-popup]").length'),1);check('Existing installation upgrades metadata and restores toolbar automatically on restart');
  const file=path.resolve(__dirname,'../.cache/grabber-results.json'),result=JSON.parse(fs.readFileSync(file,'utf8'));result.checks.push(...checks);result.passed=result.checks.length;fs.writeFileSync(file,JSON.stringify(result,null,2));await cmd('quit');return;
 }
 await cmd('extensions');await waitFor(()=>ui.evaluate('state.extensionReady&&!state.extensionBusy'),'ready');await cmd('extension-install',source);await waitFor(()=>ui.evaluate('state.extensions?.length===1&&!state.extensionBusy'),'installed');const id=await ui.evaluate('state.extensions[0].id');
 assert.equal(await ui.evaluate('!!state.extensions[0].source.popup'),true);assert.equal(await ui.evaluate('state.extensions[0].source.icon.length>0'),true);
 await cmd('navigate','http://127.0.0.1:3153/page');await delay(1200);const first=await ui.evaluate('state.active');
 await cmd('new-url','http://127.0.0.1:3153/page');await delay(1200);const second=await ui.evaluate('state.active');assert.notEqual(first,second);
 assert.equal(await ui.evaluate(`document.querySelector('[data-extension-popup="${id}"] img').naturalWidth>0`),true);check('Real extension icon appears in toolbar');
 const pages=(await targets()).filter(t=>t.type==='page'&&t.url==='http://127.0.0.1:3153/page');assert.equal(pages.length,2);
 let activeWindow;
 for(const t of pages){const c=new CDP(t.webSocketDebuggerUrl);connections.push(c);await c.evaluate('document.body.dataset.azyNativeTest="page"');const win=(await c.call('Browser.getWindowForTarget')).windowId; const hasFocus=await c.evaluate('document.hasFocus()');if(hasFocus)activeWindow=win;}
 assert.ok(activeWindow,'active website has native focus');
 await cmd('extension-popup',id);await waitFor(()=>ui.evaluate('!!state.extensionPopup'),'popup opened');
 const popupTarget=await waitFor(async()=> (await targets()).find(t=>t.type==='page'&&t.url===`chrome-extension://${id}/popup.html`),'popup target');const popup=new CDP(popupTarget.webSocketDebuggerUrl);connections.push(popup);
 await waitFor(()=>popup.evaluate('document.querySelector("#list .item")!==null'),'actual Grabber asset list');
 assert.equal(await ui.evaluate('state.active'),second);const tab=await popup.evaluate('(async()=> (await chrome.tabs.query({active:true,currentWindow:true}))[0])()');assert.equal(tab.windowId,activeWindow);assert.equal(tab.url,'http://127.0.0.1:3153/page');check('Popup targets selected website with identical URLs in two tabs');
 const callbackId=await popup.evaluate('new Promise(r=>chrome.tabs.query({active:true,currentWindow:true},t=>r(t[0].id)))');assert.equal(callbackId,tab.id);check('Promise and callback forms of active-tab query agree');
 assert.ok((await popup.evaluate('document.querySelector("#list").innerText')).includes('azy-popup-test'));await popup.evaluate('window.ipc?.postMessage(JSON.stringify({cmd:"settings",value:{dark:true}}))');assert.equal(await ui.evaluate('state.settings.dark'),false);check('Real GLB Grabber detects assets without browser UI privileges');
 const shot=await popup.call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});fs.writeFileSync(path.resolve(__dirname,'../.cache/AzyCore-extension-popup-preview.png'),Buffer.from(shot.data,'base64'));
 await popup.evaluate('document.querySelector("#list .actions button").click()');await waitFor(()=>popup.evaluate('document.querySelector("#list").innerText.includes("Downloaded")'),'download success');
 const download=await waitFor(()=>{const folder=path.join(process.env.AZY_CORE_DATA_DIR,'Downloads');const walk=d=>fs.existsSync(d)?fs.readdirSync(d,{withFileTypes:true}).flatMap(f=>f.isDirectory()?walk(path.join(d,f.name)):[path.join(d,f.name)]):[];return walk(folder).find(p=>p.endsWith('.glb'))},'portable GLB download');assert.deepEqual(fs.readFileSync(download),bytes);check('GLB download succeeds and bytes match inside application data directory');
 await cmd('escape');await waitFor(()=>ui.evaluate('!state.extensionPopup'),'popup dismissed');await cmd('extension-popup',id);await waitFor(()=>ui.evaluate('!!state.extensionPopup'),'popup reopened');
 // Use the same tab command as the UI, below; changing tabs must dismiss the popup.
 await ui.evaluate(`send('switch',{id:${first}})`);await waitFor(()=>ui.evaluate('!state.extensionPopup'),'switch dismiss');
 await cmd('extension-popup',id);await waitFor(()=>ui.evaluate('!!state.extensionPopup'),'popup reopen on first tab');await cmd('extension-popup',id);await waitFor(()=>ui.evaluate('!state.extensionPopup'),'toggle dismiss');
 await cmd('extension-popup',id);await waitFor(()=>ui.evaluate('!!state.extensionPopup'),'popup reopen before new tab');await cmd('new');await waitFor(()=>ui.evaluate('!state.extensionPopup'),'new tab dismiss');check('Popup closes on Escape, tab switch, toolbar toggle, and new tab');
 fs.writeFileSync(path.resolve(__dirname,'../.cache/grabber-results.json'),JSON.stringify({passed:checks.length,checks},null,2));await cmd('quit');
})().catch(async e=>{console.error(e);if(ui)try{console.error(await ui.evaluate('({popup:state.extensionPopup,error:state.extensionError,extensions:state.extensions})'))}catch{}process.exitCode=1}).finally(()=>{connections.forEach(c=>c.close());server.closeAllConnections();server.close()});
