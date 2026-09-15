// Disposable PostgreSQL integration. Does NOT read DATABASE_URL or the production .env.
const assert = require('node:assert/strict');
const {test,before,after} = require('node:test');
const path = require('node:path');
const {randomUUID} = require('node:crypto');
const {Pool} = require('pg');
if (process.env.CATALOG_DISPOSABLE_DB !== '1') throw new Error('Explicit disposable database opt-in required');
const address = new URL(process.env.CATALOG_TEST_DATABASE_URL || '');
if (!['localhost','127.0.0.1'].includes(address.hostname) || address.port !== '35432' || address.pathname !== '/arbx_catalog_ci') {
  throw new Error('Refusing a non-disposable database target');
}
const build = process.env.CATALOG_BUILD_DIR;
if(!build) throw new Error('CATALOG_BUILD_DIR required');
const {catalogSql,parseCatalogQuery,catalogPage} = require(path.join(build,'backend/api-server/src/routes/liquidity-catalog-core.js'));
const {serveLiquidityCatalog} = require(path.join(build,'backend/api-server/src/routes/liquidity-catalog.js'));
const schema = 'catalog_test_' + randomUUID().replaceAll('-','');
const admin = new Pool({connectionString:address.toString(),max:1});
const pool = new Pool({connectionString:address.toString(),max:2,options:`-c search_path=${schema}`});
const id = n => `00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const addr = n => `0x${String(n).padStart(40,'0')}`;
before(async()=>{
  await admin.query(`CREATE SCHEMA ${schema}`);
  await pool.query(`
    CREATE TABLE chains_runtime(chain_id bigint PRIMARY KEY,name text,enabled boolean,rpc_http_url text);
    CREATE TABLE dexes(id uuid PRIMARY KEY,name text,protocol_type text,is_active boolean);
    CREATE TABLE factories(id uuid PRIMARY KEY,dex_id uuid REFERENCES dexes(id),chain_id integer,address text);
    CREATE TABLE tokens(id uuid PRIMARY KEY,chain_id integer,address text,symbol text);
    CREATE TABLE pools(id uuid PRIMARY KEY,chain_id integer,address text,factory_id uuid REFERENCES factories(id),
      token0_id uuid REFERENCES tokens(id),token1_id uuid REFERENCES tokens(id),fee_tier int,is_active boolean);
    INSERT INTO chains_runtime VALUES (1,'Mainnet fixture',true,'https://rpc.invalid/secret'),
      (11155111,'Testnet fixture',false,'https://rpc.invalid/secret'),(9007199254740993,'Big ID',false,'');
  `);
  for(const [n,name] of [[1,'DEX_A'],[2,'DEX_B']]) await pool.query('INSERT INTO dexes VALUES($1,$2,$3,$4)',[id(n),name,'UNISWAP_V2',true]);
  for(const [n,dex,chain] of [[11,1,'1'],[12,1,'11155111'],[13,2,'11155111'],[14,1,'10']])
    await pool.query('INSERT INTO factories VALUES($1,$2,$3,$4)',[id(n),id(dex),chain,addr(n)]);
  for(const [n,chain,symbol] of [[21,'1','AAA'],[22,'1','BBB'],[23,'11155111','CCC'],[24,'11155111','DDD']])
    await pool.query('INSERT INTO tokens VALUES($1,$2,$3,$4)',[id(n),chain,addr(n%2+1),symbol]);
  for(const [n,chain,factory,t0,t1,active] of [[31,'1',11,21,22,true],[32,'1',11,21,22,false],
    [33,'11155111',12,23,24,true],[34,'11155111',13,23,24,true],
    [35,'1',12,21,22,true]]) // intentionally inconsistent factory chain: must not leak.
    await pool.query('INSERT INTO pools VALUES($1,$2,$3,$4,$5,$6,$7,$8)',[id(n),chain,addr(n),id(factory),id(t0),id(t1),3000,active]);
  // Real schema allows factories without a DEX. They are not reachable pool inventory.
  await pool.query('INSERT INTO factories VALUES($1,NULL,1,$2)',[id(15),addr(15)]);
  await pool.query('INSERT INTO pools VALUES($1,1,$2,$3,$4,$5,3000,true)',[id(36),addr(36),id(15),id(21),id(22)]);
});
after(async()=>{await pool.end();try{await admin.query(`DROP SCHEMA IF EXISTS ${schema} CASCADE`);}finally{await admin.end();}});
async function query(raw={}) {
  const q=parseCatalogQuery(raw), sql=catalogSql(q);
  const client=await pool.connect();
  try {
    await client.query('BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY');
    const result=await client.query(sql.text,sql.values);
    const response=catalogPage(q,result.rows,new Date().toISOString());
    await client.query('COMMIT');return response;
  } catch(e){await client.query('ROLLBACK');throw e;}finally{client.release();}
}
test('real PostgreSQL root aggregates all chains without credential columns',async()=>{
  const result=await query();assert.equal(result.count,4);
  assert.equal(result.items.find(r=>r.id==='1').pool_count,'2');
  assert.equal(result.items.find(r=>r.id==='11155111').dex_count,'2');
  assert.ok(!JSON.stringify(result).includes('secret'));
});
test('runtime-missing chain remains explicit unknown',async()=>{
  const row=(await query()).items.find(r=>r.id==='10');
  assert.equal(row.registered,false);assert.equal(row.active,null);
});
test('large bigint identity remains exact through driver and JSON',async()=>{
  const row=(await query()).items.find(r=>r.id==='9007199254740993');
  assert.equal(row.chain_id,'9007199254740993');assert.equal(row.pool_count,'0');
});
test('keyset pages neither repeat nor truncate known chains',async()=>{
  const one=await query({limit:'2'}), two=await query({limit:'2',after:one.next_after});
  assert.deepEqual([...one.items,...two.items].map(r=>r.id),['1','10','11155111','9007199254740993']);
  assert.equal(two.next_after,null);
});
test('DEX and pool counts are chain-specific and include inactive inventory',async()=>{
  const result=await query({level:'dexes',chain_id:'1'});
  assert.equal(result.count,1);assert.equal(result.items[0].pool_count,'2');
  const pools=await query({level:'pools',chain_id:'1',dex_id:id(1)});
  assert.equal(pools.count,2);assert.equal(pools.items[1].active,false);
  assert.ok(pools.items.every(r=>r.chain_id==='1'&&r.token0_symbol==='AAA'));
});
test('same token address on testnet keeps different identity and symbol',async()=>{
  const result=await query({level:'pools',chain_id:'11155111',dex_id:id(1)});
  assert.equal(result.count,1);assert.equal(result.items[0].token0_symbol,'CCC');
});
test('cross-chain DEX selection produces no borrowed inventory',async()=>{
  assert.equal((await query({level:'pools',chain_id:'1',dex_id:id(2)})).count,0);
});
test('real ILIKE treats percent underscore and backslash literally',async()=>{
  for(const q of ['%','\\',"' OR true --"]) assert.equal((await query({q})).count,0);
  assert.equal((await query({level:'dexes',chain_id:'1',q:'DEX_A'})).count,1);
});
test('actual handler uses the pg pool and returns current wire schema',async()=>{
  let code,body;const res={setHeader(){},status(v){code=v;return this},json(v){body=v}};
  await serveLiquidityCatalog({query:{level:'pools',chain_id:'1'}},res,{pool,logger:{warn(){}}});
  assert.equal(code,200);assert.equal(body.schema_version,1);assert.equal(body.execution_verified,false);assert.equal(body.count,2);
});
test('negative control reproduces untyped INTEGER parameter overflow',async()=>{
  await assert.rejects(pool.query('SELECT id FROM pools WHERE chain_id=$1',['9007199254740993']),
    error=>error.code==='22003');
});
test('full bigint range can drill into INTEGER pool storage without a 503',async()=>{
  for(const chain_id of ['2147483648','9007199254740993','9223372036854775807']){
    for(const level of ['dexes','pools']){
      const result=await query({level,chain_id});
      assert.equal(result.count,0);assert.equal(result.scope.chain_id,chain_id);
    }
  }
});
test('HTTP handler returns empty pool inventory for a valid large chain identity',async()=>{
  let code,body;const res={setHeader(){},status(v){code=v;return this},json(v){body=v}};
  await serveLiquidityCatalog({query:{level:'pools',chain_id:'9007199254740993'}},res,{pool,logger:{warn(){}}});
  assert.equal(code,200);assert.equal(body.count,0);assert.deepEqual(body.items,[]);
});
test('root pool count equals reachable pages and excludes orphaned factories',async()=>{
  const raw=await pool.query('SELECT count(*)::int AS n FROM pools p JOIN factories f ON f.id=p.factory_id AND f.chain_id=p.chain_id WHERE p.chain_id=1');
  assert.equal(raw.rows[0].n,3,'fixture must reproduce the old overcount');
  const root=(await query()).items.find(r=>r.id==='1');
  const first=await query({level:'pools',chain_id:'1',limit:'1'});
  const second=await query({level:'pools',chain_id:'1',limit:'1',after:first.next_after});
  assert.equal(root.pool_count,String(first.count+second.count));
  assert.deepEqual([...first.items,...second.items].map(r=>r.id),[id(31),id(32)]);
  assert.equal(second.next_after,null);
});

test('nullable DEX protocol survives real PostgreSQL and the client contract',async()=>{
  const {readCatalog}=require(path.join(build,'frontend/app/dex-registry/liquidity-catalog-model.js'));
  await pool.query('UPDATE dexes SET protocol_type=NULL WHERE id=$1',[id(1)]);
  try {
    for(const level of ['dexes','pools']) {
      const wire=await query({level,chain_id:'1'});
      const result=readCatalog(wire,{level,chainId:'1',dexId:null,search:''});
      assert.ok(result.items.length>0);
      assert.ok(result.items.every(row=>row.protocol_type===null));
      assert.equal(wire.execution_verified,false);
    }
  } finally {
    await pool.query('UPDATE dexes SET protocol_type=$1 WHERE id=$2',['UNISWAP_V2',id(1)]);
  }
});
