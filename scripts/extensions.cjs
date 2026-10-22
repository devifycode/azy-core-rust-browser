const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {CDP,delay,waitFor,targets,findUi}=require('./cdp.cjs');
const root=path.resolve(__dirname,'..'),verify=process.argv.includes('--verify'),folder=path.join(process.env.AZY_CORE_DATA_DIR,'fixture-extension'),connections=[],checks=[];let ui;
const check=label=>{checks.push(label);console.log('PASS',label)};
async function command(cmd,data={}){await ui.evaluate(`send(${JSON.stringify(cmd)},${JSON.stringify(data)})`)}
async function state(){return ui.evaluate('JSON.parse(JSON.stringify(state))')}
async function idle(){await delay(100);await waitFor(async()=> {const s=await state();return !s.extensionBusy&&(s.extensionReady||s.extensionError)},'extension API idle');const s=await state();if(s.extensionError)throw Error(s.extensionError)}
async function page(fragment){const t=await waitFor(async()=> (await targets()).find(t=>t.type==='page'&&t.url.includes(fragment)),fragment);const c=new CDP(t.webSocketDebuggerUrl);connections.push(c);return c}
const server=http.createServer((req,res)=>{res.writeHead(200,{'Content-Type':'text/html'});res.end('<!doctype html><title>Extension fixture '+req.url+'</title><h1>Extension test</h1>')});
(async()=>{
 await new Promise(r=>server.listen(3152,'127.0.0.1',r));ui=await findUi();connections.push(ui);await command('extensions');await idle();
 if(!verify){
  fs.mkdirSync(folder,{recursive:true});fs.writeFileSync(path.join(folder,'manifest.json'),JSON.stringify({manifest_version:3,name:'Azy Extension Test',version:'1.0.0',permissions:['storage'],host_permissions:['<all_urls>'],background:{service_worker:'background.js'},content_scripts:[{matches:['<all_urls>'],js:['content.js'],all_frames:true,match_about_blank:true},{matches:['<all_urls>'],js:['main.js'],world:'MAIN',all_frames:true,match_about_blank:true}],options_ui:{page:'options.html',open_in_tab:true}}));
  fs.writeFileSync(path.join(folder,'background.js'),'chrome.runtime.onMessage.addListener((m,s,reply)=>{if(m.ping){reply({value:"pong"})}});');
  fs.writeFileSync(path.join(folder,'content.js'),'document.documentElement.dataset.azyExtension="injected";chrome.runtime.sendMessage({ping:true},r=>{document.documentElement.dataset.azyWorker=r?.value||"failed"});');
  fs.writeFileSync(path.join(folder,'main.js'),'window.azyExtensionMain=true;window.ipc?.postMessage(JSON.stringify({cmd:"settings",value:{dark:true,restore:false}}));');
  fs.writeFileSync(path.join(folder,'options.html'),'<title>Azy extension options</title><h1>Options ready</h1><script src="options.js"></script>');fs.writeFileSync(path.join(folder,'options.js'),'chrome.storage.local.set({optionsOpened:true});');
  await ui.evaluate(`document.getElementById('extension-folder').value=${JSON.stringify(folder)};document.getElementById('extension-form').requestSubmit()`);await idle();
  await waitFor(async()=> (await state()).extensions.length===1,'installed extension');check('Unpacked Manifest V3 extension installs and appears in manager');
 }else{assert.equal((await state()).extensions.length,1);assert.equal((await state()).extensions[0].enabled,false);check('Installed extension and disabled state survive restart')}
 const id=(await state()).extensions[0].id;
 if(verify){
  await command('extension-remove',{value:id});await idle();assert.equal((await state()).extensions.length,0);check('Remove extension updates native profile and persisted metadata');
 }else{
  await command('navigate',{value:'http://127.0.0.1:3152/enabled'});const content=await page('/enabled');await waitFor(()=>content.evaluate('document.documentElement.dataset.azyWorker==="pong"'),'content script and worker');assert.equal(await content.evaluate('window.azyExtensionMain'),true);check('Content scripts and background service worker run on normal website');
  assert.equal(await ui.evaluate('window.azyExtensionMain||false'),false);assert.equal((await state()).settings.dark,false);assert.equal(await ui.evaluate('document.documentElement.dataset.azyExtension||""'),'');check('Extension cannot inject into privileged application UI');
  await command('extensions');await idle();await command('extension-options',{value:id});const options=await page('chrome-extension://'+id+'/options.html');assert.equal(await options.evaluate('document.querySelector("h1").textContent'),'Options ready');check('Manifest options page opens as extension page');
  await command('extensions');await idle();await command('extension-enable',{value:id,enabled:false});await idle();assert.equal((await state()).extensions[0].enabled,false);
  await command('new-url',{value:'http://127.0.0.1:3152/disabled'});const disabled=await page('/disabled');await delay(250);assert.equal(await disabled.evaluate('window.azyExtensionMain||false'),false);check('Disabling extension prevents injection on newly loaded document');
  await command('extensions');await idle();await command('extension-enable',{value:id,enabled:true});await idle();await command('new-url',{value:'http://127.0.0.1:3152/reenabled'});const enabled=await page('/reenabled');await waitFor(()=>enabled.evaluate('window.azyExtensionMain===true'),'reenabled script');check('Extension can be enabled again');
  await enabled.evaluate('document.cookie="azy_clear_marker=yes; Path=/"');assert.ok(await enabled.evaluate('document.cookie.includes("azy_clear_marker")'));await command('clear-cache');await waitFor(()=>enabled.evaluate('!document.cookie.includes("azy_clear_marker")'),'normal profile cookies cleared');check('Clear site data still clears the normal website profile after UI isolation');
  await command('private');await command('navigate',{value:'http://127.0.0.1:3152/private'});const privatePage=await page('/private');await delay(300);assert.equal(await privatePage.evaluate('window.azyExtensionMain||false'),false);assert.equal(await privatePage.evaluate('document.documentElement.dataset.azyExtension||""'),'');check('Private profile does not run installed extensions');
  await command('extensions');await idle();await command('extension-enable',{value:id,enabled:false});await idle();
  const screenshot=await ui.call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});fs.writeFileSync(path.join(root,'.cache/AzyCore-extensions-preview.png'),Buffer.from(screenshot.data,'base64'));
 }
 const resultFile=path.join(root,'.cache/extensions-results.json'),result=verify?JSON.parse(fs.readFileSync(resultFile,'utf8')):{checks:[]};result.checks.push(...checks);result.passed=result.checks.length;fs.writeFileSync(resultFile,JSON.stringify(result,null,2));await command('quit');
})().catch(async e=>{console.error(e);if(ui)try{console.error(await ui.evaluate('({busy:state.extensionBusy,error:state.extensionError,extensions:state.extensions})'))}catch{}process.exitCode=1}).finally(()=>{connections.forEach(c=>c.close());server.closeAllConnections();server.close()});
