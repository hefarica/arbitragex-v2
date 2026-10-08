# POOL-RESOLVE-01 — la pata de resolución que faltaba

**Orden:** t84 · **Perfil:** Data · **Intento:** `b5bdc7e2-fad7-41b3-bf3c-1c25646e7c51`
**Rama:** `feat/pool-resolve-01` · **NO MERGEADO** (el aterrizaje es `t86` de Release).

## IDENTIDAD ANTES DEL PRIMER COMMIT (clon aislado)

```
clon          = C:/Users/HFRC/Desktop/arbx-t49/repo      (aislado; el compartido no se tocó)
rama_actual   = tmp/t78-probe
origin/main   = b3b734582c1e1f271b3b8439430cdada5e762449
HEAD          = b3b734582c1e1f271b3b8439430cdada5e762449
arbol         = ef575a7e04a84c2c00ff8f0d9b00575badf0c616
worktree      = limpio (git status --porcelain vacío)
rama nueva    = feat/pool-resolve-01, creada desde esa base
```

---

## 1. ANTES — por comando, con CONTROL pasado

```
$ psql -tAc "SELECT 1"                                                       -> 1     (CONTROL)
$ psql -tAc "SELECT count(*) FILTER (WHERE is_active) AS activos, count(*) AS total FROM pools"
1228|4005
$ psql -tAc "SELECT count(*) AS pares, count(*) FILTER (WHERE n>=2) AS con_2_o_mas
             FROM (SELECT (token0_id,token1_id) p, count(*) n FROM pools WHERE is_active GROUP BY 1) t"
829|192
$ psql -tAc "SELECT count(*) AS total, count(*) FILTER (WHERE resolved_pool_addr IS NOT NULL) AS resueltos,
                    count(*) FILTER (WHERE is_resolved) AS marcados FROM observed_unindexed_pairs"
7515|800|800
$ psql -tAc "SET max_parallel_workers_per_gather=0; SELECT rejection_reason, count(*) FROM opportunities
             WHERE detected_at > now() - interval '1 hour'
               AND rejection_reason IN ('v3_pair_no_pools','v3_pool_not_catalogued','single_pool_no_spread')
             GROUP BY 1 ORDER BY 2 DESC"
v3_pool_not_catalogued|16856
v3_pair_no_pools|11235
single_pool_no_spread|35
$ docker exec arbitragex-v2-redis-1 redis-cli XINFO STREAM arbx:opps:validated | grep -A1 entries-added
entries-added  12247832
$ psql -tAc "SELECT count(*) FROM simulations"                                -> 0
```

**`observed_unindexed_pairs`: 7.515 filas y CERO LECTORES.** Barrido de todo el árbol:
```
$ git grep -rn "observed_unindexed_pairs" | grep -v '^audits/\|GEMINI.md\|schema-drift'
backend/searcher-rs/src/pool_discovery.rs  (writer: INSERT … ON CONFLICT … observation_count + 1)
```
El único código que la menciona es el **escritor**. No había lector. **(t78 midió 7.494 filas; hoy 7.515 — la tabla crece.)**

---

## 2. EL DEFECTO REAL (corrige la premisa de la orden, y es más chico de lo que decía)

La orden afirma que falta la pata de resolución completa. **Medido en fuente, la resolución POR PAR ya existía**: `discover_from_intent` ya barre las factories sembradas con `IUniswapV2Factory.getPairCall` (`pool_discovery.rs:229`) y `IUniswapV3Factory.getPoolCall` (`:274`) sobre los cuatro tiers canónicos. Lo que estaba roto es **qué se hacía con la respuesta**:

```rust
// ANTES (pool_discovery.rs, discover_from_intent)
let mut resolved_pool = None;
for (f_id, pool_addr, proto, dex_name, fee_raw) in discovered_pools {
    let e_pool = Address::from_slice(pool_addr.as_slice());   // <- la factory YA respondió
    ...
    if let Ok(pool_ref) = self.hydrate_and_persist_pool(...).await {
        resolved_pool = Some(e_pool);        // sólo si la hidratación tuvo éxito
        ...
    } else {
        warn!("pool_discovery.hydration_failed");   // la dirección se DESCARTA
    }
}
self.record_observation(..., resolved_pool).await;   // -> NULL si la hidratación falló
```

**Dos defectos encadenados:**
1. **La dirección se descarta** cuando la hidratación falla, aunque la vista de fábrica ya la haya respondido.
2. **El upsert borra lo ya sabido**: `is_resolved = $6, resolved_pool_addr = $7` sin condición ⇒ una observación posterior fallida **borra** una dirección ya resuelta.

Eso explica el 89,7% con `resolved_pool_addr IS NULL`: **no es que nadie resolviera, es que la respuesta se tiraba.**

---

## 3. EL DIFF — las dos piezas que la orden pide

```
 backend/searcher-rs/src/pool_discovery.rs          | +221 -4
 backend/searcher-rs/src/workers/pool_enumeration_worker.rs | +122
 2 files changed, 339 insertions(+), 4 deletions(-)
```

### 3.1 PATA DE RESOLUCIÓN — `resolve_pair_pools` (pool_discovery.rs:1200)

Método **público y llamable** con las MISMAS dos vistas de fábrica que el camino vivo (`getPair` V2 fee 30 · `getPool` V3 sobre 100/500/3000/10000), contra las factories **sembradas** (`get_factories`). Devuelve cada acierto con su fee en **basis points**, la convención que `enumerate_and_persist_pool` espera. Fail-honest: una factory no sembrada no se consulta; un cero es "esa factory no tiene ese pool"; un error de RPC es un miss. **Nunca fabrica un pool.**

### 3.2 PERSISTIR `resolved_pool_addr` + `is_resolved`, y que no se borre

```rust
// AHORA
let mut resolved_addr: Option<Address> = None;
for (f_id, pool_addr, proto, dex_name, fee_raw) in discovered_pools {
    let e_pool = Address::from_slice(pool_addr.as_slice());
    if resolved_addr.is_none() { resolved_addr = Some(e_pool); }   // <- apenas la factory responde
    ...
}
self.record_observation(..., resolved_addr).await;   // <- se persiste hidrate o no
```
y el upsert pasa a ser **monótono en la resolución**:
```sql
DO UPDATE SET
    observation_count = observed_unindexed_pairs.observation_count + 1,
    last_seen_at = NOW(),
    is_resolved = observed_unindexed_pairs.is_resolved OR EXCLUDED.is_resolved,
    resolved_pool_addr = COALESCE(observed_unindexed_pairs.resolved_pool_addr, EXCLUDED.resolved_pool_addr)
```
**La resolución sólo avanza. Una dirección conocida no se anula nunca.** La escritura del sweep reusa la misma monotonía (`COALESCE`), y el hex lo produce PostgreSQL (`'0x' || encode($2,'hex')`), así que el string almacenado no depende de ninguna convención de formato de Rust.

### 3.3 CONSUMIDOR — `pending_observed_pairs` + `sweep_observed_pairs`

**La consulta** (`pool_discovery.rs:1307`) — el lector que no existía: pares observados de la cadena para los que **no existe ningún pool ACTIVO** (que es exactamente lo que lee `impact_index.rs:602` para armar el universo evaluable). Ordena por `observation_count DESC` (máxima señal primero) y es **auto-drenante**: en cuanto el par tiene un pool activo, la fila deja de matchear ⇒ el barrido converge en vez de reprocesar para siempre.

**El llamador** (`pool_enumeration_worker.rs:412`) — `sweep_observed_pairs()`, invocado desde `run()` **después de cada `run_tick()`, con la MISMA periodicidad**: `self.cfg.interval` (`POOL_ENUM_INTERVAL_MS`, default **1 h**) y el MISMO presupuesto (`max_new`, default 50/tick). Sigue colgado del mismo gate de arranque ya existente: el worker sólo se spawnea si `ARBX_POOL_ENUM_MODE == "shadow"` (`scanner.rs:614-622`), así que **no se introdujo ningún flag nuevo ni se activó nada por sorpresa**.

Por fila: resolver si falta la dirección → **gate de seguridad** → `enumerate_and_persist_pool(..., activate)` (la función que ya existía, `pool_discovery.rs:1063`).
Contadores al `info!` agregado (`poolenum.observed_sweep`): `attempted, resolved_now, activated, persisted_inactive, safety_blocked, unresolvable, failed` (R9: un resumen por tick, no una línea por ítem).

### 3.4 EL SQL NUEVO, VERIFICADO CONTRA LA BASE VIVA (sólo lectura)

Las dos sentencias que agregué se probaron contra el esquema real antes de commitear:

```
$ psql "SELECT count(*) FROM observed_unindexed_pairs o WHERE o.chain_id = 1 AND NOT EXISTS (…)"   # verbatim del consumidor
6203
$ psql "SELECT count(*) FILTER (WHERE o.resolved_pool_addr IS NOT NULL) AS con_direccion,
               count(*) FILTER (WHERE o.resolved_pool_addr IS NULL) AS sin_direccion FROM …"
30|6173|6203
$ psql "EXPLAIN UPDATE observed_unindexed_pairs SET resolved_pool_addr = COALESCE(resolved_pool_addr,'0x'||encode('\\x00'::bytea,'hex')), is_resolved = TRUE WHERE id = '…'::uuid"
Update on observed_unindexed_pairs  (cost=0.28..8.30 rows=0 width=0)
  ->  Index Scan using observed_unindexed_pairs_pkey on observed_unindexed_pairs
```

**Backlog real que el barrido tiene por delante: 6.203 pares**, de los que **6.173 (99,5%) no tienen dirección** (necesitan la pata de resolución) y **30 ya la tienen** (sólo hay que reintentar la hidratación). El `SELECT` se ejecutó (sólo lectura); el `UPDATE` **no se ejecutó**: se verificó con `EXPLAIN`, que planifica sin escribir. Las filas quedaron intactas.

---

## 4. EL GATE DE SEGURIDAD DE TOKENS: **SE LLAMA, NO SE EVITA**

El barrido usa **el mismo método y el mismo piso** que el tick de enumeración:

```rust
let s0 = self.token_safety(&t0s).await;      // pool_enumeration_worker.rs:493-511
let s1 = self.token_safety(&t1s).await;
let activate = match (s0, s1) {
    (SafetyVerdict::Unsafe, _) | (_, SafetyVerdict::Unsafe) => { safety_blocked += 1; continue; }
    (SafetyVerdict::Safe, SafetyVerdict::Safe) => true,
    _ => false,   // Unrated -> se persiste INACTIVO para un tick futuro
};
```
- `token_safety` es literalmente el mismo método (`SELECT safety_score … WHERE … ttl_expires_at > NOW()`), y el piso sigue siendo `TOKEN_SAFETY_FLOOR` / `cfg.safety_floor` (`:50`, `:87`). **`TOKEN_SAFETY_FLOOR` intacto.**
- `Unsafe` ⇒ **se saltea el par entero** (no se indexa ni se persiste): indexar un par tóxico para fabricar flujo sería abrir la puerta a perder capital.
- **Cero filas escritas en `token_safety_cache`.** No se fabricó ningún veredicto. El barrido *consume* el veredicto, no lo produce.

## 5. LO QUE NO SE HIZO, con esas palabras

- **NO se toca `is_active` por SQL a mano.** Ningún `UPDATE pools` en esta tarea.
- **NO se baja el umbral de spread ni de net-profit.** El 74,60% restante ya está evaluado y son pérdidas y equilibrios genuinos; un parche ahí no crea flujo, sólo mueve el umbral.
- **NO se mergea.** El aterrizaje es `t86`. Ningún deploy, ninguna firma, ningún broadcast, ningún capital.

---

## 6. DESPUÉS — lo que se puede y lo que NO se puede medir hoy

**La pareja antes/después DE MI CÓDIGO es NO COMPUTADA, y lo digo con esas palabras: el código NO está desplegado** (no hay merge, y el deploy es de `t86`). No existe todavía una ventana donde mi cambio haya corrido. Cualquier "después" que presentara sería una ventana del sistema **sin** mi cambio.

**Lo que sí medí, en la misma ventana y sobre el sistema vivo** (para separar mi efecto del movimiento propio del pipeline) — y **NO es atribuible a mi cambio**:

| Magnitud | t78/t79 (referencia) | t84 muestra 1 | t84 muestra 2 (+minutos) |
|---|---|---|---|
| pools activos / total | 1.197 / 3.935 | **1.228 / 4.005** | **1.229 / 4.006** |
| pares del universo / con ≥2 | 804 / 187 | **829 / 192** | **830 / 192** |
| `observed_unindexed_pairs` resueltos | 775 | **800** | **801** |
| rechazos estructurales (1 h) | — | 16.856 + 11.235 + 35 | 16.853 + 11.222 + 37 |
| `validated` `entries-added` | 12.247.832 | **12.247.832** | **12.247.832** |
| `simulations` | 0 | **0** | **0** |

**Lectura honesta:** el camino `reactive` que YA existía sigue moviendo el universo, **pero a un ritmo de +1 pool activo y +1 resolución por pocos minutos de reloj** (1.228→1.229 y 800→801 entre mis dos muestras), mientras **`validated` sigue congelado en 12.247.832 y `simulations` en 0** — la cadena sigue sin cerrarse. Los rechazos estructurales siguen masivos. **Mi cambio no está en ninguna de esas mediciones.**

**Plan de verificación post-`t86` (mismas queries, misma ventana):**
1. `SELECT count(*) FILTER (WHERE resolved_pool_addr IS NOT NULL) FROM observed_unindexed_pairs` → debe **subir** desde 800 (la pata de resolución persiste lo que antes descartaba).
2. `SELECT count(*) FILTER (WHERE is_active) FROM pools` → debe subir desde 1.228.
3. `SELECT count(*) FROM opportunities WHERE detected_at > now() - interval '1 hour' AND rejection_reason IN ('v3_pair_no_pools','v3_pool_not_catalogued')` → debe **bajar** de la tasa de referencia (16.856 + 11.235/h en la ventana medida).
4. `XINFO STREAM arbx:opps:validated` → `entries-added` debe dejar de estar congelado en 12.247.832.
5. `SELECT count(*) FROM simulations` → si sale de 0, la cadena se probó de punta a punta por primera vez desde 2026-09-17.
6. Log: `docker logs arbitragex-v2-searcher-rs-1 | grep observed_sweep` → `attempted/resolved_now/activated/safety_blocked`.

---

## 7. PRECISIÓN QUE NO SE PUEDE PERDER

**Recuperar el 25,40% devuelve al motor oportunidades EVALUABLES, no rentables.** `t48` midió una ruta real **10,82× corta** contra su hurdle de fees (spread 5,57 bps contra 60,27 bps). **25,40% es techo de flujo evaluable, NO de ganancia**, y este PR no cambia eso: lo único que cambia es que esas oportunidades **llegan a ser juzgadas** en vez de rebotar con *"rejecting WITHOUT RPC (R8)"*.

---

## 8. VERIFICACIÓN DE COMPILACIÓN

Se compila en **WSL2** (toolchain 1.91.0, la que fija `rust-toolchain.toml` del repo; en Windows el build está bloqueado por Smart App Control, `os error 4551`, precedente de t8/t25). El resultado exacto (`cargo check -p searcher-rs`) está en el PR; si la compilación no hubiera pasado, este documento lo diría en lugar de citar un PASS.

---

*Clon aislado; identidad registrada antes del primer commit. Sin merge, sin deploy, sin firma, sin broadcast, sin capital. `ARBX_TRADE_MODE=paper`. El gate de seguridad de tokens se llamó, no se evitó. `P/N` no se mueve con este PR: sólo se mueve si el código aterriza y la evidencia se liga al criterio.*
