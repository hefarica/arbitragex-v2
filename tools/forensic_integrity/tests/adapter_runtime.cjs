// Deliberately synthetic UNIT vectors. This is not a production browser trace.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const a = require(path.resolve(process.argv[2]));
let count = 0;
function test(name, fn) { return Promise.resolve().then(fn).then(() => {count++;console.log('PASS '+name);}); }
(async()=>{
 await test('sorted keys',()=>assert.equal(a.canonicalJson({z:0,a:'x'}),'{"a":"x","z":0}'));
 await test('zero preserved',()=>assert.equal(a.canonicalJson(0),'0'));
 await test('null preserved',()=>assert.equal(a.canonicalJson(null),'null'));
 for(const [name,val] of [['NaN',NaN],['Infinity',Infinity],['float',1.3],['unsafe',9007199254740992],['undefined',undefined],['date',new Date()],['surrogate','\ud800'],['non-ascii-key',{'é':1}],['sparse',new Array(2)]]){
  await test('reject '+name,()=>assert.throws(()=>a.canonicalJson(val)));
 }
 await test('decimal string',()=>assert.equal(a.canonicalJson({amount:'1000000000000000001'}),'{"amount":"1000000000000000001"}'));
 await test('unicode values',()=>assert.equal(a.canonicalJson({a:'caña 🧪'}),'{"a":"caña 🧪"}'));
 await test('known hash',async()=>assert.equal(await a.sha256({}), '44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a'));
 await test('drop detected',()=>assert.deepEqual(a.compareFields({net:'-3'}, {}, ['net']),[{field:'net',issue:'dropped'}]));
 await test('nullification detected',()=>assert.equal(a.compareFields({net:'-3'},{net:null},['net'])[0].issue,'nullified'));
 await test('invented zero detected',()=>assert.equal(a.compareFields({net:null},{net:0},['net'])[0].issue,'invented'));
 await test('alias explicit',()=>assert.equal(a.compareFields({id:'x'},{event_id:'x'},['id'],{id:'event_id'}).length,0));
 const r=await a.makeReceipt('unit-event','ingested','a'.repeat(64),{amount:'1',loss:'-1'},1000);
 await test('receipt payload hash',async()=>assert.equal(r.output_hash,await a.sha256(r.payload)));
 const next=await a.makeReceipt('unit-event','evaluated','a'.repeat(64),{amount:'1',loss:'-1'},1001,r);
 await test('receipt linked',()=>{assert.equal(next.input_hash,r.output_hash);assert.equal(next.parent_hash,r.receipt_hash);});
 await test('cross-context rejected',()=>assert.rejects(a.makeReceipt('unit-event','persisted','b'.repeat(64),{},1002,next)));
 if(process.argv[3]) fs.writeFileSync(process.argv[3],JSON.stringify({r,next,unicode_hash:await a.sha256({a:'caña 🧪',empty:[],nil:null,z:0})},null,2));
 console.log(`TOTAL ${count} passed; scope=UNIT_SYNTHETIC`);
})().catch(e=>{console.error(e);process.exitCode=1});
