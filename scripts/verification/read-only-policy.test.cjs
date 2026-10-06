const test=require('node:test'), assert=require('node:assert/strict');
const {safeEnginePacket,safePollingBody,safeHttp,verified}=require('./read-only-policy.cjs');
for(const frame of ['40','2','3','5','2probe','3probe','42["subscribe:opportunities"]','42["subscribe:runtime_ack"]'])
  test('original transport/read subscription allowed: '+frame,()=>assert.equal(safeEnginePacket(frame),true));
for(const frame of ['','42["execute",{}]','42["runtime:set",{}]','42["sign",{}]','42["subscribe:admin"]','40/admin,','42[','42{}',Buffer.from('40')])
  test('unknown or mutating frame blocked: '+String(frame),()=>assert.equal(safeEnginePacket(frame),false));
test('mixed polling payload cannot hide a mutation after a valid frame',()=>{
  assert.equal(safePollingBody('40\x1e42["subscribe:opportunities"]'),true);
  assert.equal(safePollingBody('40\x1e42["execute",{}]'),false);
});
test('only the real same-origin Engine.IO polling POST is allowed',()=>{
  const origin='https://dapp.example';const path='/socket.io/?EIO=4&transport=polling&sid=opaque';
  assert.equal(safeHttp('POST',origin+path,'40',origin),true);
  for(const url of [origin+'/admin/sign', 'https://other.example'+path,origin+'/socket.io/?EIO=4&transport=websocket'])
    assert.equal(safeHttp('POST',url,'40',origin),false);
  assert.equal(safeHttp('DELETE',origin+path,'40',origin),false);
});
for(const status of [undefined,'skipped','not_run','failed','not_observed'])
  test('missing or unproved coverage cannot pass: '+status,()=>assert.equal(verified({catalog:{status}},['catalog']),false));
test('empty or duplicate required coverage cannot manufacture green',()=>{
  assert.equal(verified({},[]),false);
  assert.equal(verified({catalog:{status:'passed'}},['catalog','catalog']),false);
});
test('all exact required checks must pass',()=>{
  const checks={http:{status:'passed'},catalog:{status:'passed'},socket:{status:'passed'}};
  assert.equal(verified(checks,['http','catalog','socket']),true);
  assert.equal(verified(checks,['http','catalog','socket','served_sha']),false);
});

// ── ACK-PARSER-01: the acknowledgement id is part of the frame's STRUCTURE ──
// Socket.IO puts an optional acknowledgement id between the packet type and the
// payload: `420[...]` is an EVENT asking for an ack, not an ack. The old parser
// required the literal prefix `42[`, so it refused a valid frame before it ever
// looked at the event name.
test('an acknowledged read subscription crosses in its valid form',()=>{
  assert.equal(safeEnginePacket('420["subscribe:runtime_ack"]'),true);   // the frame the browser actually emitted
  assert.equal(safeEnginePacket('42["subscribe:runtime_ack"]'),true);    // unchanged: no ack id
  assert.equal(safeEnginePacket('4237["subscribe:prices"]'),true);       // multi-digit ack id
  assert.equal(safeEnginePacket('42999999999999["subscribe:prices"]'),true); // the id VALUE is irrelevant
});
test('recognising the syntax authorises nobody and proves no server ack',()=>{
  // The ack id is a client-chosen integer. Interpreting it says nothing about the
  // emitter's authority and nothing about whether the server ever acknowledged --
  // those remain separate checks. A valid frame shape still buys no authorisation.
  assert.equal(safeEnginePacket('420["subscribe:admin"]'),false);
  assert.equal(safeEnginePacket('420["runtime:set"]'),false);
});
test('the fix is NOT a blind prefix allowance',()=>{
  for(const frame of ['420','420anything','420{}','420x','42[' ,'420[','42{}[', '420["subscribe:prices"]extra'])
    assert.equal(safeEnginePacket(frame),false,frame);
});
for(const frame of ['42["execute"]','420["execute"]','4217["execute"]','42["sign"]','420["sign"]',
  '42["runtime:set"]','420["runtime:set"]','4217["runtime:set"]'])
  test('a write event stays blocked with or without an ack id: '+frame,()=>assert.equal(safeEnginePacket(frame),false));
for(const frame of ['42/admin,["subscribe:prices"]','420/admin,["subscribe:prices"]','42/,["subscribe:prices"]',
  '420/,["subscribe:prices"]','41/admin,'])
  test('a namespace stays blocked with or without an ack id: '+frame,()=>assert.equal(safeEnginePacket(frame),false));
for(const frame of ['42x["subscribe:prices"]','420x["subscribe:prices"]','42<0>["subscribe:prices"]',
  '420\ufeff["subscribe:prices"]','4237x["subscribe:prices"]'])
  test('a malformed ack shape stays blocked: '+frame,()=>assert.equal(safeEnginePacket(frame),false));
test('a polling body carrying several ack-bearing frames is judged frame by frame',()=>{
  assert.equal(safePollingBody('420["subscribe:runtime_ack"]\x1e4237["subscribe:prices"]\x1e40'),true);
  // one bad frame anywhere poisons the body, even wedged between two good ones
  assert.equal(safePollingBody('420["subscribe:prices"]\x1e420["execute"]\x1e42["subscribe:prices"]'),false);
  assert.equal(safePollingBody('42["subscribe:prices"]\x1e42x["subscribe:prices"]'),false);
  assert.equal(safePollingBody('420["subscribe:prices"]\x1e'),false);
});
