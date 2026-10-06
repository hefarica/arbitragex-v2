'use strict';
// Forward original Engine.IO packets, not synthetic responses. Unknown writes stay blocked.
const subscriptions = new Set(['subscribe:opportunities','subscribe:route_discovery','subscribe:runtime_ack',
  'subscribe:convergence','subscribe:telemetry','subscribe:prices']);
const MAX_ENGINE_PACKET_BYTES = 65536;

/**
 * Interpret the Engine.IO/Socket.IO frame STRUCTURE — including the parts the old
 * `packet.startsWith('42[')` test stepped over. This function authorises NOTHING:
 * it only answers "is this a well-formed Socket.IO EVENT frame, and which event,
 * acknowledgement id and namespace does it carry?".
 *
 * Grammar accepted (Socket.IO v4 over Engine.IO v4, EVENT frames only):
 *   '4'                        Engine.IO MESSAGE
 *   '2'                        Socket.IO EVENT
 *   [0-9]+      (optional)     acknowledgement id the client is requesting:
 *                              `420[...]` is an EVENT asking for an ACK, NOT an ACK
 *   '/' name ',' (optional)    namespace
 *   <json array>               payload; [0] is the event name
 *
 * Anything else returns null = "not a frame this verifier can interpret", which
 * every caller MUST treat as blocked: a different Engine.IO or Socket.IO type, a
 * non-digit where the acknowledgement id belongs (`42x[...]`), a namespace with no
 * comma, or a payload that is not a JSON array.
 *
 * The acknowledgement id is a CLIENT-chosen integer. Interpreting it proves the
 * frame is syntactically an EVENT-with-ACK-request; it does NOT authorise the
 * emitter and does NOT show the server ever issued an acknowledgement. Those
 * remain separate checks.
 *
 * SIZE IS CHECKED FIRST, deliberately: a frame over the limit is rejected before
 * its shape is examined, so no prefix of it can be read as evidence of anything.
 */
function parseSocketIoEvent(packet) {
  if (typeof packet !== 'string' || packet.length > MAX_ENGINE_PACKET_BYTES) return null;
  if (packet[0] !== '4') return null;                       // Engine.IO MESSAGE
  if (packet[1] !== '2') return null;                       // Socket.IO EVENT
  let i = 2;
  const digitsStart = i;
  while (i < packet.length && packet[i] >= '0' && packet[i] <= '9') i++;
  const ackId = i > digitsStart ? packet.slice(digitsStart, i) : null;
  let namespace = null;
  if (packet[i] === '/') {
    const comma = packet.indexOf(',', i);
    if (comma === -1) return null;                          // namespace without its comma
    namespace = packet.slice(i, comma);
    i = comma + 1;
  }
  if (packet[i] !== '[') return null;                       // payload must be the JSON array
  let payload;
  try { payload = JSON.parse(packet.slice(i)); } catch { return null; }
  if (!Array.isArray(payload) || payload.length < 1) return null;
  return { ackId, namespace, payload, event: payload[0] };
}

function safeEnginePacket(packet) {
  if (typeof packet !== 'string' || packet.length > MAX_ENGINE_PACKET_BYTES) return false;
  if (['1','2','3','5','6','2probe','3probe','40','41'].includes(packet)) return true;
  if (packet.startsWith('40{')) {
    try { const auth=JSON.parse(packet.slice(2)); return auth!==null && typeof auth==='object' && !Array.isArray(auth); }
    catch { return false; }
  }
  // 1) Interpret the structure FIRST.
  const frame = parseSocketIoEvent(packet);
  if (frame === null) return false;
  // 2) Only THEN apply authorisation, with the namespace and event controls
  //    EXACTLY as strict as before: this verifier speaks the default namespace
  //    only, and only the allowlisted subscription events.
  if (frame.namespace !== null) return false;
  return subscriptions.has(frame.event);
}
function safePollingBody(body) {
  return typeof body==='string' && body.length>0 && body.split('\x1e').every(safeEnginePacket);
}
function safeHttp(method, url, body, origin) {
  const target=new URL(url);
  if (['GET','HEAD','OPTIONS'].includes(method)) return true;
  return method==='POST' && target.origin===origin && target.pathname==='/socket.io/' &&
    target.searchParams.get('EIO')==='4' && target.searchParams.get('transport')==='polling' && safePollingBody(body);
}
function verified(checks, required) {
  return Array.isArray(required) && required.length>0 && new Set(required).size===required.length &&
    required.every(id=>Object.hasOwn(checks,id) && checks[id]?.status==='passed');
}
module.exports={safeEnginePacket,safePollingBody,safeHttp,verified,parseSocketIoEvent,MAX_ENGINE_PACKET_BYTES};
