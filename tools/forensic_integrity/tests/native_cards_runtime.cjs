// Test-only producer fixtures. No market data, no production certification.
const assert = require('node:assert/strict');
const {runRealCardPipeline, presentRealField} = require(process.argv[2]);
const H='a'.repeat(64);const t=1800000000000;
const context={event_id:'TEST_ONLY',strategy_id:'BASE:triangular',context_id:H,snapshot_hash:H,
 config_hash:H,snapshot_generated_at_ms:t,pricebus:{schema:'arbx.pricebus.export.v1',source:'canonical_pricebus',snapshot_hash:H,scope:'TEST_ONLY_fixture'},rejection_reason:'non_positive_profit'};
const policy={now:()=>t,deadline_ms:500,per_field_timeout_ms:30,concurrency:2,max_clock_skew_ms:0,snapshot_max_age_ms:100};
const req=(name,applicable=true)=>({name,unit:'USD',applicable,applicability_rule:'TEST_rule'});
const val=(v)=>({state:'computed',value:v,unit:'USD',context_id:H,valid_until_ms:t+100,
 source:{kind:'calculation',reference:'TEST_ONLY',as_of_ms:t,input_hashes:[H],transform_id:'TEST_transform',transform_version:'1'}});
const prod=(name,v,deps=[])=>({field:name,depends_on:deps,compute:async()=>val(v)});
let passed=0;
async function test(name,f){await f();passed++;console.log('PASS '+name);}
(async()=>{
 await test('complete numbers',async()=>{const o=await runRealCardPipeline(context,[req('net')],[prod('net','-3')],policy);assert.equal(o.completeness,'complete');assert.equal(o.fields.net.value,'-3');});
 await test('all applicable fields run on rejected opportunity',async()=>{let n=0;const o=await runRealCardPipeline(context,[req('a'),req('b')],[{...prod('a','1'),compute:async()=>{n++;return val('1')}},{...prod('b','2',['a']),compute:async()=>{n++;return val('2')}}],policy);assert.equal(n,2);assert.equal(o.context.rejection_reason,'non_positive_profit')});
 await test('missing producer is repair',async()=>{const o=await runRealCardPipeline(context,[req('net')],[],policy);assert.equal(o.completeness,'repair_required');assert.equal(o.fields.net.value,null);});
 await test('real zero preserved',async()=>{const o=await runRealCardPipeline(context,[req('net')],[prod('net','0')],policy);assert.equal(presentRealField(o,'net',t).text,'0')});
 await test('placeholder rejected',async()=>{const o=await runRealCardPipeline(context,[req('net')],[prod('net','no computado')],policy);assert.equal(o.completeness,'repair_required')});
 await test('float USD rejected',async()=>{const o=await runRealCardPipeline(context,[req('net')],[prod('net',0.5)],policy);assert.equal(o.completeness,'repair_required')});
 await test('NA requires rule reason',async()=>{const o=await runRealCardPipeline(context,[req('flash',false)],[],policy);assert.equal(o.completeness,'repair_required')});
 await test('genuine NA explicit',async()=>{const o=await runRealCardPipeline(context,[{...req('flash',false),not_applicable_reason:'own_capital'}],[],policy);assert.equal(o.completeness,'complete');assert.equal(presentRealField(o,'flash',t).text,'No aplica')});
 await test('applicable NA rejected',async()=>{const o=await runRealCardPipeline(context,[req('net')],[{...prod('net','2'),compute:async()=>({state:'not_applicable',unit:'USD',value:null,reason:'no_data'})}],policy);assert.equal(o.completeness,'repair_required')});
 await test('cross opportunity context rejected',async()=>{const o=await runRealCardPipeline(context,[req('net')],[{...prod('net','2'),compute:async()=>({...val('2'),context_id:'b'.repeat(64)})}],policy);assert.equal(o.fields.net.reason,'cross_context_value')});
 await test('timeout bounded and explicit',async()=>{const o=await runRealCardPipeline(context,[req('net')],[{...prod('net','2'),compute:async()=>new Promise(()=>{})}],policy);assert.equal(o.fields.net.reason,'producer_timeout')});
 await test('dependency failure propagated',async()=>{const o=await runRealCardPipeline(context,[req('a'),req('b')],[prod('b','2',['a'])],policy);assert.equal(o.fields.b.reason,'applicable_upstream_failed')});
 await test('missing manifest dependency',async()=>{const o=await runRealCardPipeline(context,[req('b')],[prod('b','2',['a'])],policy);assert.equal(o.fields.b.reason,'dependency_not_in_manifest')});
 await test('cyclic graph rejected',async()=>{const o=await runRealCardPipeline(context,[req('a'),req('b')],[prod('a','1',['b']),prod('b','2',['a'])],policy);assert.equal(o.fields.a.reason,'dependency_cycle_or_unresolved')});
 await test('snapshot age cannot be hidden',async()=>{const o=await runRealCardPipeline({...context,snapshot_generated_at_ms:t-101},[req('net')],[prod('net','2')],policy);assert.equal(o.completeness,'repair_required')});
 await test('no signers or certificate',async()=>{const o=await runRealCardPipeline(context,[req('net')],[prod('net','2')],policy);assert.equal(o.execution_authorized,false);assert.equal(o.source_authenticity_verified,false)});
 await test('fresh render guard',async()=>{const o=await runRealCardPipeline(context,[req('net')],[prod('net','2')],policy);assert.equal(presentRealField(o,'net',t+101).text,'Error de datos')});
 await test('collision rejected',async()=>{await assert.rejects(()=>runRealCardPipeline(context,[req('net')],[prod('net','2'),prod('net','3')],policy),/duplicate/)});
 await test('undeclared producer rejected',async()=>{await assert.rejects(()=>runRealCardPipeline(context,[req('net')],[prod('x','2')],policy),/not_in_field_manifest/)});
 await test('errors do not leak secrets',async()=>{const o=await runRealCardPipeline(context,[req('net')],[{...prod('net','2'),compute:async()=>{throw Error('SENSITIVE_TEST_STRING')}}],policy);assert.ok(!JSON.stringify(o).includes('SENSITIVE_TEST_STRING'))});
 await test('noncanonical price feed refused',async()=>{await assert.rejects(()=>runRealCardPipeline({...context,pricebus:{source:'other_feed'}},[req('net')],[prod('net','1')],policy),/canonical_pricebus/)});
 await test('out-of-order manifest is not a false cycle',async()=>{const o=await runRealCardPipeline(context,[req('b'),req('a')],[prod('b','2',['a'])],policy);assert.equal(o.fields.b.reason,'applicable_upstream_failed')});
 console.log(JSON.stringify({tests:passed,failed:0,scope:'synthetic_native_pipeline_unit_tests'}));
})().catch(e=>{console.error(e);process.exitCode=1});
