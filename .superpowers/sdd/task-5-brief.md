# Task Brief: WebSocket Real-Time Streaming

## Context

Este es el Task 5 del Plan Maestro OMEGA. Depende de Task 1 (Redis schema) y Task 2 (HotPathEmitter).

## Goal

Implementar streaming WebSocket en tiempo real para oportunidades calientes (<5ms emit latency).

## Files

- Modify: `backend/api-server/src/websocket.ts` - Add OpportunityHotStreamer class
- Modify: `edge/dev-local/src/index.ts` - Add /ws/hot-opportunities namespace
- Create: `frontend/lib/websocket-client.ts` - HotOpportunityWebSocket client + React hook

## Interfaces

- Consumes: Redis Streams arbx:hot:detected, arbx:hot:simulated (Task 1/2)
- Produces: Socket.IO events opportunity:detected, opportunity:validated

## Steps (exactos)

### Step 1: Add OpportunityHotStreamer to websocket.ts

Add class to `backend/api-server/src/websocket.ts`:

```typescript
// Consumer group for hot opportunity streaming
const HOT_OPPORTUNITIES_GROUP = 'ws-emitter-g0';
const HOT_DETECTED_STREAM = 'arbx:hot:detected';
const HOT_SIMULATED_STREAM = 'arbx:hot:simulated';

export interface HotOpportunityStreamerOptions {
  io: Server;
  redisUrl: string;
  logger?: Console;
}

export class OpportunityHotStreamer {
  private io: Server;
  private redis: Redis;
  private logger: Console;
  private running = false;
  private consumerName: string;

  constructor(opts: HotOpportunityStreamerOptions) {
    this.io = opts.io;
    this.redis = new Redis(opts.redisUrl);
    this.logger = opts.logger ?? console;
    this.consumerName = `ws-emitter-${process.pid}-${Date.now()}`;
  }

  async start(): Promise<void> {
    // Create consumer groups if not exist (ignore BUSYGROUP error)
    try {
      await this.redis.xgroup('CREATE', HOT_DETECTED_STREAM, HOT_OPPORTUNITIES_GROUP, '$', 'MKSTREAM');
    } catch (e: any) {
      if (!e.message?.includes('BUSYGROUP')) throw e;
    }
    try {
      await this.redis.xgroup('CREATE', HOT_SIMULATED_STREAM, HOT_OPPORTUNITIES_GROUP, '$', 'MKSTREAM');
    } catch (e: any) {
      if (!e.message?.includes('BUSYGROUP')) throw e;
    }

    this.running = true;
    this.logger.log('[HotStreamer] Consumer groups created, starting poll loop');

    // Start polling loops
    this.pollLoop(HOT_DETECTED_STREAM, 'opportunity:detected');
    this.pollLoop(HOT_SIMULATED_STREAM, 'opportunity:validated');
  }

  private async pollLoop(stream: string, eventName: string): Promise<void> {
    while (this.running) {
      try {
        // XREADGROUP with 1s block
        const results = await this.redis.xreadgroup(
          'GROUP', HOT_OPPORTUNITIES_GROUP, this.consumerName,
          'BLOCK', 1000,
          'STREAMS', stream, '>'
        ) as [string, [string, string[]][]][] | null;

        if (results) {
          for (const [, messages] of results) {
            for (const [id, fields] of messages) {
              const data = this.parseFields(fields);
              // Emit to all clients in 'opportunities' room
              const start = process.hrtime.bigint();
              this.io.to('opportunities').emit(eventName, { ...data, _stream_id: id });
              const elapsedNs = process.hrtime.bigint() - start;
              const elapsedMs = Number(elapsedNs) / 1_000_000;
              if (elapsedMs > 5) {
                this.logger.warn(`[HotStreamer] Slow emit: ${elapsedMs.toFixed(2)}ms > 5ms target`);
              }
            }
          }
        }
      } catch (e) {
        this.logger.error('[HotStreamer] Poll error:', e);
        await new Promise(r => setTimeout(r, 1000));
      }
    }
  }

  private parseFields(fields: string[]): Record<string, string> {
    const result: Record<string, string> = {};
    for (let i = 0; i < fields.length; i += 2) {
      result[fields[i]] = fields[i + 1];
    }
    return result;
  }

  async stop(): Promise<void> {
    this.running = false;
    await this.redis.quit();
  }
}
```

### Step 2: Integrate in api-server index.ts

In `backend/api-server/src/index.ts`, after `setupWebSocketGateway()`:

```typescript
import { OpportunityHotStreamer } from './websocket.js';

// ... after const io = setupWebSocketGateway(httpServer);

const hotStreamer = new OpportunityHotStreamer({ io, redisUrl: REDIS_URL, logger });
hotStreamer.start().catch(e => logger.error({ err: e.message }, 'hot streamer failed to start'));
```

### Step 3: Add Socket.IO namespace to edge

Add to `edge/dev-local/src/index.ts`:

```typescript
import { Server as SocketIOServer } from 'socket.io';

// After existing wsProxy setup, add:
const hotOpportunitiesNamespace = io.of('/ws/hot-opportunities');

hotOpportunitiesNamespace.on('connection', (socket) => {
  logger.info({ event: 'hot_opps.connect', socket: socket.id }, 'client connected to hot opportunities');
  
  // Join opportunities room
  socket.join('opportunities');
  
  // Send snapshot on connect (XREVRANGE latest 10)
  redisClient.xrevrange('arbx:hot:detected', '+', '-', 'COUNT', 10)
    .then(items => {
      socket.emit('snapshot', { opportunities: items, timestamp: Date.now() });
    })
    .catch(err => {
      logger.warn({ err: err.message }, 'failed to send hot opps snapshot');
    });
  
  socket.on('disconnect', () => {
    logger.info({ event: 'hot_opps.disconnect', socket: socket.id }, 'client disconnected');
  });
});
```

### Step 4: Create frontend WebSocket client

Create `frontend/lib/websocket-client.ts`:

```typescript
"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import { io, type Socket } from "socket.io-client";

export interface HotOpportunity {
  id: string;
  chain_id: string;
  strategy_kind: string;
  detected_at_ms: string;
  [key: string]: string;
}

export interface HotOpportunityCallbacks {
  onDetected?: (opp: HotOpportunity) => void;
  onValidated?: (opp: HotOpportunity) => void;
  onSnapshot?: (snapshot: { opportunities: unknown[]; timestamp: number }) => void;
  onError?: (error: Error) => void;
}

export interface HotOpportunityWebSocketOptions {
  url: string;
  authToken?: string;
  reconnectBackoffMs?: number;
  maxReconnectDelayMs?: number;
  onStatusChange?: (status: HotOpportunityStatus) => void;
}

export type HotOpportunityStatus = 
  | "idle"
  | "connecting" 
  | "connected"
  | "reconnecting"
  | "disconnected"
  | "error";

export class HotOpportunityWebSocket {
  private socket: Socket | null = null;
  private options: HotOpportunityWebSocketOptions;
  private callbacks: HotOpportunityCallbacks;
  private reconnectAttempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private running = false;

  constructor(
    options: HotOpportunityWebSocketOptions,
    callbacks: HotOpportunityCallbacks = {}
  ) {
    this.options = {
      reconnectBackoffMs: 1000,
      maxReconnectDelayMs: 30000,
      ...options,
    };
    this.callbacks = callbacks;
  }

  connect(): void {
    if (this.running) return;
    this.running = true;
    this.doConnect();
  }

  private doConnect(): void {
    this.setStatus("connecting");

    const { url, authToken } = this.options;
    const connectOpts: Record<string, unknown> = {
      reconnection: false, // We handle reconnection manually
      timeout: 5000,
    };

    if (authToken) {
      connectOpts.auth = { token: authToken };
      connectOpts.query = { token: authToken };
      connectOpts.extraHeaders = { "x-arbx-admin-token": authToken };
    }

    this.socket = io(url, connectOpts);

    this.socket.on("connect", () => {
      this.reconnectAttempt = 0;
      this.setStatus("connected");
    });

    this.socket.on("disconnect", () => {
      this.setStatus("disconnected");
      if (this.running) this.scheduleReconnect();
    });

    this.socket.on("connect_error", (err) => {
      this.setStatus("error");
      this.callbacks.onError?.(err);
      if (this.running) this.scheduleReconnect();
    });

    this.socket.on("opportunity:detected", (data: HotOpportunity) => {
      this.callbacks.onDetected?.(data);
    });

    this.socket.on("opportunity:validated", (data: HotOpportunity) => {
      this.callbacks.onValidated?.(data);
    });

    this.socket.on("snapshot", (data: { opportunities: unknown[]; timestamp: number }) => {
      this.callbacks.onSnapshot?.(data);
    });
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) return;

    this.setStatus("reconnecting");
    const delay = Math.min(
      this.options.reconnectBackoffMs! * Math.pow(2, this.reconnectAttempt),
      this.options.maxReconnectDelayMs!
    );
    this.reconnectAttempt++;

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      if (this.running) this.doConnect();
    }, delay);
  }

  private setStatus(status: HotOpportunityStatus): void {
    this.options.onStatusChange?.(status);
  }

  disconnect(): void {
    this.running = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.socket?.disconnect();
    this.socket = null;
    this.setStatus("disconnected");
  }
}

export interface UseHotOpportunitiesOptions {
  url?: string;
  authToken?: string;
  enabled?: boolean;
}

export interface UseHotOpportunitiesReturn {
  status: HotOpportunityStatus;
  opportunities: HotOpportunity[];
  reconnect: () => void;
}

export function useHotOpportunities(
  options: UseHotOpportunitiesOptions = {}
): UseHotOpportunitiesReturn {
  const { 
    url = process.env.NEXT_PUBLIC_WS_URL ?? "ws://localhost:8788/ws/hot-opportunities",
    authToken,
    enabled = true,
  } = options;

  const [status, setStatus] = useState<HotOpportunityStatus>("idle");
  const [opportunities, setOpportunities] = useState<HotOpportunity[]>([]);
  const [reconnectTick, setReconnectTick] = useState(0);
  const clientRef = useRef<HotOpportunityWebSocket | null>(null);

  const addOpportunity = useCallback((opp: HotOpportunity) => {
    setOpportunities((prev) => {
      if (prev.some((p) => p.id === opp.id)) return prev;
      return [opp, ...prev].slice(0, 100);
    });
  }, []);

  useEffect(() => {
    if (!enabled || typeof window === "undefined") {
      setStatus("idle");
      return;
    }

    const client = new HotOpportunityWebSocket(
      { url, authToken, onStatusChange: setStatus },
      {
        onDetected: addOpportunity,
        onValidated: addOpportunity,
        onSnapshot: (snap) => {
          const items = snap.opportunities as HotOpportunity[];
          setOpportunities(items.slice(0, 100));
        },
      }
    );

    clientRef.current = client;
    client.connect();

    return () => {
      client.disconnect();
      clientRef.current = null;
    };
  }, [url, authToken, enabled, reconnectTick, addOpportunity]);

  const reconnect = useCallback(() => {
    setReconnectTick((t) => t + 1);
  }, []);

  return { status, opportunities, reconnect };
}
```

### Step 5: Typecheck api-server

```bash
cd backend/api-server && npm run typecheck 2>&1 | head -50
```
Expected: No type errors

### Step 6: Typecheck frontend

```bash
cd frontend && npx tsc --noEmit -p tsconfig.json 2>&1 | head -50
```
Expected: No type errors

### Step 7: Commit

```bash
git add backend/api-server/src/websocket.ts backend/api-server/src/index.ts edge/dev-local/src/index.ts frontend/lib/websocket-client.ts
git commit -m "feat(websocket): add hot opportunity streaming with <5ms latency target"
```

## Acceptance Criteria

- [ ] OpportunityHotStreamer class added to websocket.ts with XREADGROUP
- [ ] Consumer group ws-emitter-g0 created for both streams
- [ ] Emits opportunity:detected and opportunity:validated events
- [ ] Edge namespace /ws/hot-opportunities with snapshot on connect
- [ ] Frontend HotOpportunityWebSocket client with auto-reconnect
- [ ] React hook useHotOpportunities() with opportunity buffer
- [ ] Typecheck passes for api-server
- [ ] Typecheck passes for frontend
- [ ] Commiteado

## Out of Scope

- Tests (covered en Task 8)
- Performance benchmarking (Task 7)
- Production deployment config
