# PLAN — Precios a Velocidad de la Luz (G-PRICE Speed-of-Light)

> **Objetivo:** Cuando un precio cambia, TODAS las cards existentes se re-precian en <100ms vía streaming push. Sin polling. Sin stale data.

---

## ESTADO ACTUAL (verificado en prod)

| Componente | Estado | Cadencia | Latencia |
|---|---|---|---|
| price_worker (Chainlink+Alchemy) | ✅ Funcionando | 30s exactos | 3-6s por tick |
| DexScreener | ✅ Funcionando | 15s | ~1s |
| GeckoTerminal | ⚠️ 429 flood | 60s | BROKEN (429 cada 2.1s) |
| WS push `prices:update` | ✅ Funcionando | Al cambiar hash | 19.5s (bounded por tick 30s) |
| REST `/api/prices/live` | ✅ Funcionando | On-demand | ~200ms |
| Frontend `PriceTicker` | ✅ Funcionando | Push + poll fallback 4s | — |
| **Cards re-precian** | ❌ **NO** | Nunca | G-PRICE-2 OPEN |
| **Chainlink subscribe** | ❌ **NO** | Poll 30s | G-PRICE-3 OPEN |
| **GeckoTerminal CB** | ❌ **NO** | Sin breaker | G-PRICE-5 OPEN |

---

## LOS 3 FIXES (cada uno su PR)

### FIX 1 · G-PRICE-2: Re-precio de cards via `opportunity_updated`

**Problema:** Cuando llega un `prices:update`, las cards existentes en pantalla NO se re-precian. Solo el `PriceTicker` header se actualiza. Las cards muestran los precios del momento en que fueron detectadas.

**Solución:**
```
prices:update (WS push)
  → Frontend usePricesStream ya lo recibe
  → NUEVO: hook recalcula USD values de las cards visibles
  → Cards re-render con precios frescos SIN refetch de la API
  → Solo los valores derivados cambian (amountInUsd, expectedProfit, etc.)
  → React.memo + useMemo para no re-renderear lo que no cambió
```

**Archivos:**
- `frontend/lib/hooks/usePriceDerivedValues.ts` (nuevo): hook que recibe prices + opp, calcula derived USD
- `frontend/components/opportunities/exchange/OpportunitiesExchangeClient.tsx`: conectar hook
- `frontend/components/OpportunitiesClient.tsx`: conectar hook

**Verificación:** Cambiar un precio manualmente en Redis → cards visibles actualizan en <100ms

---

### FIX 2 · G-PRICE-3: Chainlink event-driven (no poll)

**Problema:** Chainlink es la fuente Tier-0 más confiable pero se pollea cada 30s. Un update del agregador on-chain podría tardar hasta 30s en llegar.

**Solución:**
```
NUEVO: ChainlinkAggregatorSubscriber (Rust)
  → WebSocket subscribe a eventos del AggregatorProxy
  → Evento AnswerUpdated(int256 current, uint256 roundId, uint256 updatedAt)
  → On event: escribir DIRECTAMENTE al hash Redis + PUBLISH
  → Sin esperar el tick de 30s del price_worker
  → price_worker queda como fallback de reconciliación
```

**Archivos:**
- `backend/searcher-rs/src/workers/chainlink_subscriber.rs` (nuevo)
- `backend/searcher-rs/src/main.rs`: spawn del subscriber
- Solo para los 5 feeds Chainlink que ya consume el price_worker (WETH, USDC, USDT, DAI, WBTC)

**Latencia objetivo:** On-chain update → card re-priced en <500ms (vs 30s actuales)

---

### FIX 3 · G-PRICE-5: GeckoTerminal circuit breaker

**Problema:** GeckoTerminal emite WARN 429 cada 2.1s sin circuit breaker. El patrón CG429-01 (CoinGecko) ya tiene backoff 300s implementado — hay que espejarlo.

**Solución:** Copiar el patrón CG429-01:
- 3 consecutive 429s → abrir circuit breaker (skip calls por 300s)
- Log UNA línea al abrir + UNA al cerrar (no 429-flood)
- Half-open probe después del cooldown

**Archivo:** `backend/token-enricher/src/geckoterminal.rs` (o donde esté el fetch loop)

---

## HARDENING (gates para que NUNCA se degrade en silencio)

### H1 · Price freshness gate
```typescript
// En api-server prices-stream.ts:
// Si el hash no cambia en >120s (4 ticks del worker), emitir alerta:
prices:stale { chain_id, last_update_age_secs, expected_max_age: 30 }
```

### H2 · Price deviation alarm
```typescript
// Si un precio salta >20% entre ticks consecutivos, marcar como sospechoso:
prices:anomaly { token, old_price, new_price, deviation_pct }
```

### H3 · WS bridge resilience
```typescript
// El api-server WS bridge ya tiene reconexión. Añadir:
// - Heartbeat cada 30s al pubsub
// - Si psubscribe se cae, re-subscribe automáticamente y envía snapshot fresco
```

### H4 · CI gate
```bash
# automation/tools/gate-price-freshness.sh:
# Verificar que price_worker está en el compose (no se puede quitar silenciosamente)
# Verificar que subscribeToPriceUpdates está wired en prices-stream.ts
# Verificar que PriceTicker existe en el frontend
```

---

## ORDEN DE EJECUCIÓN

```
1. FIX 3 (GeckoTerminal CB)     → 30 min, sin riesgo, limpia logs
2. FIX 1 (Cards re-precio)      → 2h, frontend only, visible inmediatamente
3. FIX 2 (Chainlink subscribe)  → 4h, Rust, el mayor impacto en latencia
4. H1-H4 (Hardening)            → 1h, gates y alarmas
```

**Total estimado:** 1 día de trabajo → sistema de precios production-grade.

---

## ARQUITECTURA FINAL (después de todos los fixes)

```
┌─ ON-CHAIN (event-driven) ─────────────────────────────┐
│ Chainlink AnswerUpdated events → subscriber → Redis   │  ← FIX 2
│ (latencia: block → Redis < 2s)                        │
└───────────────────────────────────────────────────────┘

┌─ OFF-CHAIN (poll + reconcile) ────────────────────────┐
│ price_worker 30s: Chainlink + Alchemy (5 + 33 tokens) │
│ DexScreener 15s: 300+ tokens del top                 │
│ GeckoTerminal 60s: long-tail (con CB)                 │  ← FIX 3
└───────────────────────────────────────────────────────┘
           │
           ▼
┌─ REDIS (SSOT) ────────────────────────────────────────┐
│ arbx:token_prices:<chain> (hash, TTL 53s)             │
│ PUBLISH arbx:prices:updated:<chain> on every write    │
└───────────────────────────────────────────────────────┘
           │
           ▼
┌─ API-SERVER (WS bridge) ─────────────────────────────┐
│ psubscribe → HGETALL → emit prices:update to room    │
│ + staleness gate (H1) + anomaly alarm (H2)           │
└───────────────────────────────────────────────────────┘
           │
           ▼
┌─ FRONTEND (velocidad de la luz) ─────────────────────┐
│ usePricesStream: WS push → inmediato                 │
│ usePriceDerivedValues: re-calcula USD de cards       │  ← FIX 1
│ Cards re-render: SOLO las que dependen del precio    │
│   que cambió (React.memo + key comparison)           │
│ PriceTicker header: siempre fresco                   │
└───────────────────────────────────────────────────────┘
```

**Latencia objetivo end-to-end:**
- Chainlink on-chain update → card visible: **< 2s** (vs 30s actuales)
- DexScreener update → card visible: **< 1s**
- Cualquier fuente → WS push: **< 50ms** (ya logrado)
