const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),cp=require('node:child_process');
const {pathToFileURL}=require('node:url');
const {CDP,findUi,targets,waitFor,delay}=require('./cdp.cjs');
const fixture=path.join(process.env.AZY_CORE_DATA_DIR,'Laporan #1 ü.pdf');
// Two-page, dependency-free PDF fixture. Byte offsets and stream lengths are real.
if(process.argv[2]==='prepare'){
 fs.mkdirSync(path.dirname(fixture),{recursive:true});
 const objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>','<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>','<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'];
 for(let i=1;i<=2;i++){if(i===2)objects.push('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 7 0 R >>');const stream=`BT /F1 24 Tf 72 700 Td (Azy Core PDF - page ${i}) Tj ET`;objects.push(`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`)}
 let data='%PDF-1.4\n',offsets=[];objects.forEach((o,i)=>{offsets.push(Buffer.byteLength(data));data+=`${i+1} 0 obj\n${o}\nendobj\n`});const start=Buffer.byteLength(data);data+=`xref\n0 ${objects.length+1}\n0000000000 65535 f \n`+offsets.map(n=>String(n).padStart(10,'0')+' 00000 n \n').join('')+`trailer\n<< /Size ${objects.length+1} /Root 1 0 R >>\nstartxref\n${start}\n%%EOF\n`;fs.writeFileSync(fixture,data);process.exit(0);
}
let ui;const clients=[],report=path.resolve('.cache/local-pdf-results.json');
const checks=process.argv[2]==='initial'?[]:JSON.parse(fs.readFileSync(report)).checks;
const check=s=>{checks.push(s);console.log('PASS',s)};
const dialog=(action)=>new Promise((resolve,reject)=>cp.execFile('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(__dirname,'open-file-dialog.ps1'),'-Action',action,'-File',fixture],{encoding:'utf8',windowsHide:true},(e,out)=>e?reject(e):resolve(out)));
(async()=>{
 ui=await findUi();clients.push(ui);const state=()=>ui.evaluate('JSON.parse(JSON.stringify(state))'),cmd=(c,v={})=>ui.evaluate(`send(${JSON.stringify(c)},${JSON.stringify(v)})`),url=pathToFileURL(fixture).href;
 await waitFor(async()=>typeof(await state()).settings.dark==='boolean','native UI state');
 async function loaded(){return waitFor(async()=>{const s=await state();return s.tabs.find(t=>t.id===s.active&&t.url===url&&!t.loading&&!t.presentation_pending)},'PDF tab visible')}
 async function viewer(){return waitFor(async()=>{for(const t of (await targets()).filter(t=>t.type==='webview'&&t.url.includes('/edge_pdf/'))){const c=new CDP(t.webSocketDebuggerUrl);if(await c.evaluate('document.body.innerText.includes("load has completed")&&document.getElementById("pagelength")?.textContent.includes("2")')){clients.push(c);return c}c.close()}},'two-page native PDF viewer loaded')}
 if(process.argv[2]==='initial'){
  const before=await state();await cmd('open-file');const controls=JSON.parse(await dialog('Inspect'));assert.ok(controls.some(c=>c.text.includes('Dokumen PDF')));check('Ctrl+O command opens native file picker with PDF filter selected');
  await dialog('Cancel');await delay(250);assert.equal((await state()).tabs.length,before.tabs.length);assert.equal((await state()).active,before.active);check('Cancelling file picker leaves current tabs unchanged');
  await cmd('open-file');await dialog('Choose');const tab=await loaded();assert.equal((await state()).tabs.length,before.tabs.length+1);await waitFor(async()=>(await state()).tabs.find(t=>t.id===tab.id)?.title===path.basename(fixture),'decoded PDF tab title');assert.equal(tab.private,false);assert.ok((await state()).tabs.some(t=>t.id===before.active));check('Local PDF opens in new tab preserving original tab and Unicode filename');
  const pdf=await viewer();assert.ok(await pdf.evaluate('!!document.getElementById("zoom-in")&&!!document.getElementById("print")&&!!document.getElementById("saveAs")'));check('Built-in PDF reader loads both pages and provides zoom, print and save controls');
  const t=(await targets()).find(t=>t.type==='page'&&t.url===url);const page=new CDP(t.webSocketDebuggerUrl);clients.push(page);await delay(500);fs.writeFileSync(path.resolve('.cache/local-pdf-preview.png'),Buffer.from((await page.call('Page.captureScreenshot',{format:'png'})).data,'base64'));
  await cmd('new');await cmd('navigate',{value:'"'+fixture+'"'});await loaded();assert.equal((await state()).downloads.length,0);check('Quoted Windows path in address bar opens PDF inline without creating a download');
  await cmd('private');await cmd('open-file');await dialog('Choose');assert.equal((await loaded()).private,true);const saved=JSON.parse(fs.readFileSync(path.join(process.env.AZY_CORE_DATA_DIR,'browser.json')));assert.equal(saved.session.length,3);check('Opening PDF in private mode creates private tab excluded from saved session');
 }else if(process.argv[2]==='restore'){
  assert.ok(!(await state()).tabs.some(t=>t.private));await loaded();await viewer();check('Regular PDF tabs restore after restart while private PDF tabs are excluded');
 }else{
  const tab=await loaded();await viewer();assert.equal((await state()).tabs.length,1);await waitFor(async()=>(await state()).tabs.find(t=>t.id===tab.id)?.title===path.basename(fixture),'decoded argument tab title');check('Launching AzyCore.exe with a local PDF path opens the built-in reader');
 }
 fs.writeFileSync(report,JSON.stringify({checksPassed:checks.length,checks},null,2)+'\n');await cmd('quit');
})().catch(e=>{console.error(e);process.exitCode=1}).finally(()=>clients.forEach(c=>c.close()));
