'use strict';
// Tests for the socket-frame provenance discriminator (SOCKET-FRAME-PROVENANCE-01).
//
// These assert the FAIL-CLOSED contract of the discriminator, including the exact
// shape that made the original red opaque: an aggregate counter with no record of
// what produced it must keep the socket check RED.
//
// Run: node --test scripts/verification/socket-frame-provenance.test.cjs
const test = require('node:test');
const assert = require('node:assert/strict');
const { safeEnginePacket } = require('./read-only-policy.cjs');
const {
  classifyBlockedClientFrame, classifyBlockedSocketTarget, assertBlockedSocketExplained,
  summarizeBlockedSocket, recordBlocked, MAX_ENGINE_FRAME_BYTES, MAX_BLOCKED_RECORDS,
} = require('./socket-frame-provenance.cjs');

const declared = frame => classifyBlockedClientFrame(frame);

test('a well-formed unknown event emit is DECLARED, never silently ignored', () => {
  const record = declared('42["some:unmodelled:event",{"a":1}]');
  assert.equal(record.verdict, 'declared_client_coverage_gap');
  assert.equal(record.reason, 'unknown_socket_event');
  assert.equal(record.origin, 'client');
  assert.equal(record.detail.event, 'some:unmodelled:event');
  // The fact that it was BLOCKED is recorded, not hidden.
  assert.equal(record.route, 'engine_packet_rejected');
  assert.ok(record.proof && record.proof.length > 0, 'a verdict must carry its proof');
});

test('a declared gap requires provenance to be PROVEN on the client->server channel', () => {
  const record = declared('42["x"]');
  assert.match(record.proof, /client->server channel/);
  assert.equal(record.kind, 'frame');
});

test('ELIMINATION: an allowlisted event name can never reach the blocked path', () => {
  // This is the argument that licenses the declared verdict: safeEnginePacket()
  // accepts exactly this shape when the name IS allowlisted.
  for (const name of ['subscribe:opportunities', 'subscribe:route_discovery', 'subscribe:runtime_ack',
    'subscribe:convergence', 'subscribe:telemetry', 'subscribe:prices']) {
    assert.equal(safeEnginePacket('42["' + name + '"]'), true, name + ' must be forwarded, not blocked');
  }
});

test('malformed, empty, wrong-typed and mis-shaped frames stay FATAL', () => {
  for (const frame of ['42[not json', '42[]', '42[123]', '42[null]', '42["has space!"]', '4', '41', '43[1]', '',
    '42["ok"]trailing']) {
    const record = declared(frame);
    assert.equal(record.verdict, 'fatal', JSON.stringify(frame) + ' must stay fatal, got ' + record.verdict);
    assert.equal(record.reason, 'unrecognized_frame_shape');
  }
});

test('a non-string frame stays FATAL', () => {
  assert.equal(declared(Buffer.from('42["x"]')).verdict, 'fatal');
  assert.equal(declared(Buffer.from('42["x"]')).reason, 'non_string_frame');
  assert.equal(declared(undefined).verdict, 'fatal');
});

test('an OVERSIZED frame stays FATAL even when its prefix looks like a valid event', () => {
  // The policy rejects on size BEFORE inspecting shape, so the shape proves
  // nothing. Inferring "benign event" from a prefix would be the bug.
  const huge = '42["some:event",{"pad":"' + 'a'.repeat(MAX_ENGINE_FRAME_BYTES) + '"}]';
  assert.ok(huge.length > MAX_ENGINE_FRAME_BYTES);
  const record = declared(huge);
  assert.equal(record.verdict, 'fatal');
  assert.equal(record.reason, 'oversized_frame');
});

test('a foreign socket TARGET is always FATAL, never a coverage gap', () => {
  const record = classifyBlockedSocketTarget(new URL('wss://evil.example/socket.io/'), 'https://dapp.example');
  assert.equal(record.verdict, 'fatal');
  assert.equal(record.reason, 'foreign_socket_target');
  assert.equal(record.kind, 'connection');
  assert.equal(record.detail.expected_host, 'dapp.example');
});

test('the happy path still passes when nothing was blocked', () => {
  const result = assertBlockedSocketExplained({ blocked_frames: 0, blocked: [] });
  assert.equal(result.declared_coverage_gaps, 0);
  assert.equal(result.fatal, 0);
});

test('a PROVEN client coverage gap is the only non-fatal outcome, and it is reported', () => {
  const socket = { blocked_frames: 0, blocked: [] };
  recordBlocked(socket, declared('42["unmodelled:a"]'));
  recordBlocked(socket, declared('42["unmodelled:b"]'));
  const result = assertBlockedSocketExplained(socket);
  assert.equal(result.declared_coverage_gaps, 2);
  assert.equal(socket.blocked_frames, 2);
});

test('one unproven frame among proven ones keeps the whole check RED', () => {
  const socket = { blocked_frames: 0, blocked: [] };
  recordBlocked(socket, declared('42["unmodelled:a"]'));
  recordBlocked(socket, declared('42[not json'));
  assert.throws(() => assertBlockedSocketExplained(socket), /NOT proven/);
});

test('THE ORIGINAL DEFECT: a non-zero counter with NO provenance stays RED', () => {
  // This is the historical shape (runs 37225460876 / 37218901862): a bare
  // blocked_frames with no record of which route produced it. It must NOT pass.
  assert.throws(() => assertBlockedSocketExplained({ blocked_frames: 2, blocked: [] }),
    /NOT COMPUTED/);
  assert.throws(() => assertBlockedSocketExplained({ blocked_frames: 2 }),
    /NOT COMPUTED/);
});

test('a counter that does not reconcile with the records stays RED', () => {
  const socket = { blocked_frames: 3, blocked: [] };
  recordBlocked(socket, declared('42["unmodelled:a"]'));
  socket.blocked_frames = 3; // an extra increment nobody recorded
  assert.throws(() => assertBlockedSocketExplained(socket), /not accounted for/);
});

test('recordBlocked keeps the aggregate honest even past the record cap', () => {
  const socket = { blocked_frames: 0, blocked: [] };
  for (let i = 0; i < MAX_BLOCKED_RECORDS + 3; i++) recordBlocked(socket, declared('42["e' + i + '"]'));
  assert.equal(socket.blocked_frames, MAX_BLOCKED_RECORDS + 3, 'the aggregate must count reality');
  assert.ok(socket.blocked.length <= MAX_BLOCKED_RECORDS + 1, 'records are bounded');
  // Truncation is fatal, and the unreconciled count is fatal too.
  assert.throws(() => assertBlockedSocketExplained(socket), /not accounted for|NOT proven/);
  const summary = summarizeBlockedSocket(socket);
  assert.equal(summary.reasons.blocked_record_cap_exceeded, 1);
});

test('a truncated record set is fatal on its own, even if counts happened to match', () => {
  const socket = { blocked_frames: 0, blocked: [] };
  for (let i = 0; i < MAX_BLOCKED_RECORDS + 1; i++) recordBlocked(socket, declared('42["e' + i + '"]'));
  assert.throws(() => assertBlockedSocketExplained(socket), /NOT proven/);
});

test('WIRING: every provenance.<name> the entry file calls is actually exported', () => {
  // The live gate cannot run in this environment (no frontend/node_modules and no
  // DAPP_ORIGIN), so the entry-file -> module wiring is proven by resolution here
  // rather than asserted: a renamed or missing export would fail this test.
  const fs = require('node:fs');
  const path = require('node:path');
  const src = fs.readFileSync(path.join(__dirname, 'public-dapp.cjs'), 'utf8');
  const used = [...src.matchAll(/provenance\.([A-Za-z_][A-Za-z0-9_]*)\s*\(/g)].map(m => m[1]);
  assert.deepEqual([...new Set(used)].sort(),
    ['assertBlockedSocketExplained', 'classifyBlockedClientFrame', 'classifyBlockedSocketTarget', 'recordBlocked'],
    'the entry file must drive both blocked routes AND the fail-closed assertion');
  const api = require('./socket-frame-provenance.cjs');
  for (const name of new Set(used)) assert.equal(typeof api[name], 'function', name + ' is not exported');
  assert.ok(src.includes("require('./socket-frame-provenance.cjs')"), 'module must be required');
  assert.ok(src.includes('blocked:[]'), 'the report must carry the provenance array');
});

test('WIRING: the entry file keeps the aggregate counter and reconciles it', () => {
  const fs = require('node:fs');
  const path = require('node:path');
  const src = fs.readFileSync(path.join(__dirname, 'public-dapp.cjs'), 'utf8');
  // The old opaque assertion must be gone, and the aggregate must still exist.
  assert.ok(!src.includes("assert.equal(report.socket.blocked_frames,0"),
    'the opaque blocked_frames===0 assertion must be replaced, not kept alongside');
  assert.ok(src.includes('blocked_frames:0'), 'the aggregate counter must still be reported');
});
