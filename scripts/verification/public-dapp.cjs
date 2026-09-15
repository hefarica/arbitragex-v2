'use strict';
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const {createRequire}=require('node:module');
const fromFrontend=createRequire(path.resolve('frontend/package.json'));
const {chromium,expect}=fromFrontend('@playwright/test');
const {safeHttp,safeEnginePacket,verified}=require('./read-only-policy.cjs');
const {readCatalog,catalogUrl}=require(path.join(process.env.CATALOG_BUILD_DIR,'frontend/app/dex-registry/liquidity-catalog-model.js'));
const origin=new URL(process.env.DAPP_ORIGIN||'').origin;
assert.equal(new URL(origin).protocol,'https:','Live audit requires the explicit HTTPS origin');
const output=path.resolve(process.env.EVIDENCE_DIR||'public-evidence');fs.mkdirSync(output,{recursive:true});
const report={started_at:new Date().toISOString(),origin,source_sha:process.env.SOURCE_SHA||null,
  expected_served_sha:process.env.EXPECTED_SERVED_SHA||null,checks:{},pages:[],api:[],blocked_requests:[],
  page_errors:[],http_errors:[],socket:{connections:0,handshakes:0,events:{},blocked_frames:0},
  catalog_samples:[],opportunity_coverage:{},execution_verified:false,persistence_verified:false};
const required=['status_before','pages','readiness_contracts','catalog_api','catalog_ui','socket','opportunity_sample','status_after','stable_served_version'];
const pending=[];
async function check(id,fn){try{await fn();report.checks[id]={status:'passed'};}catch(e){
  report.checks[id]={status:'failed',error:String(e.message).replace(/([?&](?:token|key|secret|auth)[^=]*=)[^&\s]+/gi,'$1[REDACTED]').slice(0,1600)};
}}
function inbound(data){
  if(typeof data!=='string')return;
  for(const frame of data.split('\x1e')){
    if(/^40(?:\{|$)/.test(frame))report.socket.handshakes++;
    if(frame.startsWith('42['))try{const event=JSON.parse(frame.slice(2))[0];
      if(typeof event==='string'&&/^[\w:.-]{1,80}$/.test(event))report.socket.events[event]=(report.socket.events[event]||0)+1;
    }catch{/* Malformed frames are not accepted as event evidence. */}
  }
}
async function main(){
 const browser=await chromium.launch({headless:true});
 const context=await browser.newContext({viewport:{width:1600,height:1000},serviceWorkers:'block'});
 context.setDefaultTimeout(15000);
 await context.route('**/*',async route=>{
   const request=route.request();
   if(safeHttp(request.method(),request.url(),request.postData(),origin))return route.continue();
   report.blocked_requests.push({method:request.method(),path:new URL(request.url()).pathname});
   return route.abort('blockedbyclient');
 });
 await context.routeWebSocket('**/*',async socket=>{
   const target=new URL(socket.url());
   if(target.host!==new URL(origin).host||target.pathname!=='/socket.io/'){
     report.socket.blocked_frames++;socket.close();return;
   }
   report.socket.connections++;
   const server=socket.connectToServer();
   socket.onMessage(message=>{if(safeEnginePacket(message))server.send(message);else report.socket.blocked_frames++;});
   server.onMessage(message=>{inbound(message);socket.send(message);});
 });
 const page=await context.newPage();
 page.on('pageerror',e=>report.page_errors.push(e.name+': '+e.message.slice(0,200)));
 page.on('response',r=>{
   const u=new URL(r.url());
   if(u.origin===origin&&u.pathname.startsWith('/api/')&&r.status()>=400)
     report.http_errors.push({path:u.pathname,status:r.status()});
   if(u.origin===origin&&u.pathname==='/socket.io/'&&r.request().method()==='GET')
     pending.push(r.text().then(inbound).catch(()=>{}));
 });
 async function get(endpoint){const t=Date.now();const r=await context.request.get(origin+endpoint,{timeout:20000});
   report.api.push({path:endpoint,status:r.status(),duration_ms:Date.now()-t});
   assert.equal(r.status(),200,endpoint+' must return HTTP 200');return r.json();}
 async function status(){const value=await get('/api/status');assert.match(value?.deploy?.sha||'',/^[0-9a-f]{40}$/);
   return {sha:value.deploy.sha,id:value.deploy.id,at:value.deploy.at};}
 async function catalog(scope,after=null,limit=null){
   let endpoint=catalogUrl('',scope,after);if(limit!==null)endpoint=endpoint.replace('limit=25','limit='+limit);
   return readCatalog(await get(endpoint),scope,after);
 }
 const root={level:'chains',chainId:null,dexId:null,search:''};let rootPage=null,selected=null;
 await check('status_before',async()=>{report.before=await status();});
 await check('pages',async()=>{
   for(const url of ['/','/live-readiness','/chains','/dex-registry','/pools','/opportunities/exchange']){
     const t=Date.now(),response=await page.goto(origin+url,{waitUntil:'domcontentloaded',timeout:60000});
     const entry={path:url,http:response?.status()||0,started_at:new Date(t).toISOString(),navigation_ms:Date.now()-t};
     report.pages.push(entry);assert.equal(entry.http,200,url+' document');
     await expect(page.locator('main')).toBeVisible();
     await page.screenshot({path:path.join(output,(url==='/'?'home':url.slice(1).replaceAll('/','-'))+'.png'),fullPage:true});
   }
   assert.equal(report.pages.length,6);assert.deepEqual(report.page_errors,[]);
 });
 await check('readiness_contracts',async()=>{
   const chains=await get('/api/chains');assert.ok(Array.isArray(chains.data)&&chains.data.length>0,'chains.data required');
   const score=await get('/api/scoring/status');assert.ok(typeof score.scoring_status==='string');
   const risk=await get('/api/risk/circuit-breakers/status');assert.equal(risk.breakers?.length,10);
   report.readiness={scoring:score.scoring_status,circuit:risk.overall_state,
     breaker_states:risk.breakers.map(b=>({id:b.id,state:b.state})),live_trading:risk.live_trading};
 });
 await check('catalog_api',async()=>{
   rootPage=await catalog(root);assert.ok(rootPage.items.length>0,'No chains available to test');
   for(const chain of rootPage.items.slice(0,3)){
     const scope={level:'dexes',chainId:chain.chain_id,dexId:null,search:''};
     const dexes=await catalog(scope);const sample={chain_id:chain.chain_id,dex_count:dexes.count,pools:[]};
     report.catalog_samples.push(sample);
     for(const dex of dexes.items.slice(0,2)){
       const poolScope={level:'pools',chainId:chain.chain_id,dexId:dex.id,search:''};
       const first=await catalog(poolScope,null,1);
       sample.pools.push({dex_id:dex.id,count:first.count,ids:first.items.map(p=>p.id)});
       if(first.next_after)await catalog(poolScope,first.next_after,1);
       if(first.items.length&&!selected)selected={chain,dex,pool:first.items[0]};
     }
   }
   assert.ok(selected,'No populated chain/DEX/pool path was exercised');
 });
 await check('catalog_ui',async()=>{
   assert.ok(selected,'catalog_api did not provide a populated path');
   const panel=page.getByTestId('liquidity-catalog');
   async function transition(action,level){
     const responsePromise=page.waitForResponse(r=>{const u=new URL(r.url());return u.origin===origin&&u.pathname==='/api/v1/pools'&&
       u.searchParams.get('view')==='liquidity_catalog'&&u.searchParams.get('level')===level;},{timeout:20000});
     await action();const r=await responsePromise;assert.equal(r.status(),200);const u=new URL(r.url());
     const scope={level,chainId:u.searchParams.get('chain_id'),dexId:u.searchParams.get('dex_id'),search:u.searchParams.get('q')||''};
     const data=readCatalog(await r.json(),scope,u.searchParams.get('after'));
     await expect(panel.getByRole('alert')).toHaveCount(0);
     await expect.poll(()=>panel.getByTestId('catalog-row').evaluateAll(rows=>rows.map(row=>row.dataset.id))).toEqual(data.items.map(x=>x.id));
     return data;
   }
   await transition(()=>page.goto(origin+'/dex-registry',{waitUntil:'domcontentloaded'}),'chains');
   await transition(()=>panel.locator('[data-testid="catalog-row"][data-id="'+selected.chain.id+'"]').getByRole('button',{name:'Ver DEX',exact:true}).click(),'dexes');
   await transition(()=>panel.locator('[data-testid="catalog-row"][data-id="'+selected.dex.id+'"]').getByRole('button',{name:'Ver pools',exact:true}).click(),'pools');
   const row=panel.locator('[data-testid="catalog-row"][data-id="'+selected.pool.id+'"]');
   await row.getByText('Direcciones completas',{exact:true}).click();await expect(row.getByText('Pool ID: '+selected.pool.id,{exact:true})).toBeVisible();
   await panel.screenshot({path:path.join(output,'catalog-pool-detail.png')});
   await panel.getByLabel('Buscar en el nivel actual').fill(selected.pool.pool_address);
   const filtered=await transition(()=>panel.getByRole('button',{name:'Buscar',exact:true}).click(),'pools');
   assert.ok(filtered.items.some(p=>p.id===selected.pool.id),'Search lost the selected pool');
   await transition(()=>panel.getByRole('button',{name:'Blockchains',exact:true}).click(),'chains');
   await transition(()=>panel.getByRole('button',{name:'Blockchains',exact:true}).click(),'chains');
   await page.setViewportSize({width:390,height:844});await panel.screenshot({path:path.join(output,'catalog-mobile.png')});
   await page.setViewportSize({width:1600,height:1000});
 });
 await check('opportunity_sample',async()=>{
   const value=await get('/api/opportunities/live?limit=100');
   const rows=Array.isArray(value.items)?value.items:Array.isArray(value.opportunities)?value.opportunities:null;
   assert.ok(rows&&rows.length>0,'No real opportunity sample available');
   const buckets={2:0,3:0,4:0,5:0,other:0,unresolved:0};const states={};
   for(const row of rows){
     assert.equal(typeof row.id,'string');states[row.status]=(states[row.status]||0)+1;
     let meta=row.route_metadata;if(typeof meta==='string')try{meta=JSON.parse(meta);}catch{meta=null;}
     const adapters=meta?.dex_adapters,tokens=meta?.token_addresses,pools=meta?.pool_addresses;
     const complete=Array.isArray(adapters)&&adapters.length>0&&Array.isArray(tokens)&&tokens.length===adapters.length+1&&
       Array.isArray(pools)&&pools.length===adapters.length&&[...adapters,...tokens,...pools].every(s=>typeof s==='string'&&s.trim());
     if(!complete)buckets.unresolved++;else if(adapters.length>=2&&adapters.length<=5)buckets[adapters.length]++;else buckets.other++;
   }
   report.opportunity_coverage={sample_size:rows.length,by_hops:buckets,states,
     note:'Observed API sample only. Missing hop lengths remain not observed; this does not prove EVM execution.'};
 });
 await check('socket',async()=>{
   await expect.poll(()=>report.socket.handshakes,{timeout:20000}).toBeGreaterThan(0);
   await expect.poll(()=>['new_opportunity','route_discovery_telemetry','opportunity:detected','opportunity:validated'].reduce((sum,name)=>sum+(report.socket.events[name]||0),0),{timeout:20000}).toBeGreaterThan(0);
   assert.equal(report.socket.blocked_frames,0,'An unrecognized socket frame was blocked; coverage incomplete');
 });
 await check('status_after',async()=>{report.after=await status();});
 await check('stable_served_version',async()=>{
   assert.ok(report.before&&report.after);assert.deepEqual(report.after,report.before);
   if(report.expected_served_sha)assert.equal(report.after.sha,report.expected_served_sha);
 });
 await Promise.allSettled(pending);await context.close();await browser.close();
 report.completed_at=new Date().toISOString();report.required=required;
 report.functional_checks_passed=verified(report.checks,required)&&report.page_errors.length===0&&report.http_errors.length===0&&report.blocked_requests.length===0;
 report.release_attested=false;
 fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2));
 const files=fs.readdirSync(output).filter(f=>f!=='manifest.json').map(file=>({file,sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(output,file))).digest('hex')}));
 fs.writeFileSync(path.join(output,'manifest.json'),JSON.stringify({run_id:process.env.GITHUB_RUN_ID,attempt:process.env.GITHUB_RUN_ATTEMPT,files},null,2));
 console.log(JSON.stringify({checks:report.checks,served:report.after,socket:report.socket,coverage:report.opportunity_coverage,functional_checks_passed:report.functional_checks_passed},null,2));
 if(!report.functional_checks_passed)process.exitCode=1;
}
main().catch(e=>{report.fatal=e.name+': '+String(e.message).slice(0,1000);report.functional_checks_passed=false;
 fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2));console.error('Public audit failed before completion');process.exitCode=1;});
