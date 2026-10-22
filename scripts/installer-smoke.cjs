const assert=require('node:assert/strict'),path=require('node:path');
const {findUi}=require('./cdp.cjs');
(async()=>{const ui=await findUi();try{
 const state=await ui.evaluate('JSON.parse(JSON.stringify(state))');
 assert.equal(path.resolve(state.dataPath).toLowerCase(),path.resolve(process.env.AZY_CORE_DATA_DIR).toLowerCase());
 assert.ok(state.tabs.length>0);
 await ui.evaluate('send("quit")');
}finally{ui.close()}})().catch(e=>{console.error(e);process.exitCode=1});
