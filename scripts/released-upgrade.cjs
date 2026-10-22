// Real production-to-production update through the public GitHub release channel.
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),crypto=require('node:crypto'),assert=require('node:assert/strict');
const {CDP,targets,waitFor,delay}=require('./cdp.cjs');
const root=process.env.AZY_UPGRADE_TEST_ROOT,from=process.env.AZY_UPGRADE_FROM,to=process.env.AZY_UPGRADE_TO,folder=path.join(root,'Azy Core ü'),exe=path.join(folder,'AzyCore.exe'),profile=path.join(folder,'AzyCore.Data'),clients=[];
const hash=f=>crypto.createHash('sha256').update(fs.readFileSync(f)).digest('hex');let original;
async function findUi(version){return waitFor(async()=>{for(const target of await targets()){if(target.type!=='page')continue;const c=new CDP(target.webSocketDebuggerUrl);try{if(await c.evaluate('typeof receive==="function"')&&await c.evaluate(`state.update?.current===${JSON.stringify(version)}`)){clients.push(c);return c}}catch{}c.close()}},`production browser ${version}`,45000)}
(async()=>{
 fs.mkdirSync(profile,{recursive:true});fs.copyFileSync(path.resolve(`.cache/upgrade-client-${from}/AzyCore.exe`),exe);const oldHash=hash(exe);
 fs.writeFileSync(path.join(profile,'keep.txt'),'preserved profile');fs.writeFileSync(path.join(folder,'AzyCore.BUILD-INFO.json'),JSON.stringify({version:from}));
 fs.writeFileSync(path.join(profile,'browser.json'),JSON.stringify({settings:{dark:true,restore:false},bookmarks:[{title:'Keep bookmark',url:'https://example.com/',time:1}],downloads:[{id:1,url:'https://example.com/file',path:'previous.txt',status:'complete',time:1}]}));
 process.env.AZY_CORE_DATA_DIR=profile;
 original=cp.spawn(exe,[],{windowsHide:true,stdio:'ignore',env:{...process.env,AZY_CORE_DATA_DIR:profile}});
 let ui=await findUi(from);const state=()=>ui.evaluate('JSON.parse(JSON.stringify(state))'),cmd=c=>ui.evaluate(`send(${JSON.stringify(c)})`);
 await cmd('about');await cmd('update-check');await waitFor(async()=>['available','error'].includes((await state()).update.status),'published newer release',45000);assert.equal((await state()).update.status,'available',(await state()).update.message);assert.equal((await state()).update.version,to);
 console.log('PASS Previous production release discovers the latest signed release on GitHub');
 await cmd('update-download');await waitFor(async()=>['ready','error'].includes((await state()).update.status),'production package downloaded',60000);assert.equal((await state()).update.status,'ready',(await state()).update.message);assert.equal(hash(exe),oldHash);console.log('PASS Previous client downloads and validates the new package without changing its running executable');
 await cmd('update-install');await waitFor(()=>original.exitCode!==null||original.signalCode!==null,'previous process exits',15000);ui.close();ui=await findUi(to);const s=await state(),build=JSON.parse(fs.readFileSync(path.resolve('BUILD-INFO.json')));
 assert.equal(hash(exe),build.sha256.toLowerCase());assert.equal(s.settings.dark,true);assert.equal(s.settings.restore,false);assert.ok(s.bookmarks.some(b=>b.title==='Keep bookmark'));assert.equal(fs.readFileSync(path.join(profile,'keep.txt'),'utf8'),'preserved profile');assert.equal(JSON.parse(fs.readFileSync(path.join(folder,'AzyCore.BUILD-INFO.json'))).version,to);assert.equal(s.settings.video_fullscreen,true);assert.equal(s.downloads[0].seen,true);
 console.log('PASS Actual released executable upgrades in place, restarts, migrates settings and preserves profile data');
 fs.writeFileSync(path.resolve('.cache/released-upgrade-results.json'),JSON.stringify({checksPassed:3,from,to,sha256:hash(exe)},null,2)+'\n');await cmd('quit');await delay(400);
})().catch(e=>{console.error(e);process.exitCode=1}).finally(()=>{clients.forEach(c=>c.close());if(original&&original.exitCode===null&&original.signalCode===null)original.kill()});
