import { Server } from 'socket.io';
import { Server as HttpServer } from 'http';
import { Redis } from 'ioredis';
import {
    safeTokenEqual,
    runtimeAckBroadcastTotal,
    runtimeAckBroadcastLatencyMs,
} from '@arbx/shared';

// ---------------------------------------------------------------------------
// Tipos públicos
// ---------------------------------------------------------------------------

/**
 * OMEGA-7 PR-1: runtime_ack room identifier.
 *
 * Clients subscribe via `socket.emit('subscribe:runtime_ack')`. The api-server
 * emits a single `runtime_ack` event into this room *after* a successful
 * PostgreSQL INSERT in POST /api/system/runtime-ack (invariant I-2).
 */
export const RUNTIME_ACK_ROOM = 'runtime_ack' as const;

/**
 * FASE OMEGA — Cartridge telemetry room. Clients subscribe via
 * `socket.emit('subscribe:telemetry')` and receive a `telemetry` event for every
 * `log_quantum` message the Rust cartridges publish to `arbx:cartridge:telemetry`.
 */
export const CARTRIDGE_TELEMETRY_ROOM = 'telemetry' as const;

/**
 * FASE OMEGA — shape of a cartridge telemetry message. The Rust `log_quantum`
 * host binding publishes `{ cartridge_id, level, message, ts }`; the payload is
 * intentionally open (`[k: string]: unknown`) because cartridge authors may add
 * arbitrary structured fields — telemetry is observational, not a strict contract.
 */
export interface CartridgeTelemetry {
    cartridge_id?: string;
    level?: string;
    message?: string;
    [k: string]: unknown;
}

/**
 * QUANTUM FULLSTACK SYMMETRY — Route Discovery telemetry room. Clients subscribe
 * via `socket.emit('subscribe:route_discovery')` and receive a
 * `route_discovery_telemetry` event for every message the Rust radar publishes
 * to `arbx:route_discovery:telemetry` (tick / route_candidate /
 * strategy_applicability / rejected / route_intent.emitted).
 */
export const ROUTE_DISCOVERY_TELEMETRY_ROOM = 'route_discovery' as const;

/**
 * Shape of a route-discovery telemetry message. Open (`[k: string]: unknown`)
 * because the 5 event types share only the `event` discriminator — telemetry is
 * observational, not a strict contract (mirrors {@link CartridgeTelemetry}).
 */
export interface RouteDiscoveryTelemetry {
    event?: string;
    [k: string]: unknown;
}

/** Layers accepted by the runtime_ack handler. Isomorphic 1:1 to the Zod enum
 *  in routes/system-manifest.ts:28-31 — DO NOT drift these without updating
 *  both the server schema and the frontend schema together (invariant I-3). */
export type RuntimeAckLayer =
    | 'api'
    | 'persistence'
    | 'redis_pubsub'
    | 'searcher_rs'
    | 'arc_swap'
    | 'frontend_refresh'
    | 'readiness'
    | 'audit';

/** Status terminal-set accepted by the runtime_ack handler. Isomorphic 1:1 to
 *  routes/system-manifest.ts:32. */
export type RuntimeAckStatus =
    | 'pending'
    | 'received'
    | 'applied'
    | 'rejected'
    | 'timeout'
    | 'failed';

/**
 * OMEGA-7 PR-1: WSS broadcast payload for a runtime ack.
 *
 * Shape is **isomorphic 1:1** to RuntimeAckSchema in
 * routes/system-manifest.ts:20-35. Eleven fields, exact same names, exact
 * same enums. The route handler emits *the parsed Zod object* unchanged, so
 * any drift between this type and the Zod schema is a bug — the schema is
 * the single source of truth (invariant I-3 conservación schema_version).
 *
 * @since OMEGA-7 — Tribunal remediation Option A
 */
export interface RuntimeAckBroadcast {
    event_id: string;
    resource: string;
    chain_id: number | null;
    idempotency_key: string;
    config_hash_before: string | null;
    config_hash_after: string;
    worker_id: string;
    layer: RuntimeAckLayer;
    status: RuntimeAckStatus;
    latency_ms?: number | null | undefined;
    error?: string | null | undefined;
}

/**
 * Señal de convergencia emitida por el motor SED (sed-core, Rust) a través
 * de Redis Pub/Sub.  El frontend se suscribe vía WebSocket al room
 * `convergence` para recibir actualizaciones en tiempo real del estado del
 * pipeline de arbitraje.
 *
 * @since OMEGA-v2 — Arteria WSS
 */
export interface ConvergenceSignal {
    /** Snapshot de entropía del SED engine */
    entropy_snapshot: {
        mempool_tx_per_sec: number;
        mempool_avg_gas_price_gwei: number;
        mempool_entropy_score: number;
        reserve_divergence_max: number;
    };
    /** Métricas del pipeline SED */
    pipeline_latency_ms: number;
    opportunities_detected: number;
    simulations_run: number;
    simulations_success: number;
    /** Timestamp ISO8601 */
    timestamp: string;
    /** Versión del schema para compatibilidad futura */
    schema_version: number;
}

// ---------------------------------------------------------------------------
// Utilidades internas
// ---------------------------------------------------------------------------

// API-2: WebSocket CORS allowlist. ALLOWED_ORIGINS=comma,separated,list.
// Empty list = same-origin only (no Origin header allowed).
// "*" is INTENTIONALLY NOT supported — fail-honest.
function parseAllowedOrigins(): string[] {
    const raw = process.env["ALLOWED_ORIGINS"] ?? "";
    return raw.split(',').map((s) => s.trim()).filter(Boolean);
}

// A1 fix (audit 2026-05-10): WebSocket handshake authentication.
// Without this gate, ANY client could subscribe to `subscribe:opportunities`
// and `subscribe:metrics`, leaking real-time MEV alpha to competitors.
// The token can be supplied via three sources, in priority order:
//   1. auth payload — io.connect(URL, { auth: { token: "..." } })  (preferred)
//   2. query param  — io.connect(URL + "?token=...")               (browser fallback)
//   3. header       — X-ArbX-Admin-Token (tooling / curl)
// Constant-time compare via safeTokenEqual from @arbx/shared.
function extractHandshakeToken(handshake: {
    auth?: unknown;
    query?: Record<string, string | string[] | undefined>;
    headers?: Record<string, string | string[] | undefined>;
}): string {
    const authObj = (handshake.auth ?? {}) as { token?: unknown };
    if (typeof authObj.token === 'string' && authObj.token.length > 0) {
        return authObj.token;
    }
    const queryToken = handshake.query?.['token'];
    if (typeof queryToken === 'string' && queryToken.length > 0) {
        return queryToken;
    }
    if (Array.isArray(queryToken) && queryToken.length > 0 && typeof queryToken[0] === 'string') {
        return queryToken[0];
    }
    const headerToken = handshake.headers?.['x-arbx-admin-token'];
    if (typeof headerToken === 'string' && headerToken.length > 0) {
        return headerToken;
    }
    if (Array.isArray(headerToken) && headerToken.length > 0 && typeof headerToken[0] === 'string') {
        return headerToken[0];
    }
    return '';
}

/**
 * Validación mínima del shape de un ConvergenceSignal recibido por Redis.
 * No garantiza tipos exactos de cada campo numérico, pero asegura que el
 * objeto tiene la estructura esperada antes de re-emitirlo por WebSocket.
 */
function isValidConvergenceSignal(payload: unknown): payload is ConvergenceSignal {
    if (typeof payload !== 'object' || payload === null) return false;
    const p = payload as Record<string, unknown>;
    if (typeof p.timestamp !== 'string') return false;
    if (typeof p.schema_version !== 'number') return false;
    if (typeof p.pipeline_latency_ms !== 'number') return false;
    if (typeof p.opportunities_detected !== 'number') return false;
    if (typeof p.simulations_run !== 'number') return false;
    if (typeof p.simulations_success !== 'number') return false;
    const es = p.entropy_snapshot;
    if (typeof es !== 'object' || es === null) return false;
    const e = es as Record<string, unknown>;
    if (typeof e.mempool_tx_per_sec !== 'number') return false;
    if (typeof e.mempool_avg_gas_price_gwei !== 'number') return false;
    if (typeof e.mempool_entropy_score !== 'number') return false;
    if (typeof e.reserve_divergence_max !== 'number') return false;
    return true;
}

// ---------------------------------------------------------------------------
// Gateway WebSocket
// ---------------------------------------------------------------------------

export function setupWebSocketGateway(server: HttpServer) {
    const allowed = parseAllowedOrigins();
    const io = new Server(server, {
        cors: {
            origin: (origin, cb) => {
                // Same-origin (no Origin header) is always allowed.
                if (!origin) return cb(null, true);
                if (allowed.includes(origin)) return cb(null, true);
                // Reject — do NOT pass an Error object (Socket.IO logs it noisily and the
                // upstream client just sees a generic 403 either way).
                cb(null, false);
            },
            credentials: true,
        },
    });

    // A1 fix (audit 2026-05-10): admin-token gate on the upgrade handshake.
    // Boot validator `assertSecureBootTokens()` already guarantees
    // ARBX_ADMIN_TOKEN is non-empty, non-placeholder, and >=32 bytes — so we
    // can read it directly without re-validating the value here.
    //
    // OMEGA-8/M3 P0-2: defense-in-depth gating for the runtime_ack room.
    // The handshake already requires admin token, so any connected socket is
    // implicitly authorized. We additionally mark a per-socket capability
    // flag at handshake time and re-check it on `subscribe:runtime_ack` to
    // ensure no future refactor (e.g. relaxing the handshake gate to a
    // tiered model) can silently expose the runtime_ack room to anonymous
    // clients. Tested in `websocket.test.ts`.
    const expectedAdminToken = process.env['ARBX_ADMIN_TOKEN'] ?? '';
    io.use((socket, next) => {
        const got = extractHandshakeToken(socket.handshake);
        if (!got || !safeTokenEqual(got, expectedAdminToken)) {
            return next(new Error('unauthorized: invalid or missing admin token'));
        }
        // Mark this socket as having operator/admin privilege. Used by
        // sensitive rooms (currently `runtime_ack`) for defense-in-depth.
        (socket as unknown as { data: Record<string, unknown> }).data = {
            ...((socket as unknown as { data?: Record<string, unknown> }).data ?? {}),
            runtimeAckAllowed: true,
        };
        next();
    });

    io.on('connection', (socket: any) => {
        console.log(`[WebSocket] Nuevo cliente conectado: ${socket.id}`);

        socket.on('subscribe:opportunities', () => {
            console.log(`[WebSocket] Cliente ${socket.id} se suscribió a Oportunidades`);
            socket.join('opportunities');
        });

        socket.on('subscribe:metrics', () => {
            console.log(`[WebSocket] Cliente ${socket.id} se suscribió a Métricas`);
            socket.join('metrics');
        });

        // Arteria WSS — OMEGA-v2: suscripción a señales de convergencia del
        // motor SED (Rust).  Los clientes frontend reciben en tiempo real el
        // estado del pipeline de arbitraje.
        socket.on('subscribe:convergence', () => {
            console.log(`[WebSocket] Cliente ${socket.id} se suscribió a Convergencia`);
            socket.join('convergence');
        });

        // FASE OMEGA — Cartridge telemetry. Clients (Strategy Forge UI) subscribe to
        // receive the live `log_quantum` stream the Rust cartridges publish to
        // `arbx:cartridge:telemetry`. Same handshake auth gate as every other room.
        socket.on('subscribe:telemetry', () => {
            console.log(`[WebSocket] Cliente ${socket.id} se suscribió a Telemetría de Cartuchos`);
            socket.join(CARTRIDGE_TELEMETRY_ROOM);
        });

        // QUANTUM FULLSTACK SYMMETRY — Route Discovery telemetry. Clients (Route
        // Discovery panel) subscribe to receive the live radar stream the Rust
        // searcher publishes to `arbx:route_discovery:telemetry`. Same handshake
        // auth gate as every other room; observe-only.
        socket.on('subscribe:route_discovery', () => {
            console.log(`[WebSocket] Cliente ${socket.id} se suscribió a Telemetría de Route Discovery`);
            socket.join(ROUTE_DISCOVERY_TELEMETRY_ROOM);
        });

        // OMEGA-7 PR-1: runtime_ack room. Receives the ack broadcast emitted by
        // broadcastRuntimeAck after a successful POST /api/system/runtime-ack
        // INSERT. Frontend useActionState 12-states transitions
        // WAITING_RUNTIME_ACK → VERIFIED on receipt of an event with the
        // matching event_id.
        //
        // OMEGA-8/M3 P0-2 (defense-in-depth): verify the per-socket
        // capability flag set during the handshake. If absent (which can only
        // happen if someone bypasses io.use), refuse the join and emit a
        // structured `error` event so the client surfaces the failure rather
        // than silently waiting for a broadcast that will never arrive.
        socket.on('subscribe:runtime_ack', () => {
            const allowed = socket?.data?.runtimeAckAllowed === true;
            if (!allowed) {
                console.warn(
                    `[WebSocket] Cliente ${socket.id} INTENTÓ unirse a runtime_ack sin autorización — rechazado`,
                );
                socket.emit('error', { code: 'unauthorized', room: RUNTIME_ACK_ROOM });
                return;
            }
            console.log(`[WebSocket] Cliente ${socket.id} se suscribió a Runtime ACK`);
            socket.join(RUNTIME_ACK_ROOM);
        });

        socket.on('disconnect', () => {
            console.log(`[WebSocket] Cliente desconectado: ${socket.id}`);
        });
    });

    return io;
}

// ---------------------------------------------------------------------------
// Broadcast helpers — PostgreSQL NOTIFY → WebSocket (existente, sin cambios)
// ---------------------------------------------------------------------------

// Simulador de emisión de oportunidades
export function broadcastOpportunity(io: Server, opp: any) {
    io.to('opportunities').emit('new_opportunity', opp);
}

// ---------------------------------------------------------------------------
// Broadcast helpers — ConvergenceSignal (Redis Pub/Sub → WebSocket)
// ---------------------------------------------------------------------------

/**
 * Emite una señal de convergencia a todos los clientes suscritos al room
 * `convergence`.  Esta función es la interface síncrona para retransmisión;
 * el path principal es el flujo asíncrono vía Redis Pub/Sub en
 * {@link subscribeToConvergenceSignals}.
 *
 * @param io   — instancia de Socket.IO Server (retornada por setupWebSocketGateway)
 * @param signal — objeto ConvergenceSignal validado
 *
 * @since OMEGA-v2 — Arteria WSS
 */
export function broadcastConvergenceSignal(io: Server, signal: ConvergenceSignal) {
    io.to('convergence').emit('convergence_signal', signal);
}

// ---------------------------------------------------------------------------
// Broadcast helpers — RuntimeAck (POST /api/system/runtime-ack → WebSocket)
// ---------------------------------------------------------------------------

const RUNTIME_ACK_LAYERS: ReadonlySet<RuntimeAckLayer> = new Set<RuntimeAckLayer>([
    'api', 'persistence', 'redis_pubsub', 'searcher_rs', 'arc_swap',
    'frontend_refresh', 'readiness', 'audit',
]);
const RUNTIME_ACK_STATUSES: ReadonlySet<RuntimeAckStatus> = new Set<RuntimeAckStatus>([
    'pending', 'received', 'applied', 'rejected', 'timeout', 'failed',
]);

/**
 * Defense-in-depth type guard for a RuntimeAckBroadcast payload. The route
 * handler already validated the same shape via Zod, but this guard ensures
 * `broadcastRuntimeAck` cannot silently emit a corrupt payload if any caller
 * ever bypasses the route (tests, internal scripts, future refactors). It
 * does NOT replace the route's Zod schema — both checks coexist; the Zod
 * schema in system-manifest.ts is the source of truth (invariant I-3).
 */
export function isValidRuntimeAck(payload: unknown): payload is RuntimeAckBroadcast {
    if (typeof payload !== 'object' || payload === null) return false;
    const p = payload as Record<string, unknown>;
    if (typeof p.event_id !== 'string' || p.event_id.length === 0) return false;
    if (typeof p.resource !== 'string' || p.resource.length === 0) return false;
    if (p.chain_id !== null && typeof p.chain_id !== 'number') return false;
    if (typeof p.idempotency_key !== 'string' || p.idempotency_key.length === 0) return false;
    if (p.config_hash_before !== null && typeof p.config_hash_before !== 'string') return false;
    if (typeof p.config_hash_after !== 'string' || p.config_hash_after.length === 0) return false;
    if (typeof p.worker_id !== 'string' || p.worker_id.length === 0) return false;
    if (typeof p.layer !== 'string' || !RUNTIME_ACK_LAYERS.has(p.layer as RuntimeAckLayer)) return false;
    if (typeof p.status !== 'string' || !RUNTIME_ACK_STATUSES.has(p.status as RuntimeAckStatus)) return false;
    if (p.latency_ms !== undefined && p.latency_ms !== null && typeof p.latency_ms !== 'number') return false;
    if (p.error !== undefined && p.error !== null && typeof p.error !== 'string') return false;
    return true;
}

/**
 * OMEGA-7 PR-1: emit a runtime_ack event into the `runtime_ack` room.
 *
 * **Invariant I-2 (idempotencia POST-INSERT):** this MUST be called *only
 * after* the PostgreSQL INSERT for the same payload has succeeded inside
 * POST /api/system/runtime-ack. The route handler enforces that ordering.
 *
 * **Invariant I-1 (bijection event_id):** Socket.IO `io.to(room).emit(...)` is
 * a single fan-out — each connected subscriber receives exactly one delivery
 * per call. The frontend hook filters by `event_id` so cross-talk between
 * concurrent ack flows cannot trigger spurious onAck dispatches.
 *
 * Fail-honest: if the type guard rejects the payload (which should be
 * impossible because the route already Zod-parsed it), we increment the
 * `rejected` status counter and return without emitting — never crash the
 * surrounding INSERT-success path. The caller catches synchronous throws
 * defensively in any case.
 *
 * @since OMEGA-7 — Tribunal remediation Option A
 */
export function broadcastRuntimeAck(io: Server, payload: RuntimeAckBroadcast): void {
    if (!isValidRuntimeAck(payload)) {
        runtimeAckBroadcastTotal.labels('rejected').inc();
        console.warn(
            '[RuntimeAck] payload rejected by isValidRuntimeAck guard, not emitting',
        );
        return;
    }
    const start = process.hrtime.bigint();
    io.to(RUNTIME_ACK_ROOM).emit('runtime_ack', payload);
    const elapsedNs = process.hrtime.bigint() - start;
    const elapsedMs = Number(elapsedNs) / 1_000_000;
    runtimeAckBroadcastLatencyMs.observe(elapsedMs);
    runtimeAckBroadcastTotal.labels(payload.status).inc();
}

// ---------------------------------------------------------------------------
// Redis Pub/Sub bridge — Arteria WSS
// ---------------------------------------------------------------------------

const CONVERGENCE_CHANNEL = 'arbx:signals:convergence';

/**
 * Inicia el puente Redis → WebSocket para señales de convergencia.
 *
 * Crea una **instancia Redis dedicada** (solo SUBSCRIBE — ioredis no permite
 * mezclar comandos regulares con modo subscriber).  Se suscribe al canal
 * `arbx:signals:convergence`; cada mensaje es parseado como JSON, validado
 * con shape-check mínimo, y re-emitido al room `convergence` de Socket.IO.
 *
 * **Fail-honest**: si Redis no está disponible el WebSocket de oportunidades
 * sigue funcionando.  Los errores de parseo se loguean y se descartan (skip),
 * nunca crashean el proceso.
 *
 * **Reconexión automática**: ioredis gestiona reconnexiones por defecto
 * (`retryStrategy` built-in).  Si la conexión se pierde, los mensajes se
 * perderán durante la ventana de desconexión; al reconectar se re-suscribe
 * automáticamente.
 *
 * **Debe invocarse UNA SOLA VEZ** desde `index.ts`, después de
 * `setupWebSocketGateway()`:
 *
 * ```typescript
 * const io = setupWebSocketGateway(httpServer);
 * const convergenceSub = subscribeToConvergenceSignals(io, REDIS_URL);
 * // ... en shutdown:
 * await convergenceSub.quit().catch(() => {});
 * ```
 *
 * @param io       — instancia de Socket.IO Server
 * @param redisUrl — URL de conexión Redis (tipicamente REDIS_URL desde env)
 * @returns        — instancia Redis subscriber (para cleanup en shutdown)
 *
 * @since OMEGA-v2 — Arteria WSS
 */
export function subscribeToConvergenceSignals(io: Server, redisUrl: string): Redis {
    // Instancia DEDICADA para SUBSCRIBE.  No reutilizar la conexión Redis
    // principal (GET/SET/PUBLISH) porque ioredis entra en modo subscriber
    // donde solo comandos de subscription son válidos.
    const subscriber = new Redis(redisUrl, {
        // Fail-honest: no bloquear el boot si Redis no está disponible aún.
        // ioredis encolará los comandos y los enviará tras reconectar.
        lazyConnect: false,
        // Nunca reintentar comandos de pub/sub — son fire-and-forget.
        maxRetriesPerRequest: 1,
        // Reconexión automática con backoff exponencial (default de ioredis).
        // Se sobreescribe ligeramente para loggear cada reintento.
        retryStrategy(times: number) {
            const delay = Math.min(times * 50, 2000);
            console.log(`[ArteriaWSS] Redis subscriber reconnect attempt ${times}, retrying in ${delay}ms`);
            return delay;
        },
        // Si Redis está unreachable, no crashear — reintentar forever.
        reconnectOnError(err) {
            const targetErrors = ['ECONNREFUSED', 'ETIMEDOUT', 'ECONNRESET', 'EHOSTUNREACH'];
            const shouldReconnect = targetErrors.some(code => err.message.includes(code));
            console.log(`[ArteriaWSS] Redis subscriber error: ${err.message} — reconnect=${shouldReconnect}`);
            return shouldReconnect ? 2 : false;
        },
    });

    // ---- Eventos de conexión / reconexión (observability) ----

    subscriber.on('connect', () => {
        console.log('[ArteriaWSS] Redis subscriber connected');
    });

    subscriber.on('ready', () => {
        console.log('[ArteriaWSS] Redis subscriber ready');
    });

    subscriber.on('close', () => {
        console.log('[ArteriaWSS] Redis subscriber connection closed');
    });

    subscriber.on('end', () => {
        console.log('[ArteriaWSS] Redis subscriber connection ended');
    });

    subscriber.on('error', (err: Error) => {
        // Error-level log pero SIN throw — fail-honest.  El WebSocket de
        // oportunidades (PostgreSQL NOTIFY) sigue funcionando independientemente.
        console.error('[ArteriaWSS] Redis subscriber error (non-fatal):', err.message);
    });

    subscriber.on('reconnecting', (delayMs: number) => {
        console.log(`[ArteriaWSS] Redis subscriber reconnecting in ${delayMs}ms`);
    });

    // ---- Suscripción al canal de convergencia ----

    subscriber.subscribe(CONVERGENCE_CHANNEL).then(() => {
        console.log(`[ArteriaWSS] Subscribed to Redis channel: ${CONVERGENCE_CHANNEL}`);
    }).catch((err: Error) => {
        console.error(`[ArteriaWSS] Failed to subscribe to ${CONVERGENCE_CHANNEL}:`, err.message);
    });

    // ---- Handler de mensajes ----

    subscriber.on('message', (channel: string, message: string) => {
        if (channel !== CONVERGENCE_CHANNEL) {
            // Seguridad: ignorar mensajes de canales inesperados.
            return;
        }

        // 1. Parseo JSON con try/catch — nunca crashear por payload malformado.
        let payload: unknown;
        try {
            payload = JSON.parse(message);
        } catch (parseErr) {
            console.warn(
                '[ArteriaWSS] Invalid JSON on convergence channel, skipping. Raw (first 200 chars):',
                message.slice(0, 200)
            );
            return;
        }

        // 2. Validación mínima de shape — fail-honest: si no cumple el
        //    contrato, no re-emitir basura por WebSocket.
        if (!isValidConvergenceSignal(payload)) {
            console.warn(
                '[ArteriaWSS] Convergence signal shape validation failed, skipping. Keys found:',
                typeof payload === 'object' && payload !== null
                    ? Object.keys(payload)
                    : '(not an object)'
            );
            return;
        }

        // 3. Retransmisión al room `convergence` de Socket.IO.
        broadcastConvergenceSignal(io, payload);
    });

    return subscriber;
}

// ---------------------------------------------------------------------------
// Redis Pub/Sub bridge — Cartridge telemetry (FASE OMEGA)
// ---------------------------------------------------------------------------

const CARTRIDGE_TELEMETRY_CHANNEL = 'arbx:cartridge:telemetry';

/**
 * Emite un mensaje de telemetría de cartucho a los clientes suscritos al room
 * `telemetry`. Path principal: el puente Redis Pub/Sub en
 * {@link subscribeToCartridgeTelemetry}.
 */
export function broadcastCartridgeTelemetry(io: Server, msg: CartridgeTelemetry): void {
    io.to(CARTRIDGE_TELEMETRY_ROOM).emit('telemetry', msg);
}

/**
 * Inicia el puente Redis → WebSocket para la telemetría de cartuchos. Estructura
 * idéntica a {@link subscribeToConvergenceSignals}: instancia Redis DEDICADA
 * (modo SUBSCRIBE), suscripción a `arbx:cartridge:telemetry`, parseo JSON
 * tolerante a fallos, y re-emisión al room `telemetry`.
 *
 * **Fail-honest**: payload malformado se loguea y se descarta (skip), nunca
 * crashea. La telemetría es observacional: a diferencia de convergence (shape
 * estricto), aquí solo exigimos que el mensaje sea un objeto JSON — los cartuchos
 * pueden añadir campos arbitrarios.
 *
 * **Debe invocarse UNA SOLA VEZ** desde `index.ts`, tras `setupWebSocketGateway()`.
 *
 * @param io       — instancia de Socket.IO Server
 * @param redisUrl — URL de conexión Redis (REDIS_URL)
 * @returns        — instancia Redis subscriber (para cleanup en shutdown)
 */
export function subscribeToCartridgeTelemetry(
    io: Server,
    redisUrl: string,
    onMessage?: (msg: CartridgeTelemetry) => void,
): Redis {
    const subscriber = new Redis(redisUrl, {
        lazyConnect: false,
        maxRetriesPerRequest: 1,
        retryStrategy(times: number) {
            const delay = Math.min(times * 50, 2000);
            console.log(`[CartridgeTelemetry] Redis subscriber reconnect attempt ${times}, retrying in ${delay}ms`);
            return delay;
        },
        reconnectOnError(err) {
            const targetErrors = ['ECONNREFUSED', 'ETIMEDOUT', 'ECONNRESET', 'EHOSTUNREACH'];
            const shouldReconnect = targetErrors.some((code) => err.message.includes(code));
            return shouldReconnect ? 2 : false;
        },
    });

    subscriber.on('ready', () => {
        console.log('[CartridgeTelemetry] Redis subscriber ready');
    });
    subscriber.on('error', (err: Error) => {
        // Non-fatal — the rest of the WSS gateway keeps working.
        console.error('[CartridgeTelemetry] Redis subscriber error (non-fatal):', err.message);
    });

    subscriber.subscribe(CARTRIDGE_TELEMETRY_CHANNEL).then(() => {
        console.log(`[CartridgeTelemetry] Subscribed to Redis channel: ${CARTRIDGE_TELEMETRY_CHANNEL}`);
    }).catch((err: Error) => {
        console.error(`[CartridgeTelemetry] Failed to subscribe to ${CARTRIDGE_TELEMETRY_CHANNEL}:`, err.message);
    });

    subscriber.on('message', (channel: string, message: string) => {
        if (channel !== CARTRIDGE_TELEMETRY_CHANNEL) {
            return;
        }
        let payload: unknown;
        try {
            payload = JSON.parse(message);
        } catch {
            console.warn(
                '[CartridgeTelemetry] Invalid JSON, skipping. Raw (first 200 chars):',
                message.slice(0, 200),
            );
            return;
        }
        // Loose validation: telemetry is observational — only require a JSON object.
        if (typeof payload !== 'object' || payload === null) {
            console.warn('[CartridgeTelemetry] Non-object telemetry payload, skipping');
            return;
        }
        if (onMessage) {
            try { onMessage(payload as CartridgeTelemetry); } catch { /* cache feed is best-effort, never fatal */ }
        }
        broadcastCartridgeTelemetry(io, payload as CartridgeTelemetry);
    });

    return subscriber;
}

// ---------------------------------------------------------------------------
// Redis Pub/Sub bridge — Route Discovery telemetry (QUANTUM FULLSTACK SYMMETRY)
// ---------------------------------------------------------------------------

const ROUTE_DISCOVERY_TELEMETRY_CHANNEL = 'arbx:route_discovery:telemetry';

/**
 * Emite un mensaje de telemetría del radar a los clientes suscritos al room
 * `route_discovery`. Path principal: el puente Redis Pub/Sub en
 * {@link subscribeToRouteDiscoveryTelemetry}.
 *
 * NOTE: el nombre de evento (`route_discovery_telemetry`) es DISTINTO de
 * `telemetry` (cartuchos) a propósito — rooms y eventos separados evitan
 * cross-talk entre los dos streams.
 */
export function broadcastRouteDiscoveryTelemetry(io: Server, msg: RouteDiscoveryTelemetry): void {
    io.to(ROUTE_DISCOVERY_TELEMETRY_ROOM).emit('route_discovery_telemetry', msg);
}

/**
 * Inicia el puente Redis → WebSocket para la telemetría del radar Route
 * Discovery. Espejo 1:1 de {@link subscribeToCartridgeTelemetry}: instancia
 * Redis DEDICADA (modo SUBSCRIBE), suscripción a `arbx:route_discovery:telemetry`
 * (DEBE coincidir byte-a-byte con `telemetry.rs`), parseo JSON tolerante a
 * fallos, y re-emisión al room `route_discovery`.
 *
 * **Fail-honest**: payload malformado se loguea y se descarta; nunca crashea.
 * Validación LOOSE (objeto JSON) porque los 5 tipos de evento comparten solo el
 * discriminador `event`. El callback opcional `onMessage` alimenta el
 * `TelemetryCache` que sirve los endpoints REST de solo-lectura.
 *
 * **Debe invocarse UNA SOLA VEZ** desde `index.ts`, tras `setupWebSocketGateway()`.
 *
 * RULE 00 / NO-ACTIVE: solo observa y re-emite; jamás toca `arbx:opps:detected`
 * ni dispara ejecución.
 *
 * @param io       — instancia de Socket.IO Server
 * @param redisUrl — URL de conexión Redis (REDIS_URL)
 * @param onMessage — callback opcional para cachear cada evento (REST snapshot)
 * @returns        — instancia Redis subscriber (para cleanup en shutdown)
 */
export function subscribeToRouteDiscoveryTelemetry(
    io: Server,
    redisUrl: string,
    onMessage?: (msg: RouteDiscoveryTelemetry) => void,
): Redis {
    const subscriber = new Redis(redisUrl, {
        lazyConnect: false,
        maxRetriesPerRequest: 1,
        retryStrategy(times: number) {
            const delay = Math.min(times * 50, 2000);
            console.log(`[RouteDiscoveryTelemetry] Redis subscriber reconnect attempt ${times}, retrying in ${delay}ms`);
            return delay;
        },
        reconnectOnError(err) {
            const targetErrors = ['ECONNREFUSED', 'ETIMEDOUT', 'ECONNRESET', 'EHOSTUNREACH'];
            const shouldReconnect = targetErrors.some((code) => err.message.includes(code));
            return shouldReconnect ? 2 : false;
        },
    });

    subscriber.on('ready', () => {
        console.log('[RouteDiscoveryTelemetry] Redis subscriber ready');
    });
    subscriber.on('error', (err: Error) => {
        console.error('[RouteDiscoveryTelemetry] Redis subscriber error (non-fatal):', err.message);
    });

    subscriber.subscribe(ROUTE_DISCOVERY_TELEMETRY_CHANNEL).then(() => {
        console.log(`[RouteDiscoveryTelemetry] Subscribed to Redis channel: ${ROUTE_DISCOVERY_TELEMETRY_CHANNEL}`);
    }).catch((err: Error) => {
        console.error(`[RouteDiscoveryTelemetry] Failed to subscribe to ${ROUTE_DISCOVERY_TELEMETRY_CHANNEL}:`, err.message);
    });

    subscriber.on('message', (channel: string, message: string) => {
        if (channel !== ROUTE_DISCOVERY_TELEMETRY_CHANNEL) {
            return;
        }
        let payload: unknown;
        try {
            payload = JSON.parse(message);
        } catch {
            console.warn(
                '[RouteDiscoveryTelemetry] Invalid JSON, skipping. Raw (first 200 chars):',
                message.slice(0, 200),
            );
            return;
        }
        if (typeof payload !== 'object' || payload === null) {
            console.warn('[RouteDiscoveryTelemetry] Non-object telemetry payload, skipping');
            return;
        }
        if (onMessage) {
            try { onMessage(payload as RouteDiscoveryTelemetry); } catch { /* cache feed is best-effort, never fatal */ }
        }
        broadcastRouteDiscoveryTelemetry(io, payload as RouteDiscoveryTelemetry);
    });

    return subscriber;
}
