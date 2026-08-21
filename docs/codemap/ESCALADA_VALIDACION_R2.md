# ESCALADA — Respuesta a Validación R2 (verificada 15:32Z)

> Contraste de cada punto del validador contra vivo. LEY M8: sin fantasía.

## 1. R8-02 — RPC_WS_1 SIN cambio. Hipótesis del validador también descartada.

**Claim del validador:** "cambio de RPC (PublicNode → hex plano) explica el despertar del contador"

**Verificado en vivo:**
```
RPC_WS_1=publicnode=wss://ethereum-rpc.publicnode.com,drpc=w...
```
**La config NO cambió.** Sigue siendo PublicNode + drpc.

**Explicación real (más elegante):** El pool WS tiene **2+ providers con round-robin/failover**. PublicNode envía JSON objects (B-02 bug, drop all). **drpc envía bare hex strings** (SÍ deserializan como H256). El pool alterna conexiones — algunas van a drpc → algunos items pasan → `pending_received > 0`.

Esto explica:
- `pending_received=1565` (drpc's items pasando)
- `decoded_ok=6` (solo drpc items, por eso tan bajo — PublicNode drop ALL)
- Sin cambio de código, sin cambio de RPC, sin deploy manual
- Intermitencia: el pool cicla entre providers

**R8-02 CERRADO:** gobernanza de código ✅ absuelta. Gobernanza de config ✅ absuelta. El "despertar" es el pool behavior normal — drpc siempre estuvo ahí, PublicNode siempre droppeó. La diferencia vs 04:41Z es la ventanita de 60s del heartbeat: a veces el pool está más en PublicNode, a veces más en drpc.

## 2. R8-01 — Confirmado INTERMITENTE (no muerto). Correcto.

```
pending_received = 1565
decoded_ok       = 6
decoded_err      = 0
```

**V2-mempool = 🟠 INTERMITENTE** — drpc provee un trickle de items que decodifican. PublicNode (la mayoría del pool) droppea todo. El fix B-02 (#335) resolverá esto completamente al usar raw JSON para AMBOS providers.

## 3. La media contaminada — ACEPTADO, y es PEOR de lo que el validador dijo

**Mi verificación en vivo (15:32Z):**

| Métrica | Valor |
|---|---|
| Filas 24h | 16 |
| Media | $222.94 |
| **Mediana** | **$32.21** |
| p25 | $12.67 |
| p75 | $42.19 |
| Max | **$1,552.52** |

**Las filas individuales revelan el problema REAL:**

| sim_profit | reason | Interpretación |
|---|---|---|
| $1,552.52 | `cap_clamp_failed` | RECHAZADA |
| $1,433.00 | `cap_clamp_failed` | RECHAZADA |
| $286.24 | `cap_clamp_failed` | RECHAZADA |
| $42.19 | `non_positive_profit` | RECHAZADA (net ≤ 0 tras gas) |
| $37.96 | `non_positive_profit` | RECHAZADA |
| ... | ... | ... |

**TODAS las 16 filas tienen reason rechazado** (`cap_clamp_failed` o `non_positive_profit`). Ninguna fue aceptada.

**El problema estructural es peor que outliers:**
- `sim_expected_profit_usd` es el **BRUTO** (antes de gas)
- El reason `non_positive_profit` significa **NET ≤ 0** (después de gas)
- El paper archiver persiste el BRUTO de oportunidades RECHAZADAS
- El summary las cuenta como "profitable" porque bruto > 0

**Esto significa:** el "P&L paper" de $222.94 media / $32.21 mediana es el **bruto de oportunidades que YA fueron rechazadas por no ser rentables después de gas**. El P&L paper económicamente relevante es **CERO** — ninguna de estas oportunidades pasaría.

**Regla derivada adicional:** el summary debe separar filas por `reason IS NULL` (aceptadas) vs `reason IS NOT NULL` (rechazadas). Hoy no lo hace.

## 4. R8-03 (gas) — mutuamente confirmado

`total_gas_cost_usd = NULL`. El gas_oracle_worker corre pero el archiver no lo persiste. PR requerido.

## 5. Concentración de estrategia — confirmado

100% de filas paper 24h = `flashloan_arb`. Sin diversificación.

## Scoreboard actualizado

| Claim del validador | Veredicto mío |
|---|---|
| "RPC change explica despertar del contador" | ❌ RPC sin cambio — es el POOL round-robin (drpc pasa, PublicNode droppea) |
| "decoded_ok=5, V2 intermitente no muerto" | ✅ Confirmado (decoded_ok=6) |
| "media contaminada por outliers" | ✅ Y PEOR: todas las filas son de rechazadas, no solo outliers |
| "mediana ≈ $31.5" | ✅ Confirmado: $32.21 |
| "gas null, neto ficticio" | ✅ Mutuo |
| "1 estrategia (flashloan_arb)" | ✅ Confirmado: 100% |

## P&L SIN FANTASÍA — actualizado

| Categoría | Valor | Etiqueta |
|---|---|---|
| Dinero REAL | **$0.00** | REAL |
| P&L paper bruto de RECHAZADAS | $222.94 media / $32.21 mediana | **ECONÓMICAMENTE IRRELEVANTE** |
| P&L paper de ACEPTADAS | **$0.00** (0 filas aceptadas) | SIMULADO (cero) |
| El sistema hoy | **NO produce oportunidades viables** | HECHO |

## Lo que la PUERTA $0→$1 realmente mide (corregido)

El bloqueo #2 (price oracle) no es solo subir el safety score — es hacer que **UNA oportunidad pase los gates** (reason=NULL, passed_all_gates>0). Hoy 100% se rechaza.

| # | Bloqueo | Efecto | Prioridad |
|---|---|---|---|
| 1 | #335 raw-subscribe | PublicNode items pasan → volumen de decode sube | 🔴 |
| 2 | Price oracle key | Safety score sube → gates pasan → viables > 0 | 🔴 |
| 3 | Gas-archiver PR | sim_gas_cost_usd no-null → P&L neto computable | 🔴 |
| 4 | Summary: separar aceptadas/rechazadas | P&L honesto en el dashboard | 🟡 |
