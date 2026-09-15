import { describe, it, expect, beforeAll, beforeEach, afterAll } from "vitest";
import { GenericContainer, Wait, type StartedTestContainer } from "testcontainers";
import { Pool } from "pg";
import express from "express";
import request from "supertest";
import { buildViableKpisRouter } from "../src/routes/viable-kpis.js";

let container: StartedTestContainer;
let pool: Pool;
beforeAll(async () => {
  container = await new GenericContainer("postgres:15")
    .withEnvironment({ POSTGRES_PASSWORD: "isolated-fixture", POSTGRES_DB: "hop_test" })
    .withExposedPorts(5432)
    .withWaitStrategy(Wait.forLogMessage("database system is ready to accept connections", 2)).start();
  pool = new Pool({host:container.getHost(),port:container.getMappedPort(5432),user:"postgres",password:"isolated-fixture",database:"hop_test"});
  await pool.query(`CREATE TABLE opportunities (
    chain_id integer DEFAULT 1, chain_id_out integer,
    status text, rejection_reason text, strategy_kind text,
    route_metadata jsonb, detected_at timestamptz DEFAULT now()
  )`);
}, 90000);
afterAll(async()=>{ await pool?.end(); await container?.stop(); });
beforeEach(async()=>{ await pool.query("TRUNCATE opportunities"); });
function metadata(hops:number) {
  return {dex_adapters:Array(hops).fill("unit-v2"),token_addresses:[...Array.from({length:hops},(_,i)=>`token-${i}`),"token-0"],pool_addresses:Array.from({length:hops},(_,i)=>`pool-${i}`)};
}
async function insert(route:unknown,status="detected",reason:string|null=null) {
  await pool.query("INSERT INTO opportunities(status,rejection_reason,strategy_kind,route_metadata) VALUES($1,$2,'unit-only',$3)",[status,reason,JSON.stringify(route)]);
}
async function read() {
  const app=express();app.use(buildViableKpisRouter(pool));
  return request(app).get("/api/v1/analytics/viable-kpis?hours=1");
}
describe("HOPS-PROVENANCE — real PostgreSQL aggregation",()=>{
  it("counts complete 2/3/4/5 by token/adapter topology consistently with the view model",async()=>{
    for(const h of [2,3,4,5])await insert(metadata(h));
    const r=await read();expect(r.status).toBe(200);
    expect(r.body.data.by_hops).toEqual([2,3,4,5].map(hops=>({hops,n:1})));
    expect(r.body.data.totals).toEqual({total:4,viable:4,routed:4,viability_pct:100});
  });
  it("does not count unresolved pools as complete routes or invent fewer swaps",async()=>{
    for(const h of [2,3,4,5]) {
      const r=metadata(h);
      await insert({...r,pool_addresses:r.pool_addresses.slice(1)});
    }
    await insert({...metadata(3),pool_addresses:"not-an-array"});
    const r=await read();expect(r.status).toBe(200);
    expect(r.body.data.by_hops).toEqual([]);
    expect(r.body.data.totals).toEqual({total:5,viable:5,routed:0,viability_pct:100});
    // viable is pipeline lifecycle only, NOT a declaration of executability.
  });
  it("excludes explicit rejections even when a stale status says detected",async()=>{
    await insert(metadata(4),"detected","TokenNotAllowed:unit-only");
    await insert(metadata(3),"rejected",null);
    await insert(metadata(5));
    const r=await read();expect(r.status).toBe(200);
    expect(r.body.data.by_hops).toEqual([{hops:5,n:1}]);
    expect(r.body.data.totals).toEqual({total:3,viable:1,routed:1,viability_pct:33.3});
    expect(r.body.data.by_kind).toEqual([{strategy_kind:"unit-only",n:1}]);
  });
  it("malformed/scalar/missing topology produces no invented hops and no SQL crash",async()=>{
    for(const r of [{},{dex_adapters:"bad",token_addresses:[]},{dex_adapters:["v2"],token_addresses:"bad"},{dex_adapters:["v2","v2"],token_addresses:["a","b"]},null])await insert(r);
    const r=await read();expect(r.status).toBe(200);
    expect(r.body.data.by_hops).toEqual([]);
    expect(r.body.data.totals.routed).toBe(0);
    expect(r.body.data.totals.total).toBe(5);
  });
  it("an empty window reports null viability, not fabricated success",async()=>{
    const r=await read();expect(r.status).toBe(200);
    expect(r.body.data.totals).toEqual({total:0,viable:0,routed:0,viability_pct:null});
  });
});


describe("PR566 review — metadata element parity with the canonical ViewModel", () => {
  for (const field of ["token_addresses", "pool_addresses", "dex_adapters"] as const) {
    for (const invalid of [null, 42, true, {}, "", " ", "\t\n", "\u00a0", "\ufeff"]) {
      it(`${field} rejects ${JSON.stringify(invalid)} without inventing a complete route`, async () => {
        const valid = metadata(3);
        const values: unknown[] = [...valid[field]];
        values[1] = invalid;
        await insert({ ...valid, [field]: values });
        await insert(metadata(5));
        const res = await read();
        expect(res.status).toBe(200);
        expect(res.body.data.by_hops).toEqual([{hops: 5, n: 1}]);
        expect(res.body.data.totals).toEqual({total: 2, viable: 2, routed: 1, viability_pct: 100});
      });
    }
  }
});

describe("PR569 single-chain cycle closure matches the frontend", () => {
  it.each([2, 3, 4, 5])("does not credit an open %i-hop path as routed", async (hops) => {
    const route = metadata(hops);
    route.token_addresses[hops] = "another-token";
    await insert(route);
    await insert(metadata(hops));
    const result = await read();
    expect(result.status).toBe(200);
    expect(result.body.data.by_hops).toEqual([{ hops, n: 1 }]);
    expect(result.body.data.totals).toEqual({ total: 2, viable: 2, routed: 1, viability_pct: 100 });
  });
  it("compares cycle endpoints without case drift", async () => {
    const route = metadata(3);
    route.token_addresses[3] = "TOKEN-0";
    await insert(route);
    expect((await read()).body.data.by_hops).toEqual([{ hops: 3, n: 1 }]);
  });
  it("retains genuine cross-chain open paths", async () => {
    const route = metadata(3);
    route.token_addresses[3] = "other-chain-token";
    await pool.query("INSERT INTO opportunities(status,strategy_kind,route_metadata,chain_id,chain_id_out) VALUES('detected','cross_chain',$1,1,42161)", [JSON.stringify(route)]);
    expect((await read()).body.data.by_hops).toEqual([{ hops: 3, n: 1 }]);
  });
});
