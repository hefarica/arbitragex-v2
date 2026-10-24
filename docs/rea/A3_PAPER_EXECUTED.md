# A3 — `paper_executed`: estado paper SEPARADO de `executed`

Fecha: **2026-10-23** · Autor: verificador (t22) · Repo: `hefarica/arbitragex-v2` · Base: `origin/main` @ `697f1b5f`.

**AUTORIZACIÓN DEL OPERADOR (literal, sin reinterpretar):**
> `CONFIRMED: A3 status paper_executed separado de executed (paper, sin live)`

---

## 0. El problema, medido (no supuesto)

`status = 'executed'` es **inalcanzable en paper por diseño**: su **único** escritor es el relayer y sólo tras inclusión on-chain
(`backend/relays-client/src/persistence.rs:47-59`: `ExecutionStatus::Included => "executed"`, con `WHERE id = $1 AND status IN ('simulated','executing')`;
la construcción de `Included` vive en `backend/relays-client/src/submit_engine.rs:854-883`, `wait_for_inclusion` post-broadcast).
Producción corre en paper, así que **no hay difusión ⇒ no hay inclusión ⇒ no hay `executed`** (medido en la PG del VPS: `executed = 0` sobre 515,989 filas de
`opportunities`, `executions` con 0 filas, `paper_trade_runs` con 0 filas).

El camino paper **sí decide y sí persiste** algo equivalente — simulación `passed` + neto positivo + fila de evidencia — pero se guarda en
`paper_trade_runs` y **no** en `opportunities.status`. Consecuencia: la puerta de fase (`status='executed'`) y la realidad paper hablan idiomas distintos.
`paper_executed` es el estado honesto que cierra esa brecha **sin** confundirse con ejecución on-chain.

---

## 1. Dónde vive el vocabulario de `status` (citado, ruta:línea)

| Artefacto | Ubicación | Qué dice |
|---|---|---|
| **CHECK canónico** | `database/migrations/003_opportunities.sql:20-23` | `status TEXT NOT NULL DEFAULT 'detected' CHECK (status IN ('detected','validated','simulated','scored','executing','executed','reconciled','rejected','failed'))` |
| Documentación del enum (Rust) | `backend/searcher-rs/src/persistence.rs:15-18` | copia literal del vocabulario del CHECK (comentario) — **actualizado en esta unidad** para incluir `paper_executed` |
| Escritor de `executed` (LIVE) | `backend/relays-client/src/persistence.rs:47-59` | `'executed'` sólo tras inclusión; guarda `status IN ('simulated','executing')` |
| Escritor de `reconciled` | `backend/recon/src/persistence.rs:49-51` | `WHERE id=$1 AND status IN ('executed','simulated','executing')` — **no** incluimos `paper_executed`: un paper trade no se reconcilia contra la cadena |
| Escritor de `validated`/`rejected` | `backend/selector-api/src/persistence.ts:27-36` | guarda `status IN ('detected','validated','scored')` — **no puede pisar** `paper_executed` |
| Escritor del camino paper | `backend/api-server/src/paper/executor.ts` (persistía sólo `paper_trade_runs`) | **destino de esta unidad** |
| ⚠️ Tabla DISTINTA | `database/migrations/005_executions.sql` | `executions_status_check` = `('submitted','included','reverted','dropped','replaced','not_implemented')` — **NO admite `executed`** y **no se toca**: `executions` es de ejecución real |

No se editó ninguna migración histórica (regla del repo: *forward-only + idempotente*).

---

## 2. Migración (aditiva, idempotente, lock-safe)

**`database/migrations/127_opportunities_paper_executed_status.sql`** — sólo **amplía** el vocabulario del CHECK; no reescribe filas.

- **Aditiva**: los 9 valores previos se conservan; se añade `paper_executed`.
- **Idempotente y lock-safe**: el runner (`database/run_migrations.sh`) **no tiene ledger** y re-ejecuta TODOS los archivos en cada deploy, y
  `opportunities` es tabla **HOT** (INSERTs del searcher + DELETEs de purge) con `lock_timeout=10s` (FREEZE-01). Por eso el DDL vive dentro de un
  `DO $$` cuyo `IF` lee **sólo catálogos** (`pg_constraint`) y sólo ejecuta `ALTER TABLE` cuando el CHECK genuinamente no contiene `paper_executed`
  ⇒ el re-run no-op **no pide ningún lock de tabla** (patrón exigido por `automation/tools/lint-migration-rerun-lock-safety.sh`).
- Si la DB llegara **sin** el CHECK, lo crea con el vocabulario completo (fail-safe), y lo deja documentado con `COMMENT ON COLUMN`.

---

## 3. Criterio EXACTO (definido sobre el pipeline existente, no inventado)

Una fila pasa a `paper_executed` **si y sólo si** concurren las tres condiciones, todas ya computadas hoy:

| # | Condición | Dónde se decide / persiste (citado) |
|---|---|---|
| (i) | la simulación declara `status === "passed"` | `backend/sim-ctl/src/sim_engine.rs` fija `passed = slippage <= umbral` (con decimales REALES desde F2/#1033) y `backend/sim-ctl/src/persistence.rs:96` persiste la fila `simulations` |
| (ii) | `net_yield_wei > 0` (neto tras gas + decoherencia) | `backend/api-server/src/paper/executor.ts`: `calculateNetTopologicalYield(...)` → `isAccepted = net_yield_wei > BigInt(0)` |
| (iii) | existe la fila de **evidencia** `paper_trade_runs` | `backend/api-server/src/paper/executor.ts`: `persistRun(...)` → `INSERT INTO paper_trade_runs ... RETURNING id` |

**Evidencia asociada al estado**: `paper_trade_runs.id` (= `paper_run_id`) + `opportunities.id` + `net_yield_wei`, publicados en el log
`paper_executor.paper_executed`. El estado nunca se escribe “a mano”: se escribe **después** de que la evidencia exista.

---

## 4. Escritor (paper, sin live)

En `backend/api-server/src/paper/executor.ts`, dentro de `if (isAccepted)`, **después** de `persistRun`:

```
UPDATE opportunities
   SET status = 'paper_executed', updated_at = NOW()
 WHERE id = $1
   AND status IN ('detected','validated','simulated','scored')
```

- La **guarda** `status IN (...)` impide pisar estados **terminales** (`rejected`, `failed`, `executed`, `reconciled`): un `executed` on-chain
  **jamás** se degrada a paper (probado en la integración, caso d).
- Si el UPDATE falla, se loguea `paper_executor.paper_executed_status_err` y **no** se rompe el loop: la fila de `paper_trade_runs` ya es la
  evidencia y **no se fabrica** un estado que no se pudo escribir (R8).
- `executed` **no se toca en ningún punto**; el camino live (`relays-client`) queda intacto, igual que `configs/app.toml`, `live_exec_policy.rs` y
  el broadcast path de `bundle_builder.rs`.

---

## 5. Tests

| Tipo | Archivo | Qué demuestra |
|---|---|---|
| **Unit** (vitest, lane unit) | `backend/api-server/src/paper/executor.test.ts` | (1) con `passed`+neto>0 persiste evidencia y mueve el estado a `paper_executed`; (2) con neto≤0 no escribe nada; (3) con `failed` no toca la DB; (4) el UPDATE está guardado; (5) si el UPDATE falla, la evidencia queda y el loop no rompe; (6) **no-broadcast estructural**: la fuente del ejecutor no importa ni invoca `ethers`/`viem`/`web3`/`sendTransaction`/`eth_sendRawTransaction`/`privateKey`, y el único estado que escribe es `paper_executed` |
| **Integración** (PG real, lane `test:integration`) | `backend/api-server/test/paper-executed-paper-mode.test.ts` | (a) el CHECK **acepta** `paper_executed` (migración aplicada) y conserva el vocabulario previo; (b) el escritor mueve la fila a `paper_executed` y **`executed` sigue en 0**; (c) la migración es **idempotente** (se ejecuta 2 veces más y no rompe ni duplica el CHECK); (d) **no degrada** una fila `executed` |

El lane de integración de CI (`integration-tests.yml`) aplica `automation/scripts/migrate.sh` sobre PG16 real **antes** de los tests ⇒ la migración 127
se valida ahí, y `npm run test:integration -w @arbx/api-server` corre el archivo de `test/`.

---

## 6. GATE (post-deploy)

```sql
-- 1) la migración está aplicada y el estado es aceptado
SELECT pg_get_constraintdef(oid) FROM pg_constraint
 WHERE conrelid = 'opportunities'::regclass AND conname = 'opportunities_status_check';
-- -> debe contener 'paper_executed'

-- 2) >=1 paper_executed y 0 executed (paper_mode intacto)
SELECT status, count(*) FROM opportunities GROUP BY 1 ORDER BY 2 DESC;
-- -> paper_executed >= 1 ; executed = 0

-- 3) cada paper_executed tiene su evidencia
SELECT o.id, o.updated_at, r.id AS paper_run_id, r.actual_profit_usd, r.reason
  FROM opportunities o LEFT JOIN paper_trade_runs r ON r.opportunity_id = o.id
 WHERE o.status = 'paper_executed'
 ORDER BY o.updated_at DESC LIMIT 10;
-- -> paper_run_id no nulo y actual_profit_usd > 0 en todas
```

**Estado de este gate: NO EJECUTADO** — requiere merge + deploy (y que el camino paper produzca una sim `passed` con neto>0; con F1/F2 recién
mergeados, eso es exactamente lo que queda por medir). No se declara cumplido.

---

## 7. Fail-honest (lo que esta unidad NO demuestra)

1. **El gate post-deploy no se ejecutó**: sin merge+deploy no hay filas `paper_executed` en producción. Las queries quedan listas arriba.
2. **`paper_executed` no es rentabilidad**: dice “el camino paper pasó simulación y el neto calculado fue > 0”, ni más ni menos. No hay gas real,
   ni inclusión, ni PnL realizado; por eso el estado **no** es `executed` ni se reconcilia (`recon` no lo incluye).
3. **No se tocó el camino live** ni se habilitó difusión: el ejecutor paper no tiene cliente de cadena (test estructural lo fija).
4. **Una fila puede quedar sin `paper_executed`** aunque la simulación pase: si el UPDATE del estado falla, la evidencia queda en `paper_trade_runs`
   y el evento de error lo declara; no se reintenta a ciegas (evita duplicar runs).
5. **El vocabulario del CHECK queda en la DB**: si otra sesión añade estados en paralelo, hay que re-mirar el `IF` de la migración (compara contra
   `paper_executed`, no contra una lista completa).
