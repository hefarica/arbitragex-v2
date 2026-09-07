# WO-01 — APPLY: listener `new_opportunity` en `frontend/lib/websocket-client.ts`

**Fecha:** 2026-09-06 · **Estado:** APPLIED_VERIFIED · **Scope:** solo archivos del charter (cliente WS + su test). Cero git, cero deploy.

## 1. Hallazgo (CRITICO N3 #1 + informe /goal)

El api-server emite el broadcast insignia `new_opportunity` al room `opportunities`
(`backend/api-server/src/websocket.ts:339-341`, `broadcastOpportunity`, alimentado por
PostgreSQL `LISTEN opportunities_channel` en `backend/api-server/src/index.ts:1847-1863`),
pero el cliente `HotOpportunityWebSocket` (`frontend/lib/websocket-client.ts`) SOLO
registraba listeners para `opportunity:detected` / `opportunity:validated` (eventos del
`OpportunityHotStreamer`, streams Redis `arbx:hot:*`). El broadcast llegaba a nadie en ese
cliente.

**Delimitación honesta del alcance:** `frontend/features/opportunities/socket-lifecycle.ts:81`
(feed de la página /opportunities vía `useOpportunitiesStream`) YA escucha `new_opportunity`.
El gap era específicamente la clase `HotOpportunityWebSocket` de `frontend/lib/` —
este WO cierra ese gap; no toca el flujo features/.

## 2. Contrato del server verificado (lectura, no suposición)

- Emisor: `broadcastOpportunity(io, opp)` → `io.to('opportunities').emit('new_opportunity', opp)`.
- Fuente del payload: trigger PG `trg_notify_opportunity` AFTER INSERT ON opportunities
  (`backend/api-server/inject-trigger.mjs:10-21`) → `pg_notify('opportunities_channel', row_to_json(NEW)::text)`
  → `index.ts:1853-1854` parsea el JSON y lo pasa intacto. **El payload es una fila
  completa de la tabla `opportunities`.**
- Columnas del escritor canónico (`backend/searcher-rs/src/persistence.rs:148-160`,
  struct `shared_rs::contracts::Opportunity`):
  `id` (uuid → JSON **string**), `chain_id` (u64/bigint → JSON **number**),
  `strategy_kind` (**string**), `dex_a`, `dex_b?`, `pair_symbol`, `token_in`, `token_out`,
  `amount_in_wei`, `expected_profit_usd` / `net_expected_profit_usd` (USD, number|null),
  `roi_pct?`, `risk_score?`, `block_number?`, `status` (enum PG
  `'detected'|'validated'|'simulated'|'scored'|'executing'|'executed'|'reconciled'|'rejected'|'failed'`,
  migration 003), `rejection_reason?`, `trace_id`, `detected_at` (timestamptz → **ISO 8601 string**),
  `route_metadata` (jsonb), `cartridge_id?`.

## 3. Cambio aplicado (aditivo y quirúrgico)

`frontend/lib/websocket-client.ts` — dos adiciones, cero líneas existentes modificadas:

1. **Adaptador puro exportado** `adaptNewOpportunityToHotEvent(payload: unknown): HotOpportunityEvent | null`
   — mapea SOLO campos con correspondencia 1:1 veraz (RULE 00 / R8 fail-honest):
   - `id` (string uuid) → `id`.
   - `chain_id` number → `chain_id` string (`String(n)`); string pasa directo.
   - `strategy_kind` string → directo.
   - `detected_at` ISO → `detected_at_ms` epoch-ms string (`Date.parse`); si no es
     parseable se OMITE el campo — jamás se emite `"NaN"`.
   - **NO se fabrican** `status` (el enum PG no pertenece al union `"passed"|"failed"`
     del hot stream), `net_profit_wei` / `gas_used` (el row trae USD, no wei) ni
     `timestamp_ms` (no existe en el row).
   - Payload no-objeto o sin `id` uuid string → `null` (descarte fail-honest).
2. **Listener aditivo** en `connect()` tras `opportunity:validated`: despacha el payload
   adaptado al MISMO flujo que `opportunity:detected` (`onDetectedCallbacks`); `null`
   → descarte silencioso sin throw. Los listeners existentes quedan intactos.

### Diff completo — `frontend/lib/websocket-client.ts` (modificado)

```diff
diff --git a/frontend/lib/websocket-client.ts b/frontend/lib/websocket-client.ts
index 6ded6e24..c81f98af 100644
--- a/frontend/lib/websocket-client.ts
+++ b/frontend/lib/websocket-client.ts
@@ -32,6 +32,56 @@ export interface WebSocketClientOptions {
   logger?: Console;
 }
 
+/**
+ * WO-01 (2026-09-06): adapta el payload del evento `new_opportunity` al shape
+ * `HotOpportunityEvent` que consumen los callbacks `onDetected`.
+ *
+ * Contrato del server (backend/api-server/src/websocket.ts:339
+ * `broadcastOpportunity` ← index.ts:1847-1863 LISTEN `opportunities_channel`
+ * ← trigger PG `trg_notify_opportunity` AFTER INSERT, payload =
+ * `row_to_json(NEW)`): una fila completa de la tabla `opportunities` (escritor
+ * canónico: backend/searcher-rs/src/persistence.rs). Campos relevantes:
+ *   id (uuid string) · chain_id (number) · strategy_kind (string) ·
+ *   detected_at (ISO 8601 string) · status ('detected'|…|'rejected'|'failed')
+ *   · expected_profit_usd / net_expected_profit_usd (USD, number|null).
+ *
+ * RULE 00 / R8 fail-honest — solo se mapean campos con correspondencia 1:1
+ * veraz; NUNCA se fabrican valores:
+ *   - `status` del row PG ('detected' etc.) NO pertenece al union
+ *     "passed" | "failed" del hot stream → se omite.
+ *   - `net_profit_wei` / `gas_used` no existen en el row PG (el row trae USD,
+ *     no wei) → se omiten.
+ *   - `detected_at` ISO se convierte a `detected_at_ms` (epoch ms string); si
+ *     no es parseable se omite el campo, jamás se emite "NaN".
+ *
+ * @returns null cuando el payload no es objeto o carece de `id` uuid string
+ *          (payload corrupto → se descarta, R8).
+ */
+export function adaptNewOpportunityToHotEvent(
+  payload: unknown,
+): HotOpportunityEvent | null {
+  if (typeof payload !== "object" || payload === null) return null;
+  const row = payload as Record<string, unknown>;
+  if (typeof row.id !== "string" || row.id.length === 0) return null;
+
+  const event: HotOpportunityEvent = { id: row.id };
+  if (typeof row.chain_id === "number") {
+    event.chain_id = String(row.chain_id);
+  } else if (typeof row.chain_id === "string") {
+    event.chain_id = row.chain_id;
+  }
+  if (typeof row.strategy_kind === "string") {
+    event.strategy_kind = row.strategy_kind;
+  }
+  if (typeof row.detected_at === "string") {
+    const ms = Date.parse(row.detected_at);
+    if (!Number.isNaN(ms)) {
+      event.detected_at_ms = String(ms);
+    }
+  }
+  return event;
+}
+
 /**
  * Cliente WebSocket para recibir oportunidades hot path en tiempo real.
  * Conecta al namespace /ws/hot-opportunities y se suscribe al room 'opportunities'.
@@ -97,6 +147,19 @@ export class HotOpportunityWebSocket {
       this.onValidatedCallbacks.forEach((cb) => cb(data));
     });
 
+    // WO-01 (2026-09-06): el api-server emite el broadcast insignia
+    // `new_opportunity` (PostgreSQL LISTEN opportunities_channel →
+    // broadcastOpportunity) al MISMO room `opportunities`; este cliente antes
+    // no lo escuchaba — el broadcast llegaba a nadie. Listener ADITIVO:
+    // despacha al mismo flujo que `opportunity:detected` con el payload PG
+    // adaptado (adaptNewOpportunityToHotEvent); payload corrupto se descarta
+    // (R8 fail-honest). Los listeners existentes quedan intactos.
+    this.socket.on("new_opportunity", (data: unknown) => {
+      const adapted = adaptNewOpportunityToHotEvent(data);
+      if (adapted === null) return;
+      this.onDetectedCallbacks.forEach((cb) => cb(adapted));
+    });
+
     this.socket.on("error", (err: { code: string; room?: string }) => {
       this.opts.logger?.error("[HotOpportunityWebSocket] Server error:", err);
     });
```

### Archivo nuevo — `frontend/lib/websocket-client.test.ts`

Búsqueda `**/websocket-client*.test.*` (glob sobre el repo): **no existía suite** — se
creó una. Mockea `socket.io-client` con `vi.mock` (fake socket con handlers por evento,
mismo patrón que `features/opportunities/socket-lifecycle.test.ts`). 12 tests:

**`adaptNewOpportunityToHotEvent` (fn pura, fixture = row PG fiel al INSERT canónico):**
1. mapea `id`, `chain_id` (number→string), `strategy_kind`, `detected_at` (ISO→ms string);
   `detected_at_ms` siempre string numérico (nunca "NaN").
2. NO fabrica `status` / `net_profit_wei` / `gas_used` / `timestamp_ms` (R8).
3. `detected_at` no parseable → omite `detected_at_ms`, conserva el resto.
4. `chain_id` string pasa directo.
5. payload corrupto (`null`, `undefined`, string, number, array, `{}`, `id` number, `id` "") → `null`.

**`HotOpportunityWebSocket` (wiring con socket fake):**
6. `connect()` registra el listener `new_opportunity` (aditivo).
7. evento `connect` del socket → emite `subscribe:opportunities` (room compartido).
8. `new_opportunity` con row PG → despacha a `onDetected` con payload adaptado.
9. `new_opportunity` corrupto → NO despacha ni lanza (fail-honest).
10. `opportunity:detected` sigue despachando SIN adaptación (misma referencia de objeto
    — guard de no-regresión del listener existente).
11. `opportunity:validated` sigue despachando a `onValidated` (intacto).
12. `off()` de `onDetected` remueve el callback para AMBOS eventos.

```ts
// frontend/lib/websocket-client.test.ts
// WO-01 (2026-09-06) — suite nueva: no existía test del cliente WS.
//
// Cobertura del contrato de eventos del room `opportunities`:
//   server emite → "new_opportunity"          (PG LISTEN, row_to_json(NEW))
//   server emite → "opportunity:detected"     (hot streamer, Redis stream)
//   server emite → "opportunity:validated"    (hot streamer, Redis stream)
//   cliente emite → "subscribe:opportunities" (on connect)
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("socket.io-client", () => ({ io: vi.fn() }));

import { io } from "socket.io-client";
import {
  HotOpportunityWebSocket,
  adaptNewOpportunityToHotEvent,
} from "./websocket-client";

type Handler = (...args: unknown[]) => void;

// Anotación explícita (mismo patrón que socket-lifecycle.test.ts) — rompe el
// ciclo de auto-referencia de `on` que dispara TS7022.
type MockSocket = {
  connected: boolean;
  on: (event: string, handler: Handler) => unknown;
  emit: (event: string, ...args: unknown[]) => unknown;
  disconnect: () => void;
  trigger: (event: string, ...args: unknown[]) => void;
};

function makeFakeSocket(): MockSocket {
  const handlers = new Map<string, Handler>();
  const socket: MockSocket = {
    connected: false,
    on: vi.fn((event: string, handler: Handler) => {
      handlers.set(event, handler);
      return socket;
    }),
    emit: vi.fn(),
    disconnect: vi.fn(),
    trigger: (event: string, ...args: unknown[]) =>
      handlers.get(event)?.(...args),
  };
  return socket;
}

// Fiel al escritor canónico (backend/searcher-rs/src/persistence.rs,
// INSERT INTO opportunities): columnas y tipos tal como los serializa
// row_to_json(NEW) — id uuid string, chain_id number, detected_at ISO string.
const PG_ROW = {
  id: "3f9c2b1e-8a7d-4c5b-9e6f-1a2b3c4d5e6f",
  chain_id: 1,
  strategy_kind: "dex_arb",
  dex_a: "uniswap-v2",
  dex_b: "sushiswap",
  pair_symbol: "WETH/USDC",
  token_in: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  token_out: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  amount_in_wei: "1000000000000000000",
  expected_profit_usd: 12.34,
  net_expected_profit_usd: 8.21,
  roi_pct: 0.42,
  risk_score: 0.11,
  block_number: 21000000,
  status: "detected",
  rejection_reason: null,
  trace_id: "7c1d9e0a-52f4-4b8a-a3d6-9f0e1d2c3b4a",
  detected_at: "2026-09-06T12:34:56.789+00:00",
  route_metadata: {},
  cartridge_id: null,
};

// ─── adaptNewOpportunityToHotEvent (fn pura) ──────────────────────────────────

describe("adaptNewOpportunityToHotEvent — contrato PG row_to_json(NEW)", () => {
  it("mapea id, chain_id (number→string), strategy_kind y detected_at (ISO→ms string)", () => {
    const adapted = adaptNewOpportunityToHotEvent(PG_ROW);

    expect(adapted).toEqual({
      id: PG_ROW.id,
      chain_id: "1",
      strategy_kind: "dex_arb",
      detected_at_ms: String(Date.parse("2026-09-06T12:34:56.789+00:00")),
    });
    // detected_at_ms es SIEMPRE un string numérico — nunca "NaN".
    expect(/^\d+$/.test(adapted!.detected_at_ms!)).toBe(true);
  });

  it("NO fabrica status/net_profit_wei/gas_used/timestamp_ms (R8 fail-honest)", () => {
    const adapted = adaptNewOpportunityToHotEvent(PG_ROW)!;

    // El row PG trae status='detected' y USD, no wei — nada de eso se inventa.
    expect(adapted.status).toBeUndefined();
    expect(adapted.net_profit_wei).toBeUndefined();
    expect(adapted.gas_used).toBeUndefined();
    expect(adapted.timestamp_ms).toBeUndefined();
  });

  it("detected_at no parseable → omite detected_at_ms pero conserva el resto", () => {
    const adapted = adaptNewOpportunityToHotEvent({
      ...PG_ROW,
      detected_at: "not-a-date",
    })!;

    expect(adapted.detected_at_ms).toBeUndefined();
    expect(adapted.id).toBe(PG_ROW.id);
    expect(adapted.chain_id).toBe("1");
  });

  it("chain_id como string pasa directo (tolerancia de shape)", () => {
    const adapted = adaptNewOpportunityToHotEvent({
      ...PG_ROW,
      chain_id: "8453",
    })!;

    expect(adapted.chain_id).toBe("8453");
  });

  it("payload corrupto → null (R8: descartar, jamás despachar basura)", () => {
    expect(adaptNewOpportunityToHotEvent(null)).toBeNull();
    expect(adaptNewOpportunityToHotEvent(undefined)).toBeNull();
    expect(adaptNewOpportunityToHotEvent("string")).toBeNull();
    expect(adaptNewOpportunityToHotEvent(42)).toBeNull();
    expect(adaptNewOpportunityToHotEvent([1, 2])).toBeNull();
    expect(adaptNewOpportunityToHotEvent({})).toBeNull();
    // El contrato del server SIEMPRE trae id uuid string — otro tipo se descarta.
    expect(adaptNewOpportunityToHotEvent({ ...PG_ROW, id: 42 })).toBeNull();
    expect(adaptNewOpportunityToHotEvent({ ...PG_ROW, id: "" })).toBeNull();
  });
});

// ─── HotOpportunityWebSocket — wiring de listeners ───────────────────────────

describe("HotOpportunityWebSocket — listeners del room opportunities", () => {
  let fake: MockSocket;

  beforeEach(() => {
    fake = makeFakeSocket();
    vi.mocked(io).mockClear();
    vi.mocked(io).mockImplementation(() => fake as never);
  });

  function makeClient() {
    return new HotOpportunityWebSocket({
      url: "http://localhost:8080",
      token: "test-token",
    });
  }

  it("connect() registra el listener new_opportunity (aditivo)", () => {
    makeClient().connect();

    expect(fake.on).toHaveBeenCalledWith(
      "new_opportunity",
      expect.any(Function),
    );
  });

  it("'connect' del socket emite subscribe:opportunities (room compartido por ambos eventos)", () => {
    makeClient().connect();

    fake.trigger("connect");

    expect(fake.emit).toHaveBeenCalledWith("subscribe:opportunities");
  });

  it("new_opportunity (row PG) despacha a onDetected con payload adaptado", () => {
    const client = makeClient();
    const onDetected = vi.fn();
    client.onDetected(onDetected);
    client.connect();

    fake.trigger("new_opportunity", PG_ROW);

    expect(onDetected).toHaveBeenCalledTimes(1);
    expect(onDetected).toHaveBeenCalledWith({
      id: PG_ROW.id,
      chain_id: "1",
      strategy_kind: "dex_arb",
      detected_at_ms: String(Date.parse("2026-09-06T12:34:56.789+00:00")),
    });
  });

  it("new_opportunity corrupto NO despacha ni lanza (fail-honest)", () => {
    const client = makeClient();
    const onDetected = vi.fn();
    client.onDetected(onDetected);
    client.connect();

    expect(() => fake.trigger("new_opportunity", "basura")).not.toThrow();
    expect(() => fake.trigger("new_opportunity", null)).not.toThrow();

    expect(onDetected).not.toHaveBeenCalled();
  });

  it("opportunity:detected sigue despachando SIN adaptación (referencia intacta)", () => {
    const client = makeClient();
    const onDetected = vi.fn();
    client.onDetected(onDetected);
    client.connect();

    const hotPayload = {
      id: "stream-1",
      chain_id: "1",
      strategy_kind: "dex_arb",
      status: "passed" as const,
      net_profit_wei: "1000",
    };
    fake.trigger("opportunity:detected", hotPayload);

    expect(onDetected).toHaveBeenCalledTimes(1);
    // Mismo objeto por referencia — el listener existente no fue modificado.
    expect(onDetected).toHaveBeenCalledWith(hotPayload);
    expect(onDetected.mock.calls[0]![0]).toBe(hotPayload);
  });

  it("opportunity:validated sigue despachando a onValidated (intacto)", () => {
    const client = makeClient();
    const onValidated = vi.fn();
    client.onValidated(onValidated);
    client.connect();

    const hotPayload = { id: "stream-1", status: "failed" as const };
    fake.trigger("opportunity:validated", hotPayload);

    expect(onValidated).toHaveBeenCalledTimes(1);
    expect(onValidated).toHaveBeenCalledWith(hotPayload);
  });

  it("unsubscribe de onDetected remueve el callback para ambos eventos", () => {
    const client = makeClient();
    const onDetected = vi.fn();
    const off = client.onDetected(onDetected);
    client.connect();

    fake.trigger("opportunity:detected", { id: "a" });
    fake.trigger("new_opportunity", PG_ROW);
    expect(onDetected).toHaveBeenCalledTimes(2);

    off();
    fake.trigger("opportunity:detected", { id: "b" });
    fake.trigger("new_opportunity", PG_ROW);
    expect(onDetected).toHaveBeenCalledTimes(2);
  });
});
```

## 4. Verificación (comandos EXACTOS ejecutados)

Todos desde `c:/Users/HFRC/Desktop/arbitragex-v2-main (17)/frontend` (FE sin lockfile;
vitest bin en `frontend/node_modules/.bin`):

| Comando | Salida resumida |
|---|---|
| `npx vitest run lib/websocket-client.test.ts` | `Test Files 1 passed (1) · Tests 12 passed (12)` (corrido 2 veces: inicial y post-refactor; +1 final sobre estado final del archivo) |
| `npx tsc --noEmit` | `TSC_EXIT=0`, sin errores (corrido sobre el estado final tras corrección de comentario) |
| `npx vitest run lib features/opportunities` (no-regresión) | `Test Files 49 passed (49) · Tests 510 passed (510)` |

Detalle fail-honest del proceso: la primera pasada de `tsc` reportó `TS7022` en el test
(auto-referencia del fake socket sin anotación de tipo); se corrigió con anotación
explícita `const socket: MockSocket` (patrón ya usado en `socket-lifecycle.test.ts`) y se
re-verificó en el estado final.

## 5. Cumplimiento del charter

- Solo se editaron `frontend/lib/websocket-client.ts` y se creó `frontend/lib/websocket-client.test.ts`.
- Cero git (add/commit/push/stash/checkout), cero deploy, cero VPS, cero broadcast.
- Cambio aditivo: ninguna línea preexistente del cliente modificada (diff lo evidencia).
- RULE 00: adaptación mapea únicamente campos con fuente veraz en el row PG; nada fabricado.
- R8: payload corrupto → descarte con `null`, sin throw, sin re-etiquetar.

## 6. Nota para el operador (fuera de charter, solo observación)

El server NO emite `_stream_id` en `new_opportunity` (solo el hot streamer lo agrega a
`opportunity:detected`/`opportunity:validated`); el adaptador lo omite coherentemente.
Adicionalmente, `frontend/features_backup/opportunities/socket-lifecycle.ts` (backup, no
recolectado por vitest según include patterns) también escucha `new_opportunity` — sin
acción requerida.
