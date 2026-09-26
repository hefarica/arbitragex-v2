/**
 * CHANNELS-REGISTRY (2026-09-26) — canonical declared-wiring registry for the
 * Redis Streams bus (R10 companion to the runtime /api/v1/channels surface).
 *
 * Two truths, never conflated:
 *  - THIS registry = DECLARED wiring truth: which stream is produced/consumed
 *    by which module, and its wiring status as verified against the code and
 *    the 2026-09-26 24-container runtime survey.
 *  - GET /api/v1/channels (routes/channels.ts) = RUNTIME truth: XLEN,
 *    heartbeat, pending, zombies — read live from Redis.
 *
 * Status vocabulary (R10 — declared-but-never-produced is NOT "zero"):
 *  - "active"               — produced and consumed in the last survey window.
 *  - "wired-starved"        — producer+consumer code exists and is wired, but
 *                             upstream starvation keeps XLEN at 0.
 *  - "no-producer-observed" — the stream is declared/created (XGROUP MKSTREAM
 *                             or emitter behind a gate) but no producer has
 *                             ever written an entry in production.
 *
 * The test companion (channels-registry.test.ts) asserts that every path
 * declared here EXISTS in the repo — wiring claims are checked, not prose.
 * Status transitions are deliberate edits: when a PR wires a producer, it
 * MUST flip the status here in the same PR (R10 discipline).
 */

export type ChannelStatus = "active" | "wired-starved" | "no-producer-observed";

export interface ChannelEntry {
  /** Redis stream key. */
  name: string;
  /** Producing module(s) — repo-relative path(s) that must exist. */
  producer: string[];
  /** Consuming modules — repo-relative path(s) that must exist. */
  consumers: string[];
  status: ChannelStatus;
  /** What starves (or dies) when this channel does not circulate. */
  impact: string;
}

export const CANONICAL_CHANNEL_REGISTRY: ChannelEntry[] = [
  {
    name: "arbx:opps:detected",
    producer: ["backend/searcher-rs"],
    // NOTA R10 (mapeo incompleto): el contenedor "enricher" también consume
    // este stream en runtime (encuesta 24 contenedores, 2026-09-26) pero su
    // módulo repo no está identificado — se corrige aquí cuando se mapee.
    consumers: ["backend/selector-api/src/consumer.ts"],
    status: "active",
    impact:
      "sin circulación no existen oportunidades: todo el funnel (validated→simulated→executed) muere aguas abajo",
  },
  {
    name: "arbx:opps:validated",
    producer: ["backend/selector-api/src/consumer.ts"],
    consumers: ["backend/sim-ctl"],
    status: "wired-starved",
    impact:
      "sim-ctl sin entrada ⇒ simulations=0 ⇒ relays/recon/paper sin trabajo (observado: gate selector rechaza 100% por v3_quote_unavailable, estancado 8.5 días)",
  },
  {
    name: "arbx:opps:simulated",
    producer: ["backend/sim-ctl"],
    consumers: ["backend/relays-client"],
    status: "wired-starved",
    impact: "relays-client sin candidatas ⇒ XLEN=0 desde el génesis (R10: no es 'cero evaluado')",
  },
  {
    name: "arbx:opps:executed",
    producer: ["backend/relays-client"],
    consumers: ["backend/recon"],
    status: "wired-starved",
    impact: "recon sin ejecuciones que reconciliar ⇒ XLEN=0 (depende de simulated)",
  },
  {
    name: "arbx:scoring:scored",
    producer: ["backend/api-server"],
    consumers: ["backend/api-server/src/routes/scored-opportunities-archiver.ts"],
    status: "active",
    impact: "histórico de scoring para KPIs; sin él, scored_opportunities deja de crecer",
  },
  {
    name: "arbx:route_discovery:outcomes",
    producer: ["backend/searcher-rs"],
    consumers: ["backend/api-server/src/routes/route-discovery-outcome-sink.ts"],
    status: "active",
    impact: "evidencia de descubrimiento de rutas; sink con retraso histórico (pendiente de reclamo PEL)",
  },
  {
    name: "arbx:hot:detected",
    producer: ["backend/api-server/src/websocket.ts"],
    consumers: ["backend/api-server/src/websocket.ts"],
    status: "no-producer-observed",
    impact:
      "emisor hot gated a simulación REVM que no corre — stream declarado jamás producido (R10: NO COMPUTADO, no cero)",
  },
  {
    name: "arbx:hot:simulated",
    producer: ["backend/api-server/src/websocket.ts"],
    consumers: ["backend/api-server/src/websocket.ts"],
    status: "no-producer-observed",
    impact: "ídem hot:detected — wire declarado sin productor activo",
  },
];
