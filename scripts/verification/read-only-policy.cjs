'use strict';
// Forward original Engine.IO packets, not synthetic responses. Unknown writes stay blocked.
const subscriptions = new Set(['subscribe:opportunities','subscribe:route_discovery','subscribe:runtime_ack',
  'subscribe:convergence','subscribe:telemetry','subscribe:prices']);
function safeEnginePacket(packet) {
  if (typeof packet !== 'string' || packet.length > 65536) return false;
  if (['1','2','3','5','6','2probe','3probe','40','41'].includes(packet)) return true;
  if (packet.startsWith('40{')) {
    try { const auth=JSON.parse(packet.slice(2)); return auth!==null && typeof auth==='object' && !Array.isArray(auth); }
    catch { return false; }
  }
  if (!packet.startsWith('42[')) return false;
  try {
    const value=JSON.parse(packet.slice(2));
    return Array.isArray(value) && value.length>=1 && subscriptions.has(value[0]);
  } catch { return false; }
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
module.exports={safeEnginePacket,safePollingBody,safeHttp,verified};
