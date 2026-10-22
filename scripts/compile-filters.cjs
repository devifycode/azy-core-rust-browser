// Extract only unconditional hostname rules; this is not an ABP rule interpreter.
const fs=require('node:fs');
const crypto=require('node:crypto');
const sources=['easylist-adservers.txt','easyprivacy-general.txt'];
const revision=JSON.parse(fs.readFileSync('.cache/filters/commit.json','utf8')).sha;
const domains=new Set();
// Small, explicit Azy Core additions for common advertising/analytics networks.
const additionalDomains=['doubleclick.net','googlesyndication.com','googleadservices.com','googletagservices.com','google-analytics.com','analytics.google.com','adservice.google.com'];
const inputs=sources.map(file=>{
 const text=fs.readFileSync(`.cache/filters/${file}`,'utf8');
 for(const line of text.split(/\r?\n/)){
  const m=line.trim().match(/^\|\|([a-z0-9.-]+)\^$/i);
  if(m&&m[1].includes('.')&&m[1].split('.').every(label=>/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/i.test(label)))domains.add(m[1].toLowerCase());
 }
 return {file,sha256:crypto.createHash('sha256').update(text).digest('hex')};
});
if(domains.size<10000||!domains.has('adnxs.com'))throw new Error('Invalid or incomplete filter snapshot');
additionalDomains.forEach(domain=>domains.add(domain));
fs.writeFileSync('assets/filters/domains.txt',`# EasyList + EasyPrivacy hostname subset\n# The EasyList authors (https://easylist.to/)\n# CC BY-SA 3.0 or later: https://creativecommons.org/licenses/by-sa/3.0/\n# Upstream revision: ${revision}\n# Modified: unconditional hostname rules extracted, sorted and deduplicated.\n`+[...domains].sort().join('\n')+'\n');
fs.writeFileSync('assets/filters/snapshot.json',JSON.stringify({source:'The EasyList authors',revision,license:'CC-BY-SA-3.0-or-later',domains:domains.size,additionalDomains,inputs},null,2)+'\n');
console.log(`Bundled ${domains.size} hostname rules (${revision})`);
