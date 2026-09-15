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
