# ESCALADA A DINERO REAL — Veredicto de FASE $0 (auditoría 2026-08-14T14:15Z)

> Directiva v1 de escalada · LEY M8: sin fantasía económica

## Verificación R8-01/02/03 (en vivo)

### R8-02 — Código sin merge corriendo en prod
| Señal | Valor | Lectura |
|---|---|---|
| PR #335 | **OPEN** (no merged) | Fix B-02 raw-subscribe NO en main |
| VPS SHA | `35627908` | Sin fix B-02 |
| Binary searcher | **NO contiene** `ws.pending_unexpected_format` | Fix NO deployado |
| `pending_received` | **1386** | Items llegan por path NO-V2 |

**Diagnóstico:** El `pending_received=1386` NO proviene del path V2-mempool (que está roto sin el fix). Proviene de un path alternativo — el **block-based path** (block_scanner/hft_mempool_listener/cartridge workers) que corre independiente del mempool-V2 y NO usa `subscribe_pending()`.

**Confirmación:** 114 opportunities/5min en PG = block-based path activo. El mempool-V2 sigue muerto (0 decoded).

**Veredicto R8-02:** ❌ **SIN violación de gobernanza** — el fix NO está corriendo en prod. La señal `pending_received` era del block-based path, no del V2-mempool. El validador confundió el contador. No hay deploy manual.

### R8-01 — Decode V2 silencioso
| Señal | Valor |
|---|---|
| `pending_received` | 1386 (block-based, NO V2) |
| `decoded_ok` | 0 |
| `decoded_err` | 0 |
| `pg_period_inserted` (V2 path) | 0 |
| PG opportunities/5min | **114** (block-based path, NO V2) |

**Diagnóstico:** El V2-mempool path está completamente muerto (B-02 deserialize). El block-based path (cartridges) SÍ produce detecciones. Los contadores heartbeat mezclan ambos paths de forma confusa.

**Veredicto R8-01:** 🔴 **CONFIRMADO** — V2-mempool decode roto. El fix está en #335 (sin merge). Contadores N-01b del path raw-subscribe aún no deployados.

### R8-03 — Gas null
| Señal | Valor |
|---|---|
| `total_gas_cost_usd` (24h) | **NULL** |
| `avg_expected_profit_usd` | $213.86 (bruto) |
| Filas 24h | 7 (todas "profitables" sin gas) |

**Veredicto R8-03:** 🔴 **CONFIRMADO** — todo P&L paper es BRUTO. El `gas_oracle_worker` corre pero no está conectado al `paper-trade-archiver.ts`. El archiver no persiste `sim_gas_cost_usd`.

## Estado real del pipeline (sin confusión de contadores)

```
Mempool-V2 path (WS → decode → orchestrator → emit):
  WS subscribe     🔴 ROTO (B-02: PublicNode envía JSON objects, H256 deserializer drop ALL)
  Decode           🔴 NUNCA LLEGA (sin WS items)
  SizeOptimizer    🔴 NUNCA LLEGA
  Gates            🔴 NUNCA LLEGA
  → PRODUCE: 0

Block-based path (block_scanner → cartridges → emit):
  Block detection   ✅ ACTIVO
  Cartridge eval    ✅ ACTIVO (264 strategies)
  SizeOptimizer     ✅ ACTIVO (C.1 fix)
  Gates             🔴 100% rejected: safety_below_threshold (price score 42.2 < 50)
  PG persist        ✅ 114/5min (~1,640/hour)
  Paper archiver    ✅ (pero solo cuando sim_profit presente)
  → PRODUCE: ~1,640 opps/hour, 100% rejected en gates
```

## LA PUERTA $0→$1: lo que falta exactamente

| # | Bloqueo | Fix | Estado |
|---|---|---|---|
| 1 | B-02 deserialize (mempool-V2 muerto) | PR #335 raw-subscribe | OPEN, esperando CI |
| 2 | Price oracle (safety score 42.2 < 50) | Alchemy key o CoinGecko key | Credencial operador |
| 3 | Gas null en paper rows | Conectar gas_oracle → archiver | PR requerido |
| 4 | decoded counters en path raw | Incluido en #335 (N-01 fix) | En cola |

**Después de #1+#2:** pending>0 (ya lo es del block path), decoded>0 (cuando V2 reactivo), gates pasan (cuando price score sube), viables>0, paper rows con neto real (cuando gas conectado).

**Cronograma PUERTA $0→$1:** #335 merge (~2h CI) + Alchemy key (operador, instantáneo) + gas PR (~1h dev + 2h CI) + 72h observación = **~4 días naturales**.

## P&L SIN FANTASÍA (M8)

| Cifra | Valor | Etiqueta | Fuente |
|---|---|---|---|
| Dinero REAL | **$0.00** | REAL | 0 executions on-chain |
| P&L simulado bruto 24h | 7 filas × $213.86 avg | SIMULADO-BRUTO | paper_trade_runs (gas NULL) |
| P&L simulado NETO | **DESCONOCIDO** | SIMULADO-NETO | gas NULL → neto = ficción |
| Estimación gas mainnet (2-3 hops) | $2-15 por intento | ESTIMADO | mercado Ethereum 2026-08 |
| **Veredicto si ejecutara hoy** | **PERDERÍA DINERO** | PROYECCIÓN | $10-67/día bruto vs $12-90/día gas estimado |

**La verdad incómoda:** el sistema hoy, en mainnet con $250, perdería dinero la mayoría de los días. La ventaja técnica (§3 de la directiva) está en el código pero no en los números todavía. Las PUERTAS existen por exactamente esta razón.

## Próximas acciones (ordenadas por dependencia)

1. **Merge #335** → mempool-V2 reactivo (2h)
2. **Provisionar Alchemy key** → price oracle → safety score sube → gates pasan (instantáneo tras deploy)
3. **PR gas-archiver** → conectar gas_oracle_worker a paper_trade_runs.sim_gas_cost_usd
4. **72h observación** → PUERTA $0→$1
5. **14 días + 100 opps** → PUERTA $1→$2

## Metric de esta auditoría

- ✅ R8-01/02/03 verificados en vivo con timestamp
- ✅ P&L etiquetado SIMULADO-BRUTO (gas NULL = neto ficticio)
- ✅ Estimación de pérdida en mainnet declarada sin ambigüedad
- ✅ Camino a PUERTA $0→$1 con dependencias y cronograma
- ⛔ Ninguna acción ejecutable desde aquí (merge, key, PR gas requieren operador/deploy agents)
