import { Server } from 'socket.io';
import { Server as HttpServer } from 'http';
import { safeTokenEqual } from '@arbx/shared';

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

        socket.on('disconnect', () => {
            console.log(`[WebSocket] Cliente desconectado: ${socket.id}`);
        });
    });

    return io;
}

// Simulador de emisión de oportunidades
export function broadcastOpportunity(io: Server, opp: any) {
    io.to('opportunities').emit('new_opportunity', opp);
}
