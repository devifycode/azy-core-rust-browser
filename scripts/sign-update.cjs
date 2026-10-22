// Signing keys stay local. Only the public key and signed manifest are published.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'..'),privatePath=path.join(root,'.update-signing/private.pem'),publicPath=path.join(root,'assets/update-public.der');
if(process.argv.includes('--init')){
 if(fs.existsSync(privatePath)||fs.existsSync(publicPath))throw Error('Signing key already exists; refusing to rotate it');
 const pair=crypto.generateKeyPairSync('rsa',{modulusLength:3072,publicKeyEncoding:{type:'spki',format:'der'},privateKeyEncoding:{type:'pkcs8',format:'pem'}});
 fs.mkdirSync(path.dirname(privatePath),{recursive:true});fs.writeFileSync(privatePath,pair.privateKey,{flag:'wx',mode:0o600});fs.writeFileSync(publicPath,pair.publicKey,{flag:'wx'});console.log('Update signing key initialized; keep .update-signing private and backed up.');process.exit(0);
}
if(!fs.existsSync(privatePath))throw Error('Missing original signing key; restore .update-signing/private.pem from your backup.');
const info=JSON.parse(fs.readFileSync(path.join(root,'BUILD-INFO.json'))),exe=fs.readFileSync(path.join(root,'AzyCore.exe'));
assert.equal(crypto.createHash('sha256').update(exe).digest('hex'),info.sha256.toLowerCase());
const manifest={schema:1,version:info.version,platform:'windows-x64',size:exe.length,sha256:info.sha256.toLowerCase(),url:`https://github.com/devifycode/azy-core-rust-browser/releases/download/v${info.version}/AzyCore.exe`};
const bytes=Buffer.from(JSON.stringify(manifest)),signature=crypto.sign('sha256',bytes,fs.readFileSync(privatePath));assert.ok(crypto.verify('sha256',bytes,{key:fs.readFileSync(publicPath),format:'der',type:'spki'},signature));
const output=path.join(root,'.cache/publish',`v${info.version}`,'update-manifest.json');fs.mkdirSync(path.dirname(output),{recursive:true});fs.writeFileSync(output,JSON.stringify({payload:bytes.toString('base64'),signature:signature.toString('base64')},null,2)+'\n');console.log(output);
