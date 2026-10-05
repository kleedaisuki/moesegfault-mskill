/** Verify deployed product discovery and public workspace surfaces without account mutation. */
import assert from 'node:assert/strict';
import {setTimeout as delay} from 'node:timers/promises';

const product=new URL(process.argv[2]??'https://mskill.moesegfault.dev');
const workspace=new URL(process.argv[3]??'https://skills.moesegfault.dev');
const staging=product.hostname.includes('-staging.');
/** Retry only transient reachability while newly deployed custom domains propagate. */
async function get(origin,path) {
 for(let attempt=0;attempt<6;attempt++) {
  try {
   const response=await fetch(new URL(path,origin),{signal:AbortSignal.timeout(10_000),redirect:'manual'});
   if(![502,503,504].includes(response.status)||attempt===5) return response;
   await response.body?.cancel();
  } catch(error) {
   if(attempt===5||!(error.name==='TimeoutError'||['ENOTFOUND','EAI_AGAIN','ECONNRESET','ECONNREFUSED','ETIMEDOUT'].includes(error.cause?.code))) throw error;
  }
  await delay(3_000*(attempt+1));
 }
}
for(const [path,lang] of [['/','zh-CN'],['/ja/','ja'],['/en/','en']]) {
 const response=await get(product,path); assert.equal(response.status,200,`product ${path}`);
 const html=await response.text(); assert.match(html,new RegExp(`<html[^>]+lang=["']${lang}["']`),'explicit document language');
 assert.match(html,/application\/ld\+json/,'machine-readable product metadata');
 assert.match(html,/rel=["']canonical["']/,'canonical product URL');
 assert.match(html,/hreflang=/,'localized discovery');
 if(staging) assert.match(response.headers.get('x-robots-tag')??'',/noindex/);
 console.log(`PASS product ${path} language=${lang}`);
}
for(const path of ['/robots.txt','/sitemap.xml','/llms.txt','/agent.json','/guide/zh-CN.md','/guide/ja.md','/guide/en.md']) {
 const response=await get(product,path); assert.equal(response.status,200,`discovery ${path}`);
 const body=await response.text(); assert.ok(body.length>20,`nonempty ${path}`);
 if(path==='/agent.json') JSON.parse(body);
 if(path==='/robots.txt'&&staging) assert.match(body,/Disallow:\s*\//);
 console.log(`PASS discovery ${path}`);
}
const page=await get(workspace,'/'); assert.equal(page.status,200,'workspace');
assert.match(page.headers.get('content-type')??'',/text\/html/);
assert.ok((await page.text()).length>1000,'workspace shell');
const session=await get(workspace,'/web/session'); assert.ok([200,401].includes(session.status),'anonymous session surface');
const status=await session.json(); assert.ok(typeof status==='object'&&status!==null,'typed anonymous session response');
console.log('PASS workspace guest HTML/session surfaces');
console.log('Public smoke does not substitute for rendered mobile or signed-in journeys.');
