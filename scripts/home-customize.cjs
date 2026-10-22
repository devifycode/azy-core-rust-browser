const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {delay,waitFor,findUi}=require('./cdp.cjs');
const root=path.resolve(__dirname,'..'),verify=process.argv.includes('--verify');let ui;const errors=[],checks=[];
const check=label=>{checks.push(label);console.log('PASS',label)};
const server=http.createServer((req,res)=>{
 if(req.url==='/brand.svg'){res.writeHead(200,{'Content-Type':'image/svg+xml'});res.end('<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" rx="7" fill="#17823b"/><path d="M7 24 16 6l9 18h-5l-4-9-4 9z" fill="white"/></svg>');return}
 if(req.url==='/favicon.ico'){if(req.headers.host.startsWith('localhost:')){res.writeHead(200,{'Content-Type':'image/png'});res.end(fs.readFileSync(path.join(root,'logo-azy-core.png')))}else{res.writeHead(404);res.end()}return}
 res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'});res.end('<!doctype html><title>Custom shortcut target</title><link rel="icon" href="/brand.svg"><h1>Saved shortcut works</h1>')});
async function state(){return ui.evaluate('JSON.parse(JSON.stringify(state))')}
async function command(cmd,data={}){await ui.evaluate(`send(${JSON.stringify(cmd)},${JSON.stringify(data)})`)}
async function click(selector){await ui.evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`)}
async function editForm(title,url){await ui.evaluate(`document.getElementById('shortcut-title').value=${JSON.stringify(title)};document.getElementById('shortcut-url').value=${JSON.stringify(url)};document.getElementById('shortcut-form').requestSubmit()`)}
(async()=>{
 await new Promise(r=>server.listen(3149,'127.0.0.1',r));ui=await findUi();ui.onEvent=m=>{if(m.method==='Runtime.exceptionThrown')errors.push(m.params.exceptionDetails)};await ui.call('Runtime.enable');
 await waitFor(async()=> (await state()).home?.shortcuts,'home settings');
 if(verify){
  await waitFor(()=>ui.evaluate('state.home.background_image && homeBackgroundUrl.startsWith("blob:")'),'restored background image');
  assert.equal((await state()).home.shortcuts.at(-1).title,'Portal tersimpan');
  assert.equal((await state()).home.background_color,'#e8f0fe');
  await waitFor(()=>ui.evaluate('!!document.querySelector("[data-url=\\"http://127.0.0.1:3149/saved\\"] img.shortcut-favicon")?.naturalWidth'),'restored shortcut icon');
  check('Custom shortcuts, color and uploaded image survive application restart');
  await click('#customize-home');await waitFor(async()=> (await state()).panel==='customize-home','customizer');await click('#clear-home-image');
  await waitFor(async()=> !(await state()).home.background_image,'remove image');assert.equal(fs.existsSync(path.join(process.env.AZY_CORE_DATA_DIR,'home-background.png')),false);
  await click('[data-home-color=""]');await waitFor(async()=> (await state()).home.background_color==='','theme reset');check('Remove image and restore browser theme persist without deleting shortcuts');
  assert.equal((await state()).home.shortcuts.length,6);
 }else{
  assert.equal((await state()).home.shortcuts.length,5);await click('#shortcut-add');
  await editForm('Portal <Azy>','localhost:3149/portal');await waitFor(async()=> (await state()).home.shortcuts.length===6,'add shortcut');
  assert.equal(await ui.evaluate('document.querySelectorAll(".shortcut-tile")[5].querySelector(".shortcut span:last-child").textContent'),'Portal <Azy>');
  await click('[data-shortcut-edit="5"]');await editForm('Portal Azy','http://127.0.0.1:3149/edited');await waitFor(async()=> (await state()).home.shortcuts[5].title==='Portal Azy','edit shortcut');
  await click('[data-url="http://127.0.0.1:3149/edited"]');await waitFor(async()=> (await state()).tabs.some(t=>t.title==='Custom shortcut target'&&!t.loading),'shortcut opens website');
  await command('home');await waitFor(async()=> {const s=await state();return s.tabs.find(t=>t.id===s.active)?.url==='azy://newtab'},'home');
  check('Add and edit shortcut through dialog; saved link opens actual website and escapes its title');
  await waitFor(()=>ui.evaluate('!!document.querySelector("[data-url=\\"http://127.0.0.1:3149/edited\\"] img.shortcut-favicon")?.naturalWidth'),'declared shortcut favicon');
  assert.ok((await state()).home.icons['http://127.0.0.1:3149']);check('Shortcut uses native cached favicon when website declares an icon outside /favicon.ico');
  await click('#shortcut-add');await editForm('Invalid','javascript:alert(1)');assert.equal(await ui.evaluate('document.getElementById("shortcut-dialog").open'),true);assert.ok(await ui.evaluate('document.getElementById("shortcut-error").textContent.length>0'));await click('#shortcut-cancel');
  await command('save-shortcut',{title:'Invalid',value:'file:///C:/private'});await delay(150);assert.equal((await state()).home.shortcuts.length,6);check('Invalid website schemes are rejected by UI and native handler');
  await click('[data-shortcut-edit="5"]');await click('#shortcut-delete');await waitFor(async()=> (await state()).home.shortcuts.length===5,'delete shortcut');
  await click('#shortcut-add');await editForm('Portal tersimpan','http://127.0.0.1:3149/saved');await waitFor(async()=> (await state()).home.shortcuts.length===6,'saved shortcut');check('Manual shortcut deletion and replacement are saved');
  await click('[data-url="http://127.0.0.1:3149/saved"]');await waitFor(async()=> {const s=await state();return s.home.icons['http://127.0.0.1:3149']&&s.tabs.some(t=>t.url.endsWith('/saved')&&!t.loading)},'replacement shortcut icon cached');await command('home');await waitFor(async()=> {const s=await state();return s.tabs.find(t=>t.id===s.active)?.url==='azy://newtab'},'return home');
  await click('#shortcut-add');await editForm('Direct icon','http://localhost:3149/');await waitFor(async()=> (await state()).home.shortcuts.length===7,'direct-icon shortcut');
  await waitFor(()=>ui.evaluate('!!document.querySelector("[data-url=\\"http://localhost:3149/\\"] img.shortcut-favicon")?.naturalWidth'),'direct website favicon');
  await click('[data-shortcut-edit="6"]');await click('#shortcut-delete');await waitFor(async()=> (await state()).home.shortcuts.length===6,'remove direct-icon shortcut');check('New shortcut loads favicon directly from website before the website is opened');
  await click('#customize-home');await waitFor(async()=> (await state()).panel==='customize-home','customizer');await click('[data-home-color="#16324f"]');await waitFor(async()=> (await state()).home.background_color==='#16324f','dark color');
  await command('home');await waitFor(()=>ui.evaluate('getComputedStyle(document.getElementById("newtab")).color==="rgb(241, 243, 244)"'),'light text on dark color');
  await command('settings',{value:{...(await state()).settings,dark:true}});await command('home-color',{value:'#e8f0fe'});await waitFor(()=>ui.evaluate('getComputedStyle(document.getElementById("newtab")).color==="rgb(32, 33, 36)"'),'dark text on light color');
  await command('settings',{value:{...(await state()).settings,dark:false}});check('Background palette and custom color preserve readable text in either browser theme');
  const png=await ui.evaluate(`(()=>{const c=document.createElement('canvas');c.width=640;c.height=360;const x=c.getContext('2d'),g=x.createLinearGradient(0,0,640,360);g.addColorStop(0,'#635bdb');g.addColorStop(1,'#f4a261');x.fillStyle=g;x.fillRect(0,0,640,360);x.fillStyle='#ffffff55';x.beginPath();x.arc(450,130,110,0,7);x.fill();return c.toDataURL('image/png').split(',')[1]})()`);
  const fixture=path.join(process.env.AZY_CORE_DATA_DIR,'background-fixture.png');fs.writeFileSync(fixture,Buffer.from(png,'base64'));
  const document=await ui.call('DOM.getDocument');const node=await ui.call('DOM.querySelector',{nodeId:document.root.nodeId,selector:'#background-file'});await ui.call('DOM.setFileInputFiles',{nodeId:node.nodeId,files:[fixture]});
  await waitFor(()=>ui.evaluate('state.home.background_image && homeBackgroundUrl.startsWith("blob:")'),'background upload');
  const saved=fs.readFileSync(path.join(process.env.AZY_CORE_DATA_DIR,'home-background.png'));assert.equal(saved.readUInt32BE(16),640);assert.equal(saved.readUInt32BE(20),360);
  await ui.evaluate('new Promise((resolve,reject)=>{const image=new Image();image.onload=resolve;image.onerror=reject;image.src=homeBackgroundUrl})');await delay(100);
  await ui.evaluate('document.getElementById("toast").classList.add("hidden")');
  const shot=await ui.call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});fs.writeFileSync(path.join(root,'.cache/AzyCore-custom-home-preview.png'),Buffer.from(shot.data,'base64'));
  check('Local image upload is decoded, stored without upscaling, and rendered on Home');
  const data=JSON.parse(fs.readFileSync(path.join(process.env.AZY_CORE_DATA_DIR,'browser.json'),'utf8'));assert.equal(data.home.shortcuts.length,6);assert.equal(data.home.background_image,true);
 }
 assert.deepEqual(errors,[]);
 const resultPath=path.join(root,'.cache/home-customize-results.json');const result=verify?JSON.parse(fs.readFileSync(resultPath,'utf8')):{checks:[]};result.checks.push(...checks);result.passed=result.checks.length;fs.writeFileSync(resultPath,JSON.stringify(result,null,2));await command('quit');
})().catch(e=>{console.error(e);process.exitCode=1}).finally(()=>{ui?.close();server.closeAllConnections();server.close()});
