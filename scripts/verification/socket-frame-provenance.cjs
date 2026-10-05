'use strict';
// SOCKET-FRAME-PROVENANCE-01 — provenance for blocked socket traffic.
//
// WHY THIS MODULE EXISTS
// `public-dapp.cjs` used to collapse two structurally different events into one
// counter (`report.socket.blocked_frames++`, at two sites): a socket dialled to a
// host/path other than the audited WS, and a client frame `safeEnginePacket()`
// refused to forward. The socket check then asserted that counter against 0, so a
// coverage gap ("the product speaks a protocol the gate does not model") and a
// real protocol regression ("the product broke") produced the IDENTICAL opaque
// red. Neither the run nor the report could say which of the two fired.
//
// WHAT THIS MODULE DOES — and what it deliberately does NOT do
// It records, per blocked item, WHICH route fired and WHETHER the client origin
// could be proven; then it reconciles the aggregate against those records. It does
// NOT change what is blocked: `read-only-policy.cjs` still decides what is
// forwarded. This module only decides what a block MEANS.
//
// FAIL-CLOSED CONTRACT (the bar is not lowered)
// The only non-fatal verdict is `declared_client_coverage_gap`, and it is granted
// only when the discriminator can PROVE all of:
//   1. the frame arrived on the client->server channel (structural: the callback
//      is Playwright `routeWebSocket`'s `socket.onMessage`, which only ever
//      receives page-originated messages);
//   2. the frame is a well-formed socket.io event emit: `42[` + a JSON array
//      whose first element is a string in the same shape the gate already accepts
//      as an event name;
//   3. the frame is NOT oversized, so bullet 2 is not a prefix artefact of a
//      longer frame (a >65536-byte frame is rejected by the policy BEFORE its
//      shape is ever examined, so its shape proves nothing -> fatal);
//   4. the event name is provably absent from the read-only allowlist. This
//      follows BY ELIMINATION, not by assumption: `safeEnginePacket()` returns
//      true for exactly this shape when the name IS allowlisted, so a frame of
//      this shape reaching the blocked path cannot be an allowlisted one.
// EVERYTHING ELSE IS FATAL, including a foreign socket TARGET (a page dialling a
// host/path other than the audited one is never a mere coverage gap), a non-string
// frame, an oversized frame, an unparseable frame, and — critically — an aggregate
// counter that does not reconcile with the recorded provenance.
//
// The fact that an event emit was blocked is itself recorded and surfaced: a
// blocked emit means the read-only policy refused to forward it, which is the
// security control working, and it means the gate's model of the product's
// subscriptions is incomplete. It is reported, never hidden.

const assert = require('node:assert/strict');

/** Structural proof that a frame came from the audited page, not the server. */
const CLIENT_CHANNEL_PROOF = 'client->server channel (Playwright routeWebSocket socket.onMessage)';

/** Mirrors the policy's own ceiling so bullet 3 above stays true if it changes. */
const MAX_ENGINE_FRAME_BYTES = 65536;

/** Same event-name shape the gate already accepts when counting events. */
const EVENT_NAME = /^[\w:.-]{1,80}$/;

/** Same redaction discipline the report already applies to free text. */
function redact(value) {
  return String(value).replace(/([?&](?:token|key|secret|auth)[^=]*=)[^&\s]+/gi, '$1[REDACTED]');
}

function base(route, proof) {
  return { route, origin: 'client', proof };
}

/**
 * Classify one frame that `safeEnginePacket()` refused to forward.
 * Returns a record with `verdict` = 'declared_client_coverage_gap' | 'fatal'.
 */
function classifyBlockedClientFrame(data) {
  const proof = CLIENT_CHANNEL_PROOF;
  if (typeof data !== 'string') {
    return { ...base('engine_packet_rejected', proof), kind: 'frame', verdict: 'fatal',
      reason: 'non_string_frame', detail: { value_type: typeof data, bytes: null } };
  }
  if (data.length > MAX_ENGINE_FRAME_BYTES) {
    // The policy rejects on size before inspecting shape, so the shape proves
    // nothing here. Fail closed rather than infer a benign event from a prefix.
    return { ...base('engine_packet_rejected', proof), kind: 'frame', verdict: 'fatal',
      reason: 'oversized_frame', detail: { bytes: data.length, limit: MAX_ENGINE_FRAME_BYTES } };
  }
  let parsed = null;
  if (data.startsWith('42[')) {
    try { parsed = JSON.parse(data.slice(2)); } catch { parsed = null; }
  }
  if (!Array.isArray(parsed) || parsed.length < 1 || typeof parsed[0] !== 'string' || !EVENT_NAME.test(parsed[0])) {
    return { ...base('engine_packet_rejected', proof), kind: 'frame', verdict: 'fatal',
      reason: 'unrecognized_frame_shape',
      detail: { prefix: redact(data.slice(0, 64)), bytes: data.length } };
  }
  return { ...base('engine_packet_rejected', proof), kind: 'frame',
    verdict: 'declared_client_coverage_gap', reason: 'unknown_socket_event',
    detail: { event: parsed[0], bytes: data.length } };
}

/**
 * Classify one socket dialled to a host/path other than the audited WS.
 * Always fatal: a foreign target is a security-relevant event, never a coverage gap.
 */
function classifyBlockedSocketTarget(targetUrl, expectedOrigin) {
  const expected = new URL(expectedOrigin);
  return { kind: 'connection', route: 'ws_target_mismatch', origin: 'client',
    proof: 'page-initiated dial (Playwright routeWebSocket intercepts page dials)',
    verdict: 'fatal', reason: 'foreign_socket_target',
    detail: { host: targetUrl.host, path: targetUrl.pathname,
      expected_host: expected.host, expected_path: '/socket.io/' } };
}

/** Split recorded provenance into the fatal set and the declared coverage gaps. */
function summarizeBlockedSocket(socket) {
  const recorded = Array.isArray(socket?.blocked) ? socket.blocked : [];
  const fatal = recorded.filter(item => item?.verdict !== 'declared_client_coverage_gap');
  const declared = recorded.filter(item => item?.verdict === 'declared_client_coverage_gap');
  const reasons = {};
  for (const item of recorded) reasons[item?.reason || 'unspecified'] = (reasons[item?.reason || 'unspecified'] || 0) + 1;
  return { recorded, fatal, declared, reasons };
}

/** Bound on recorded provenance, so a page that floods the gate cannot grow the report without limit. */
const MAX_BLOCKED_RECORDS = 500;

/**
 * Record one blocked item and bump the aggregate. The aggregate ALWAYS counts
 * reality, even after the record cap is hit — so a truncated run fails the
 * reconciliation below instead of quietly looking explained.
 */
function recordBlocked(socket, record) {
  if (!Array.isArray(socket.blocked)) socket.blocked = [];
  if (socket.blocked.length < MAX_BLOCKED_RECORDS) socket.blocked.push(record);
  else if (socket.blocked.length === MAX_BLOCKED_RECORDS) socket.blocked.push({
    kind: 'truncation', route: 'record_cap_exceeded', origin: 'unproven', proof: null,
    verdict: 'fatal', reason: 'blocked_record_cap_exceeded', detail: { cap: MAX_BLOCKED_RECORDS },
  });
  socket.blocked_frames = (socket.blocked_frames || 0) + 1;
}

/**
 * The socket check's assertion. Fails closed on BOTH:
 *   1. any blocked item whose client origin / nature could not be proven, and
 *   2. an aggregate counter that the recorded provenance does not account for —
 *      which is what an unchecked `blocked_frames++` (an unproven origin) looks
 *      like from the report's point of view.
 * A non-zero counter with no provenance records lands in (2) and stays RED.
 */
function assertBlockedSocketExplained(socket) {
  const { recorded, fatal, declared, reasons } = summarizeBlockedSocket(socket);
  const total = socket?.blocked_frames;
  assert.equal(fatal.length, 0,
    'Socket traffic was blocked and its client origin/nature is NOT proven; the check stays red. ' +
    JSON.stringify(fatal.slice(0, 5)));
  assert.equal(total, recorded.length,
    'socket.blocked_frames (' + total + ') is not accounted for by provenance records (' +
    recorded.length + '); the origin of the blocked frame is NOT COMPUTED, so the check stays red. ' +
    'reasons=' + JSON.stringify(reasons));
  return { declared_coverage_gaps: declared.length, fatal: 0, reasons };
}

module.exports = { classifyBlockedClientFrame, classifyBlockedSocketTarget, summarizeBlockedSocket,
  assertBlockedSocketExplained, recordBlocked, CLIENT_CHANNEL_PROOF, MAX_ENGINE_FRAME_BYTES,
  MAX_BLOCKED_RECORDS, redact };
