# CAUSA RAÍZ — POR QUÉ EL ARSENAL DE CARTUCHOS NO EMITE

**Fecha:** 2026-09-30 · **Rama base:** `main` @ `6d29137d` · **Autor:** sesión de auditoría E2E
**Alcance:** diagnóstico read-only sobre VPS productivo + verificación de código en `origin/main`.
**Estado:** hallazgo verificado con mediciones reproducibles. Sin cambios de comportamiento en este documento.

> **Doctrina aplicada:** R8 (fail-honest), R9 (ventana de logs), R10 (E2E-compute guard), R11 (verificación en árbol ajeno), RULE 00 (zero-mocks). Ninguna afirmación de este informe proviene de inferencia sin medición; cada una cita el artefacto que la respalda.

---

## 1. Resumen ejecutivo

El sistema **no tiene un problema de admisión de manifiestos**. Tiene un problema de **cobertura de grafo por intent**: cuando un intent no produce aristas, el runtime cae a un contexto *DATA-GAP* cuyo mapa de digests está vacío, y entonces **los 79 cartuchos de ese intent son rechazados con la razón `manifest_not_admitted_by_backend`** — una razón que nombra al cartucho cuando el productor que falta es el grafo.

Este único mecanismo explica **~2 M de filas/día** de diagnóstico engañoso y oculta la causa real. Además, el índice V3 se publica con **unidades no uniformes**: pips crudos en un campo llamado `fee_bps` (§4). El impacto exacto de esa inconsistencia **no está determinado** en este informe: requiere auditar los consumidores antes de concluir (ver P2).

---

## 2. Medición base (VPS productivo, ventana de 2 h)

Tabla `route_discovery_outcomes_p20260930`, `inserted_at > now() - interval '2 hours'`:

| `reason` | filas | cartuchos distintos | con `had_reserves` |
|---|---:|---:|---:|
| `search_completed_within_declared_scope` | 1.025.208 | **63** | 744.417 (73 %) |
| `native_domain_solver_required` | 260.496 | **16** | — |
| `manifest_not_admitted_by_backend` | **165.110** | **79** | **0 (0 %)** |
| `insufficient_price_history` | 18.371 | 1 | — |
| `no_funding_rates_data` | 18.371 | 1 | — |
| `not_confirmed_zero_victim_guard` | 18.371 | 1 | — |
| `policy_rejected` | 495 | 55 | — |
| `token_meta_unavailable` | 57 | 1 | — |
| `v3_slot0_unavailable` | 13 | 1 | — |

**Lectura decisiva:** `had_reserves = false` en el **100 %** de las filas de `manifest_not_admitted_by_backend`, frente al 73 % de verdaderas en las filas que sí completan. El discriminador no es el manifiesto: es la presencia de datos de grafo.

> **Nota de método (R9).** Las cifras provienen de consultas sucesivas sobre una tabla viva, por lo que difieren en unos cientos de filas entre sí (p. ej. `manifest_not_admitted_by_backend` = 165.110 y 164.952; `search_completed…` = 1.025.208 y 1.024.326). La proporción es lo que se interpreta, no el dígito exacto. La tabla está particionada por día (`_p20260930`); la partición de ayer (`_p20260929`) tenía 18,5 M de filas.

> **Corrección de un supuesto previo.** Una hipótesis intermedia de esta misma sesión fue "todos los cartuchos fallan siempre". Es **falsa** y la propia tabla la refuta: 63 cartuchos completan discovery y 16 llegan al solver. El fallo es **por intent**, no global.

### 2.1 Los cuatro pools que concentran el 95,7 %

| pool | par | fee PG | `enum_source` | dirección |
|---|---|---|---:|---|---:|
| `0x464bd7e6…76b3` | WETH/XPR | 100 | `reactive` | 89.546 |
| `0x2f62f2b4…a9ec` | SHIB/WETH | 3000 | `seed` | 44.956 |
| `0x11950d14…7b58` | PEPE/WETH | 3000 | `seed` | 20.224 |
| `0x9db9e0e5…425b` | WBTC/USDT | 3000 | `seed` | 3.081 |
| | | | **total** | **157.807 / 164.952 = 95,7 %** |

---

## 3. Mecanismo exacto, verificado en código

### 3.1 El gate no falla por el manifiesto

`backend/searcher-rs/src/snapshot_services.rs:186-190`:

```rust
let id = required(spec, "mev_id")?;
let digest = required(spec, "source_digest")?;
if self.data.manifest_digests.get(id).map(String::as_str) != Some(digest) {
    return Err("manifest_not_admitted_by_backend".into());
}
```

`manifest_digests` es un `BTreeMap<String,String>` del bundle. **Si el mapa está vacío, este gate — que se evalúa primero — rechaza todo.**

Réplica exacta del algoritmo de admisión (`v4_extract_quoted_values` + `v4_admit_script` + `v4_scan_manifest_digests`) sobre los 264 `.rhai` de `backend/searcher-rs/cartridges/strategies/` de `origin/main`:

```
dir strategies presente : OK
archivos escaneados     : 264
ADMITIDOS (mapa backend): 264
skipped (v4_admit None) : 0
conflictos de mev_id    : 0
```

**Los 264 manifiestos son admitidos.** Y los 264 archivos **desplegados** en el contenedor son **byte-idénticos** a `origin/main` (264/264 `sha256sum` coincidentes, 0 diferencias). El fallo no está en los manifiestos.

### 3.2 El bundle productivo es un placeholder DATA-GAP

`backend/searcher-rs/src/cartridge_boot.rs:166-215` construye el bundle que se registra en el arranque:

```rust
// Registers the agent_v4_* bindings backed by a SnapshotServices built
// from a DATA_GAP bundle: edges/prices/quotes are EMPTY (nothing
// fabricated — R8) ... Real producers wire into this bundle in later
// phases (issue #647 family).
...
    manifest_digests: Default::default(),   // ← MAPA VACÍO
```

`spawn_cartridge_runtime` lo usa en `:220` (`SnapshotServices::new`) y lo propaga al runner en `:275`.

### 3.3 La ruta real existe pero está tras cuatro guardas, y su fallback es mudo

`cartridge_boot.rs:1749-1810` sí intenta construir el bundle real por intent:

```rust
let mut v4_registered = false;
if v4_edges.is_empty() {
    debug!(event = "cartridge.v4_intent_no_edges", ...);      // ← DEBUG, invisible a INFO
} else if let (Some(cfg), Some(identity), Some(start_token), Some(router)) = ( ... ) {
    match build_v4_intent_bundle(...) {
        Some(bundle) => { /* registra contexto real, con manifest_digests REALES */ }
        None => { debug!(event = "cartridge.v4_intent_clock_invalid", ...); }
    }
}
```

- Si se registra → `manifest_digests` **completos** → el gate pasa → el cartucho corre su flujo real.
- Si no → contexto DATA-GAP → **mapa vacío** → los 79 cartuchos del intent mueren en el primer gate con `manifest_not_admitted_by_backend`.
- El `else if let` **no tiene rama `else`**: si una de las cuatro guardas es `None`, no se emite **ningún** log. El fallback es indistinguible de "todo bien".

**Esto es una violación de R8/R10 en el eje productor→consumidor:** el veredicto en pantalla nombra al cartucho como no admitido cuando el productor ausente es el grafo del intent.

### 3.4 El escritor de slot0 y el índice V3 sí funcionan

Los cuatro pools dominantes **tienen** `arbx:v3_slot0:1:<pool>` presente (`EXISTS = 1`), y el índice `arbx:pool_index_v3` tiene 423 claves / 466 pools distintos. Es decir: **el productor de datos V3 existe y escribe.** El problema no es su ausencia sino lo que se indexa (ver §4).

---

## 4. Error de unidades de 100× en el índice V3

`pool_reserves` es el almacén del modelo **V2** (`getReserves()`). Los pools V3 se representan por `slot0` en Redis, no por `pool_reserves`. Por eso "167 pools sin reservas" **no** significa "sin datos" — significa "son V3".

Composición real de los 339 pools activos:

| `protocol_type` | pools | `pools.fee_tier` | almacén de estado |
|---|---:|---|---|
| `UNISWAP_V2` | 172 | `30` (**bps** — correcto: 0,3 %) | `pool_reserves` |
| `UNISWAP_V3` | 167 | `100/500/3000/10000` (**pips**) | `arbx:v3_slot0` |

La columna `pools.fee_tier` es **ambigua por protocolo**, lo que ya documenta `CATALOG-BACKFILL-01` (`pool_sync_worker.rs:120-148`).

### 4.1 El bug

`backend/searcher-rs/src/workers/pool_sync_worker.rs`, **ambas** rutas escriben pips crudos en un campo llamado `fee_bps`:

```rust
// ruta canónica (:1352-1358)
let tier = fee_tier.unwrap_or_default();      // p. ej. 3000 (PIPS)
by_pair.entry((lo, hi)).or_default().push(V3PoolInfo {
    pool_addr: addr.to_lowercase(),
    fee_bps: tier as u32,                     // ← 3000 "bps" = 30 %
});

// ruta resuelta (:1416-1424)
.push(V3PoolInfo { pool_addr: addr.clone(), fee_bps: tier })   // ← PIPS otra vez
```

y el `UPDATE` de `:1398-1401` escribe pips en `pools.fee_tier`.

### 4.2 Verificación en runtime

Contenido real de `arbx:pool_index_v3` en el VPS:

```
arbx:pool_index_v3:1:weth:xpr
[{"pool_addr":"0x540a6b18…5074","fee_bps":5},      ← V2, 0,05 % — correcto
 {"pool_addr":"0x06f171de…7726","fee_bps":30},     ← V2, 0,30 % — correcto
 {"pool_addr":"0x464bd7e6…76b3","fee_bps":100}]    ← V3, 100 PIPS = 0,01 %  → leído como 1 %

arbx:pool_index_v3:1:shib:weth
[{"pool_addr":"0x2f62f2b4…a9ec","fee_bps":3000}]   ← V3, 3000 PIPS = 0,3 %  → leído como 30 %
```

**El mismo índice mezcla bps (V2) con pips (V3).** Un consumidor que trate `fee_bps` de forma uniforme cotizaría los pools V3 con una fee **100× mayor** que la real (0,3 % → 30 %), lo que haría imposible cualquier ciclo rentable sobre pools V3.

### 4.3 Lo que **no** está determinado (y debe auditarse antes de tocar nada)

Sería incorrecto cerrar §4 como "bug de 100× confirmado". La evidencia muestra que las unidades **no son uniformes y ya existen normalizaciones parciales** en el código, así que el consumidor manda:

- `impact_index.rs:1394-1432` prueba **dos** comportamientos distintos en el mismo índice:
  ```rust
  pool.fee_bps = Some(3000);  assert_eq!(…[0].fee_bps, Some(3000));  // 3000 → 3000
  pool.fee_bps = Some(500);   assert_eq!(…[0].fee_bps, Some(30));    //  500 →   30
  pool.fee_bps = Some(10000); assert_eq!(…[0].fee_bps, Some(10000)); // 10000 → 10000
  ```
  Es decir: existe al menos un sitio que normaliza pips→bps (`500 → 30`) y otros valores que pasan sin tocar.
- `config_aware.rs:71` documenta que, para pools V3, el `fee_bps` por pool **proviene de `V3PoolInfo.fee_bps`**, y `config_aware.rs:829` que `default_fee_bps_for_adapter` devuelve 0 para V3 precisamente porque el fee lo aporta el pool.
- `cartridge_boot.rs:1320-1331` trata explícitamente los dos denominadores: V2 en bps/10 000 y V3 en **pips/1 000 000**, y `cartridge_boot.rs:3668` lo documenta ("fee_bps de V3 son PIPS con denominator 1_000_000").

**Conclusión honesta:** el *writer* (`pool_sync_worker.rs`) publica pips bajo un nombre de bps — eso está verificado. Si eso se convierte en un error económico depende de qué haga cada consumidor, y hay al menos una normalización parcial en juego. **La auditoría de consumidores es obligatoria y es el contenido del PR P2; no se afirma aquí un impacto de 100× en la decisión de emisión.**

---

## 5. Consecuencia sobre el mandato del operador

El mandato es *"dejarlas todas funcionales, con el mayor % del arsenal corriendo"*. El estado medido:

- **63 cartuchos** completan discovery (23 % de los 271 del registro).
- **16** llegan al solver de dominio nativo.
- **79** quedan etiquetados como no admitidos por una causa falsa.
- El techo real no lo fija el número de cartuchos sino **el grafo por intent**: sin aristas no hay oportunidad que evaluar, y el fallback enmascara el motivo.

---

## 6. Plan de corrección ordenado

Cada punto es un PR independiente con su propio ID, conforme a P-∅ (un PR = un ID).

### P1 — Honestidad del veredicto (bloqueante de todo diagnóstico posterior)
`cartridge_boot.rs` — cuando el runtime cae al contexto DATA-GAP, los cartuchos de ese intent **no** pueden reportarse como `manifest_not_admitted_by_backend`.
- Emitir **un** evento `info!` agregado por intent con la guarda exacta que falló (`no_edges` | `no_cfg` | `no_identity` | `no_start_token` | `no_router`), conforme a R9 (un resumen agregado, no log por ítem).
- Añadir la rama `else` que hoy falta en el `else if let` de `:1757`.
- Degradar la razón al productor ausente real (familia `v4_context_unavailable_*`).
- **Efecto:** ~2 M filas/día de diagnóstico engañoso pasan a ser una señal veraz y contable. No cambia ninguna decisión de emisión.

### P2 — Unidades del índice V3 (impacto funcional directo)
`pool_sync_worker.rs` — hacer que `V3PoolInfo::fee_bps` contenga realmente **bps** en ambas rutas (convertir pips→bps, `pips / 100`), o renombrar el campo a `fee_pips` y corregir todos los consumidores.
- **Antes de tocar:** auditar **todos** los consumidores de `V3PoolInfo` y de `arbx:pool_index_v3` (Rust + TS) porque el contrato es compartido; un cambio en un solo lado rompe la paridad.
- **Verificación obligatoria:** releer el índice en Redis y comprobar que un pool V3 de 0,3 % aparece como `30`, y que un ciclo V3 vuelve a ser evaluable.
- Precedente de unidad ya documentado en el propio archivo: `CATALOG-BACKFILL-01` y `fee_tier_pips_to_bps` (`pool_enumeration_worker.rs:575`).

### P3 — Cobertura de grafo por intent (el objetivo funcional)
Convertir los intents que hoy caen al fallback en intents con aristas, en este orden:
1. Intents cuyo pool tiene `v3_slot0` presente pero sin arista → revisar por qué `graph_builder` (`route_discovery/graph_builder.rs:493`) no compone arista; `agent_graph.rs:358-359` exige `sqrt_price_x96_raw` **y** `liquidity`.
2. Intents cuyo pool no tiene estado → el productor correspondiente (slot0 TTL 30 s; ¿rotación de ventana?).
3. Pools `enum_source='seed'` sin estado: 60 filas en el registro — decidir entre sincronizar o desactivar, con evidencia.

### P4 — Cola de PRs ya verificada (independiente de P1-P3)
Compilados en WSL2 esta sesión (ver §7): **#722** y **#727** con `cargo check --workspace --locked` = `EXIT=0` y 0 warnings.

---

## 7. Verificación de compilación Rust — desbloqueada

El bloqueo histórico `os error 4551` (Windows Smart App Control) está **resuelto localmente** vía WSL2 con toolchain zig (documentado en `docs/development/WSL2-RUST.md`, PR #728). Resultados de esta sesión, sobre el **árbol exacto** de cada PR extraído con `git archive` (no un mix):

| PR | commit | `cargo check --workspace --locked` | warnings |
|---|---|---|---|
| #722 `fix/rust-hotpath-dai-and-tag-01` | `61d9d5ae` | **EXIT=0** (168 s) | 0 |
| #727 `fix/price-plausibility-wire-01` | `5aa7006e` | **EXIT=0** (166 s) | 0 |
| #449 `fix/cartridge-gate-addr-01-symbol-input` | `faa6e5c8` | EXIT=0 (187 s) | 1 (pre-existente: `sqlx-postgres v0.7.4` future-incompat) |

`cargo check --workspace --locked` sobre `origin/main` completo: **EXIT=0** (150 s).

### 7.1 El PR #449 debe **cerrarse**, no mergearse

`#449` propone resolver `addr → symbol` para el gate de tokens del spine. **`main` ya resolvió ese bug de otra forma, y mejor:** *identity mode* (`searcher-rs/src/token_identity.rs`, `ConfigAwareEvaluator::with_token_identity`, gate addr-keyed por `(chain_id, address)`), que además:
- no hace una consulta a Redis por pierna,
- cachea el índice 30 s con detección de drift de allowlist,
- publica `arbx:universe:*` alimentando `POST /api/admin/tokens/resolve`,
- registra `token_identity.unresolved_symbols` de forma honesta (R8).

Verificado: `resolve_gate_tokens` / `gate_token_symbol` **no existen** en `origin/main` (0 ocurrencias), mientras que las llamadas a `index_for` / `with_token_identity` sí están presentes en `cartridge_boot.rs` (`:1594`, `:2641`). Mergear #449 reintroduciría el enfoque antiguo sobre una rama de 42 días con conflictos.

---

## 8. Correcciones a afirmaciones previas de esta sesión

Por disciplina de evidencia se registran los errores cometidos y su refutación:

| Afirmación previa | Estado | Evidencia que la corrige |
|---|---|---|
| "`origin/main` local = `85debb3a`, obsoleto" | **Falso** | `git rev-parse origin/main` = `6d29137d` = main de GitHub |
| "`faa6e5c8` está sin pushear" | **Falso** | `git log origin/<rama>..<rama>` vacío; origin contiene el commit |
| "37 PRs abiertos" | Corregido | 36 |
| "Todos los cartuchos fallan siempre el gate" | **Falso** | 63 completan discovery, 16 llegan al solver; el fallo es por intent |
| "167 pools activos están sin sincronizar" | **Falso** | Son V3: su estado es `v3_slot0` (presente), no `pool_reserves` |
| "`pools` tiene 79 filas" | **Falso** | 2.543 filas; el dato previo era `n_live_tup` sin `ANALYZE` |
| Réplica en PowerShell: "264/264 con digest conflictivo" | **Artefacto propio** | Colección de 1 elemento desenvuelta a escalar → `[0]` devolvía un `char`. La réplica correcta en Python da 264/264 **admitidos** |
| "El índice V3 tiene un bug de 100× confirmado" | **Degradado a no determinado** | `impact_index.rs:1394-1432` muestra normalización parcial (`500 → 30`) junto a `3000 → 3000`: las unidades no son uniformes y el consumidor manda. Ver §4.3 |

---

## 9. Trazabilidad de los artefactos citados

| Afirmación | Artefacto |
|---|---|
| Histograma de razones y `had_reserves` | `route_discovery_outcomes_p20260930`, ventana 2 h |
| 264 manifiestos admitidos | réplica fiel de `v4_admit_script` sobre `origin/main` |
| Archivos desplegados = `origin/main` | `sha256sum` de 264 archivos vs `git archive origin/main`: 0 diferencias |
| Mapa vacío en boot | `cartridge_boot.rs:193-215`, `:220`, `:275` |
| Guardas y fallback mudo | `cartridge_boot.rs:1749-1810` |
| Gate que rechaza primero | `snapshot_services.rs:186-190` |
| Bug de unidades | `pool_sync_worker.rs:1352-1358`, `:1398-1401`, `:1416-1424` |
| Índice con unidades mezcladas | `arbx:pool_index_v3:1:{weth:xpr,shib:weth,pepe:weth,usdt:wbtc}` |
| Composición de pools activos | `pools JOIN factories JOIN dexes`, `protocol_type` |
| Compilación de los 3 árboles | `cargo check --workspace --locked` en WSL2, árboles `git archive` |
