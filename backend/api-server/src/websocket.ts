import { Server } from 'socket.io';
import { Server as HttpServer } from 'http';
import { Redis } from 'ioredis';
import { safeTokenEqual } from '@arbx/shared';

// ---------------------------------------------------------------------------
// Tipos públicos
// ---------------------------------------------------------------------------

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
    const expectedAdminToken = process.env['ARBX_ADMIN_TOKEN'] ?? '';
    io.use((socket, next) => {
        const got = extractHandshakeToken(socket.handshake);
        if (!got || !safeTokenEqual(got, expectedAdminToken)) {
            return next(new Error('unauthorized: invalid or missing admin token'));
        }
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
