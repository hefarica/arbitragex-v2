// Tests use explicit fixture rows and recording adapters, never production services.
const assert = require('node:assert/strict');
const test = require('node:test');
const path = require('node:path');
const build = process.env.CATALOG_BUILD_DIR;
if (!build) throw new Error('CATALOG_BUILD_DIR required');
const core = require(path.join(build, 'backend/api-server/src/routes/liquidity-catalog-core.js'));
const {serveLiquidityCatalog} = require(path.join(build, 'backend/api-server/src/routes/liquidity-catalog.js'));
const model = require(path.join(build, 'frontend/app/dex-registry/liquidity-catalog-model.js'));
const root = () => core.parseCatalogQuery({});
const chain = (id='1') => ({id,chain_id:id,label:`Chain ${id}`,active:true,registered:true,dex_count:'2',factory_count:'3',pool_count:'40'});
const uid = '00000000-0000-4000-8000-000000000001';
const when = '2026-09-12T00:00:00.000Z';
const scope = {level:'chains',chainId:null,dexId:null,search:''};
test('defaults are bounded, no implicit mainnet scope', () => assert.deepEqual(root(), {level:'chains',chainId:null,dexId:null,after:null,limit:25,search:''}));
for (const bad of ['0','-1','1.5','1e3','01','0x1','9223372036854775808',' 1', '', ['1'], {id:'1'}]) {
  test(`reject malformed chain ${JSON.stringify(bad)}`, () => assert.throws(() => core.parseCatalogQuery({level:'pools',chain_id:bad})));
}
test('large chain ids never round through Number', () => assert.equal(core.parseCatalogQuery({level:'dexes',chain_id:'9007199254740993'}).chainId, '9007199254740993'));
test('maximum PostgreSQL bigint supported', () => assert.equal(core.chainIdentity('9223372036854775807'),'9223372036854775807'));
for (const bad of ['0','101','2.5','1e2',' 2','-1', ['25']]) {
  test(`reject limit ${JSON.stringify(bad)}`, () => assert.throws(() => core.parseCatalogQuery({limit:bad})));
}
test('chain scope required for child levels', () => assert.throws(() => core.parseCatalogQuery({level:'dexes'})));
test('unknown level rejected', () => assert.throws(() => core.parseCatalogQuery({level:'broadcast'})));
test('ambiguous scope rejected', () => assert.throws(() => core.parseCatalogQuery({chain_id:'1'})));
test('dex scope rejected outside pools', () => assert.throws(() => core.parseCatalogQuery({level:'dexes',chain_id:'1',dex_id:uid})));
test('injected cursor rejected', () => assert.throws(() => core.parseCatalogQuery({after:'1;DROP TABLE pools'})));
test('search length and controls bounded', () => {for(const q of ['x'.repeat(101),'a\0b']) assert.throws(() => core.parseCatalogQuery({q}));});
test('search uses bound SQL arguments and literal wildcard escaping', () => {
  const input="x%' OR true --_\\";
  const sql=core.catalogSql(core.parseCatalogQuery({q:input}));
  assert.ok(!sql.text.includes(input)); assert.equal(sql.values[1],"%x\\%' OR true --\\_\\\\%");
});
for(const level of ['chains','dexes','pools']) test(`query ${level} is read-only, keyset paginated`, () => {
  const q = core.parseCatalogQuery(level==='chains'?{limit:'2'}:{level,chain_id:'1',limit:'2'});
  const sql = core.catalogSql(q);
  assert.ok(!/\b(DELETE|UPDATE|TRUNCATE|INSERT|DROP)\b/i.test(sql.text));
  assert.match(sql.text, /ORDER BY/); assert.match(sql.text, /LIMIT \$/);
  assert.equal(sql.values.at(-1),3);
});
test('pool joins enforce factory and token chain consistency', () => {
  const sql=core.catalogSql(core.parseCatalogQuery({level:'pools',chain_id:'1'})).text;
  for(const join of ['f.chain_id=p.chain_id','t0.chain_id=p.chain_id','t1.chain_id=p.chain_id']) assert.ok(sql.includes(join));
});
test('database projection excludes secrets and does not claim LIVE', () => {
  const row={...chain(),rpc_http_url:'secret',notes:'confidential',private_key:'redacted-fixture'};
  const page=core.catalogPage(root(),[row],when);
  assert.equal(page.execution_verified,false);
  assert.ok(!JSON.stringify(page).includes('secret')); assert.ok(!('notes' in page.items[0]));
});
test('limit+one creates a continuation cursor without inflating page count', () => {
  const q=core.parseCatalogQuery({limit:'1'});const page=core.catalogPage(q,[chain('1'),chain('10')],when);
  assert.equal(page.count,1);assert.equal(page.next_after,'1');
});
test('final and empty pages have no continuation', () => {
  assert.equal(core.catalogPage(root(),[],when).next_after,null);
  assert.equal(core.catalogPage(root(),[chain()],when).next_after,null);
});
test('invalid or missing counts are not zero-filled', () => {for(const count of [undefined,null,2,'NaN','-1']) assert.throws(()=>core.catalogPage(root(),[{...chain(),pool_count:count}],when));});
test('out of scope rows rejected', () => assert.throws(() => core.catalogPage(core.parseCatalogQuery({level:'dexes',chain_id:'10'}),[{id:uid,chain_id:'1',label:'DEX',active:true,protocol_type:'V2',factory_count:'1',pool_count:'0'}],when)));
test('frontend accepts exact backend contract', () => assert.equal(model.readCatalog(core.catalogPage(root(),[chain()],when),scope).count,1));
test('frontend rejects legacy pool payload', () => assert.throws(() => model.readCatalog({count:0,chain_id:1,items:[]},scope)));
test('frontend rejects fabricated LIVE status', () => assert.throws(() => model.readCatalog({...core.catalogPage(root(),[],when),execution_verified:true},scope)));
test('frontend rejects stale scope', () => assert.throws(() => model.readCatalog(core.catalogPage(root(),[],when),{...scope,search:'new'})));
test('frontend rejects malformed and duplicate identities', () => {
  const page=core.catalogPage(root(),[chain()],when); page.items.push(page.items[0]);page.count=2;
  assert.throws(()=>model.readCatalog(page,scope));
});
test('frontend cursor must be last displayed identity', () => assert.throws(()=>model.readCatalog({...core.catalogPage(root(),[chain()],when),next_after:'10'},scope)));
test('URL carries full query and exact chain identity', () => {
  const url=model.catalogUrl('',{level:'pools',chainId:'9007199254740993',dexId:uid,search:'A/B + C'},uid);
  const parsed=new URL(url,'http://local.test');
  assert.equal(parsed.pathname,'/api/v1/pools'); assert.equal(parsed.searchParams.get('view'),'liquidity_catalog');
  assert.equal(parsed.searchParams.get('chain_id'),'9007199254740993');assert.equal(parsed.searchParams.get('q'),'A/B + C');
});
function harness(fail='') {
  const calls=[];let released=0;let discarded=false;let code;let payload;
  const client={async query(sql) {calls.push(sql); if(fail==='query'&&sql.startsWith('WITH ids'))throw new Error('secret_connection_string');if(fail==='rollback'&&(sql.startsWith('WITH ids')||sql==='ROLLBACK'))throw new Error('db_error');return {rows:sql.startsWith('WITH ids')?[chain()]:[]};},release(error){released++;discarded=!!error;}};
  const res={setHeader(){},status(value){code=value;return this;},json(value){payload=value;}};
  const deps={pool:{async connect(){if(fail==='connect')throw new Error('secret');return client;}},logger:{warn(){}}};
  return {calls,res,deps,get released(){return released},get discarded(){return discarded},get code(){return code},get payload(){return payload}};
}
test('handler executes a bounded read-only transaction and releases', async()=>{
  const h=harness();await serveLiquidityCatalog({query:{}},h.res,h.deps);
  assert.equal(h.code,200);assert.equal(h.released,1);assert.equal(h.calls[0],'BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY');
  assert.ok(h.calls.includes("SET LOCAL statement_timeout = '4000ms'"));assert.equal(h.calls.at(-1),'COMMIT');
});
test('invalid query performs no DB work',async()=>{const h=harness();await serveLiquidityCatalog({query:{level:'bad'}},h.res,h.deps);assert.equal(h.code,400);assert.equal(h.calls.length,0)});
test('unconfigured database is 503, not an empty success',async()=>{const h=harness();h.deps.pool=null;await serveLiquidityCatalog({query:{}},h.res,h.deps);assert.equal(h.code,503)});
for(const reason of ['query','connect','rollback']) test(`failure ${reason} is sanitized and connections handled`,async()=>{
  const h=harness(reason);await serveLiquidityCatalog({query:{}},h.res,h.deps);assert.equal(h.code,503);assert.deepEqual(h.payload,{error:'catalog_unavailable'});
  assert.equal(h.released,reason==='connect'?0:1);if(reason!=='connect')assert.equal(h.calls.at(-1),'ROLLBACK');if(reason==='rollback')assert.equal(h.discarded,true);
});

// Adversarial client contract cases: malformed successes must remain errors.
for (const [name, mutate] of [
  ['missing timestamp', p => { delete p.observed_at; }],
  ['invalid timestamp', p => { p.observed_at = 'not-a-date'; }],
  ['missing inactive policy', p => { delete p.counts_include_inactive; }],
  ['missing limit', p => { delete p.limit; }],
  ['fractional limit', p => { p.limit = 1.5; }],
  ['inconsistent count', p => { p.count = 0; }],
  ['blank label', p => { p.items[0].label = ' '; }],
  ['invalid boolean', p => { p.items[0].active = 'true'; }],
  ['absent count', p => { delete p.items[0].pool_count; }],
  ['numeric count', p => { p.items[0].pool_count = 40; }],
]) test('frontend refuses '+name, () => {
  const p=core.catalogPage(root(),[chain()],when); mutate(p);
  assert.throws(()=>model.readCatalog(p,scope));
});
test('frontend refuses descending keyset order',()=>{
  const p=core.catalogPage(root(),[chain('10'),chain('1')],when);
  assert.throws(()=>model.readCatalog(p,scope));
});
test('frontend refuses repeated cursor pages',()=>{
  const p=core.catalogPage(root(),[chain('1')],when);
  assert.throws(()=>model.readCatalog(p,scope,'1'));
});
test('frontend keyset order stays exact beyond Number precision',()=>{
  const p=core.catalogPage(root(),[chain('9007199254740993'),chain('9007199254740994')],when);
  assert.equal(model.readCatalog(p,scope,'9007199254740992').count,2);
});
test('frontend rejects incomplete pool identity',()=>{
  const q=core.parseCatalogQuery({level:'pools',chain_id:'1',dex_id:uid});
  const wanted={level:'pools',chainId:'1',dexId:uid,search:''};
  const item={id:uid,chain_id:'1',label:'Pool',active:true,pool_address:'0x0000000000000000000000000000000000000001',
    dex_id:uid,dex_name:'DEX',dex_active:true,protocol_type:'V2',factory_address:'0x0000000000000000000000000000000000000002',
    fee_tier:null,token0_address:null,token0_symbol:null,token1_address:null,token1_symbol:null};
  const p=core.catalogPage(q,[item],when);
  assert.equal(model.readCatalog(p,wanted).count,1);
  for(const key of ['pool_address','factory_address','dex_name','protocol_type']) {
    const bad=structuredClone(p);bad.items[0][key]='';assert.throws(()=>model.readCatalog(bad,wanted));
  }
});
const {acquireCatalogConnection}=require(path.join(build,'backend/api-server/src/routes/liquidity-catalog.js'));
test('acquisition timeout releases the eventual late client exactly once',async()=>{
  let finish;let released=0;
  const waiting=new Promise(resolve=>{finish=resolve;});
  await assert.rejects(acquireCatalogConnection({connect:()=>waiting},5),/catalog_acquire_timeout/);
  finish({release(){released++;}});await new Promise(resolve=>setImmediate(resolve));
  assert.equal(released,1);
});
test('acquisition success does not release a client still in use',async()=>{
  let released=0;const client={release(){released++;}};
  assert.equal(await acquireCatalogConnection({connect:async()=>client},5),client);
  await new Promise(resolve=>setTimeout(resolve,10));assert.equal(released,0);
});
test('late failed acquisition is handled without leaking driver details',async()=>{
  let fail;const waiting=new Promise((_,reject)=>{fail=reject;});
  await assert.rejects(acquireCatalogConnection({connect:()=>waiting},5),/catalog_acquire_timeout/);
  fail(new Error('sensitive-driver-detail'));await new Promise(resolve=>setImmediate(resolve));
});
