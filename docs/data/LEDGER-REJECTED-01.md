# LEDGER-REJECTED-01 — ¿los 598K REJECTED son el mercado o el pipeline?

**Orden:** t58 · **Perfil:** Data · **Intento:** `51deec03-0b61-4e1c-aa82-dbd5db8bead3`
**Base medida por mí:** `origin/main` = **`c89d21a3`** · **Clon aislado** `C:\Users\HFRC\Desktop\arbx-t49\repo` · Sin firma, sin broadcast, sin capital. `ARBX_TRADE_MODE=paper`.

**Aviso de staging:** este PR nace de `main` `c89d21a3`, **anterior a #818** (`GITIGNORE-DOCS-DATA-01`), así que en esta base `docs/data/` **sigue ignorado** y el archivo entró con `git add -f`. **Declarado, no oculto.** Cuando #818 mergee, deja de ser necesario.

---

## 0. VEREDICTO EN UNA LÍNEA

**DEFECTO DEL PRODUCTOR**, no mercado nominal. El rechazo masivo se compone de clases que **no son juicios económicos sobre el mercado**: `unscaled_legacy` (≈81.6%), `cap_clamp_failed` (≈8.7%) y `TokenNotAllowed:<addr>` (≈3.4%) — ninguna de ellas evalúa el spread. El mercado nominal **no puede** cargar con 598K rechazos porque una condición nominal exige que la oportunidad **haya sido evaluada**, y la clase dominante está definida por un defecto que **vuelve la evaluación sin sentido** (la propia API la excluye por "económicamente sin sentido", `paper-history-api.ts:115-117`).

**Hallazgo estructural que reordena el problema:** el label `REJECTED` **no es un juicio**. `status_from_rejection_reason` (`searcher-rs/src/persistence.rs:53-58`) lo deriva de la **presencia** de un motivo: `None => 'detected'`, `Some(_) => 'rejected'`. Es decir, `REJECTED` es un **bucket que incluye "nunca lo evaluamos"** — `TokenNotAllowed:<addr>` ("ese token ni está permitido") cae en el mismo bucket que `spread_zero_equilibrium` ("lo evaluamos y no hay spread"). **Por eso el label no puede discriminar mercado de productor: hay que abrir el motivo, y después el motivo por su valor económico.**

---

## 1. QUÉ ES EXACTAMENTE "598K runs todos REJECTED"

La frase de G3 (`audits/live-activation-package-20260917/GATES-G1-G8-ESTADO.md:9`) junta **tres poblaciones distintas** en una sola etiqueta. Separadas:

| # | Población | Tabla / columna | Qué significa de verdad |
|---|---|---|---|
| P1 | **598.878 filas** ("el ledger") | `paper_trade_runs` (fila completa) | Runs paper. **No** llevan un status `REJECTED`: llevan `reason` (NULL o texto) |
| P2 | **`REJECTED`** | `opportunities.status` derivado de `opportunities.rejection_reason` (`persistence.rs:53-58`; enum en `migrations/003_opportunities.sql:22`) | Bucket derivado de `rejection_reason IS NOT NULL` |
| P3 | **`passed=false`** | `simulations.passed` (`migrations/004_simulations.sql:13`) | La simulación no pasó. Es la que G3 cuenta como "0 candidatos" |

La fuente del conteo exacto y de su etiquetado, medida por la auditoría de ciclos reales:

```
audits/real-cycles-audit-20260916/GOAL-WORKORDERS.md:18
"... paper_trade_runs = 598,878 PERO 0/598,878 joinea con sim passed
 (join=72,717 por opportunity_id, passed=0): todas REJECTED
 (non_positive_profit|unscaled_legacy 336K+152K, cap_clamp_failed 52K, TokenNotAllowed 20K)
 = defecto R-0001."
```

y el conteo independiente de la capa de datos:

```
audits/omniscience-integration-2026-09-06/07-data-layer-CROSS.md:45
"paper_trade_runs 598.878 filas, labels=0, sim_attempts max=0 | exacto: n=598878 labeled=0
 max_attempts=0 | CONFIRMADO — mi '670K' de ronda 1 era n_live_tup (estimado inflado: hoy
 669.578 vs 598.878 exacto)"
```

**Colisión de etiquetas declarada (la clase de error que esta casa ya pagó):** G3 dice *"todos REJECTED (R-0001, ledger)"*. Pero **R-0001 es la regla que IMPIDE que una oportunidad rechazada entre al ledger** (`archiverRejectionSkip`, `paper-trade-archiver.ts:110-121`: *"a REJECTED opportunity is never a paper trade"*; y el gemelo en `relays-client`: `submit_engine.rs:84`, `persistence.rs:171-183`). Las dos consecuencias que no se pueden sostener a la vez:
- Si R-0001 se aplicó, el ledger **no puede** contener filas rechazadas ⇒ el "REJECTED" del ledger es un label heredado de **filas anteriores al gate**.
- Si las filas del ledger llevan `reason` de familia de rechazo (y **sí lo llevan**: la API las filtra por eso), entonces son **pre-gate**.

**Fecha del gate medida:** `archiverRejectionSkip` entró en `cfafa012`, **2026-08-25** (PR #464). La ventana exacta de escritura de las 598K (`MIN/MAX(created_at)`) queda **NO COMPUTADA** (§5) porque PostgreSQL es mudo desde mi estación.

**Y el otro extremo del mismo error, medido en el propio consumidor:**

```
backend/api-server/src/routes/paper-history-api.ts:114-117
// A4 fix (R8-04): split into ACCEPTED (reason IS NULL) vs ALL rows.
// The dashboard was showing avg profit of REJECTED opportunities as if it
// were P&L — economically meaningless. The `accepted` block is the honest
// P&L signal; `all` remains for volume context.
```

La API define **ACCEPTED ≡ `reason IS NULL`** (`:149`) y filtra `reason NOT LIKE '%unscaled_legacy%'` (`:122`). Es decir: el sistema **ya sabía** que esa población no es P&L. Lo que faltaba era decir que tampoco es **mercado**.

---

## 2. EL TALLY Y SU FUENTE (citado, NO re-medido)

| Familia de motivo | Filas (reportadas) | Share sobre 598.878 | ¿Es un juicio económico? |
|---|---|---|---|
| `non_positive_profit\|unscaled_legacy` | 336K + 152K = **488K** | **81.6%** | **NO** — contaminada por el defecto de escala |
| `cap_clamp_failed` | **52K** | 8.7% | **NO** — falla del clamp de sizing |
| `TokenNotAllowed:<addr>` | **20K** | 3.4% | **NO** — el token ni está permitido |
| Total explicado | ≈560K | ≈93.5% | — |

**Fuente:** `audits/real-cycles-audit-20260916/GOAL-WORKORDERS.md:18` (WO-04, marcado DONE), con el corte primario en `audits/omniscience-integration-2026-09-06/07-data-layer-CROSS.md:45`.
**Frontera declarada:** este tally lo **midió otro** (auditoría 2026-09-16) y yo **NO lo re-medí** — PostgreSQL inalcanzable (§5). No lo presento como medición propia, y las tres clases que lo componen no están discriminadas al 100% (≈93.5% explicado; el 6.5% restante no se declara).

**Lo que SÍ verifiqué por código — los productores de cada clase:**

| Clase | Productor (file:line) | Naturaleza |
|---|---|---|
| `unscaled_legacy` | Filtro de lectura: `paper-history-api.ts:122` (`reason NOT LIKE '%unscaled_legacy%'`) y su justificación `:115-117` | **Marcador de filas legacy sin escalar**. La API las **excluye**: son "económicamente sin sentido" |
| `cap_clamp_failed` | `searcher-rs/src/size_optimizer.rs:414` (`Self::CapClampFailed => "cap_clamp_failed"`) | **Sizing**: el optimizador no pudo clampear al tope |
| `TokenNotAllowed:<0x…>` | `rejection-breakdown.ts:61` (regex del formato) · `opportunities.rejection_reason` | **Allowlist/cobertura**: rechazo **antes** de leer economía |

---

## 3. LA DISTINCIÓN MERCADO vs PRODUCTOR — resuelta por INSTRUMENTO, no por plausibilidad

### 3.1 El test decidible que ya existe en el repo

El discriminador **no** es el motivo: es **si la fila llegó a tener un valor económico**. La propia ruta de breakdown lo implementa (`rejection-breakdown.ts:174-186`):

```sql
SELECT split_part(rejection_reason, ':', 1) AS family_raw,
       rejection_reason AS raw_reason,
       COUNT(*)::int                        AS n,
       COUNT(expected_profit_usd)::int      AS gross_n,     -- ← EL DISCRIMINADOR
       COUNT(net_expected_profit_usd)::int  AS net_n,
       AVG(expected_profit_usd)::text       AS avg_gross,
       AVG(net_expected_profit_usd)::text   AS avg_net
  FROM opportunities
 WHERE detected_at > now() - ($1::int * interval '1 hour')
   AND rejection_reason IS NOT NULL
 GROUP BY 1, 2 ORDER BY n DESC LIMIT 500;
```

**Regla de decisión (decidible, sin prosa):**

| `gross_n` de la familia | Lectura | Veredicto |
|---|---|---|
| **`gross_n` = 0** | ninguna fila de esa familia tiene valor económico ⇒ **nunca se evaluó** | **DEFECTO DEL PRODUCTOR** |
| **`gross_n` = n y `avg_gross ≤ 0`** | se evaluó y el mercado no daba spread | **NOMINAL DEL MERCADO** |
| **`gross_n` = n y `avg_gross > 0` con `avg_net ≤ 0`** | había spread bruto y los costos se lo comen | **NOMINAL** (pero *accionable*: fees/sizing) |

### 3.2 Clasificación por familia (semántica de cada emisor, verificada en fuente)

**PRODUCTOR — la oportunidad nunca llegó a juicio económico:**

| Motivo | Cita | Por qué es productor |
|---|---|---|
| `unscaled_legacy` | `paper-history-api.ts:115-117` | Unidades sin escalar: el número **no es** económico (la API lo excluye del P&L) |
| `cap_clamp_failed` | `size_optimizer.rs:414` | Falla del clamp de tamaño, no del mercado |
| `TokenNotAllowed:<addr>` | `rejection-breakdown.ts:61` | El token no está en la allowlist: se rechaza **antes** de leer precio/spread |
| `v3_quote_unavailable` | `dex_engine.rs:37-38, 509` | *"V3 projector missing / quote failed on a leg / both legs quoted zero (G-ECON-1)"* — **la pipeline de quotes no produjo nada usable** |
| `v3_pool_not_catalogued` · `v3_pair_no_pools` | `dex_engine.rs:39-43` | Hueco de catálogo **zero-RPC**: no se consultó la cadena |
| `v3_pool_revert` | `size_optimizer.rs:418, 433` | La sub-llamada a QuoterV2 ejecutó y el pool revirtió: quote no disponible |
| `v3_multileg_budget_exhausted` | `size_optimizer.rs:423`, `:220` (*"never a fabricated price"*) | Se agotó el presupuesto de quotes: **nunca se cotizó** |
| `no_price_oracle` | `dex_engine.rs:35-36` | Token sin precio USD canónico: sin insumo económico |

**MERCADO — hubo juicio económico y el resultado fue "no hay spread":**

| Motivo | Cita | Por qué es nominal |
|---|---|---|
| `spread_zero_equilibrium` | `dex_engine.rs:44-48` | *"the CHAINED round trip returned EXACTLY the probe — a true equilibrium, not a data gap"* |
| `spread_negative_round_trip` | `dex_engine.rs:49-53` | Round trip devolvió **menos**: **pérdida MEDIDA**, publicada como bruto negativo |
| `non_positive_profit` | `dex_engine.rs:54` (`non_positive_spread`: spread ≤ 0 tras CPMM) | Evaluado con matemática CPMM |

**ESTRUCTURAL / COBERTURA — ni mercado ni juicio económico pleno:**

| Motivo | Cita | Lectura |
|---|---|---|
| `single_pool_no_spread` | `dex_engine.rs:34`, `:287` | Un solo pool en el `ImpactSet` ⇒ no hay ciclo que arbitrar. Es **cobertura de pools**, no una lectura del mercado |

**Entonces:** de las tres clases que componen los 598K, **las tres son PRODUCTOR** (por construcción: `unscaled_legacy` = valor no económico, `cap_clamp_failed` = sizing, `TokenNotAllowed` = allowlist). El veredicto de §0 no depende de que el mercado esté o no tenso: depende de que **esas filas nunca fueron un juicio**.

---

## 4. CRUCE CON EL HALLAZGO DE t32 (24/24 rejected)

**Medido por t32 (Frontend), citado:** 24/24 oportunidades servidas `rejected`, con motivos escritos por un productor: `spread_zero_equilibrium`(9), `non_positive_profit`(3), `single_pool_no_spread`(2), `v3_multileg_budget_exhausted`(1), `v3_pool_revert`(1) — 16 con motivo nombrado, y `viable_only=true` devolviendo **200 con 0 filas**.

| Dimensión | Veredicto |
|---|---|
| **Tasa** (100% rejected) | **CONSISTENTE** — el rechazo sigue siendo total |
| **Composición** | **CONTRADICTORIO y más informativo** — la muestra reciente está dominada por `spread_zero_equilibrium` (**9/16 = 56%**), que es una clase **NOMINAL DEL MERCADO** ausente por completo del tally de 598K |

**Lectura que esto habilita (y que es el aporte del cruce):** los arreglos del 2026-10-05 **cambiaron el modo de falla**. El ledger histórico rechazaba por **no poder evaluar** (escala, clamp, allowlist, quotes ausentes: 100% productor); la muestra reciente rechaza mayormente por **haber evaluado y no encontrar spread** (56% mercado), con un remanente productor (`v3_multileg_budget_exhausted` + `v3_pool_revert` = 2/16 = 12.5%) y un remanente estructural (`single_pool_no_spread` = 12.5%).

**No es una contradicción:** es la firma de que la fix movió el cuello de botella del **productor de candidatos evaluables** al **mercado sin spread** — y el cuello declarado por G2 (`v3_quote_unavailable` → 0 candidatos evaluables) sigue siendo **productor**, de la clase de quotes (§3.2). **Un `spread_zero_equilibrium` con 0 candidatos evaluables no puede explicar G2; un `v3_quote_unavailable` sí.**

**Frontera:** la composición de la muestra reciente es de **n=16 con motivo** sobre n=24 — es una muestra, no un tally. La distribución actual completa está **NO COMPUTADA** (§5).

---

## 5. ¿SIGUE OCURRIENDO HOY? — lo medido, y lo que no se puede medir

### 5.1 Lo que SÍ medí

**Los cuatro arreglos están EN PRODUCCIÓN** (`git merge-base --is-ancestor` en mi clon):

```
4a142e7c (#797 EXACT-QUOTES-PRODUCER-01)  en_desplegado(3f00b359)=True  en_main(c89d21a3)=True
9adbbea1 (#791 PRICE-COVERAGE-01)         en_desplegado(3f00b359)=True  en_main(c89d21a3)=True
2da9a473 (#792 SPREAD-SIGNED-DELTA-01)    en_desplegado(3f00b359)=True  en_main(c89d21a3)=True
fc7aba20 (#793 ECON-AMOUNT-DENOM-01)      en_desplegado(3f00b359)=True  en_main(c89d21a3)=True
```

**El commit desplegado es `3f00b359`**, medido por el canal que SÍ alcanza el VPS (`runtime-identity-probe.yml`, run `37404762561`, **2026-10-06T02:33Z**): los diez servicios reportan `built_from_sha = 3f00b359beca82685280c5d8d30f099d8bd7d921`; `api-server` con veredicto `verified` y `container_started_at = 2026-10-06T02:33:11Z`.

**Y `main` no introduce ningún cambio de productor desde entonces:** `3f00b359` es ancestro de `c89d21a3`, y los **6 commits** intermedios son CI/SRE/docs (`9456bec3 fix(ci)`, `19198dcf feat(ci) verifier`, `e04b86cd docs(sre)`, + 3 merges). ⇒ **la ventana de código medida es: los arreglos corren desde la mañana del 2026-10-06, sin cambios de productor posteriores.**

**Corrección de fecha:** la orden dice *"los merges del 2026-10-06"*. Medido: `mergedAt` = **2026-10-05T19:12:53Z / 19:12:59Z / 19:13:05Z / 19:15:06Z**. La actividad del 10-06 (02:41-02:48Z) fue #812/#814/#815 (CI/SRE/docs).

### 5.2 Lo que NO se puede medir desde acá — **NO COMPUTADO**, con la razón exacta

```
$ sql_query: SELECT rejection_reason, COUNT(*) FROM opportunities GROUP BY 1 ORDER BY 2 DESC
{ "ok": true, "data": "", "rows_affected": 0 }        ← payload VACÍO, no un resultado de 0 filas
$ sql_tables
{ "count": 0, "tables": [] }
```

**PostgreSQL es inalcanzable desde mi estación** — igual que desde la del capitán. Eso es **NO COMPUTADO**, no un cero. **Ninguna cifra de este documento se rellenó con 0.**

**El canal de GitHub Actions que la orden propone existe, pero NO sirve para esta consulta** (medido, no supuesto):

| Workflow | ¿Alcanza el VPS? | ¿Puede correr mi SQL? |
|---|---|---|
| `pipeline-integrity.yml` | **NO** — `ssh: Could not resolve hostname arbx: Temporary failure in name resolution`, exit 255 | No |
| `gsim1-readiness-attention.yml` | **SÍ** (success 10-03/10-04/10-05, SSH por `secrets.VPS_SSH_HOST`) | No — el query está **hardcodeado** a `readiness_evidence WHERE gate_id='G-SIM-1'` (`:70`) |
| `vps-janitor.yml` · `runtime-identity-probe.yml` | **SÍ** | No — no ejecutan SQL arbitrario |
| Cualquier workflow con input libre (`sql`/`command`) | — | **No existe ninguno** (barrido de todos los dispatchables: 0 workflows con input de comando libre) |

Agregar un workflow que corra esa consulta es **cambio de código fuera de mi alcance** (`In scope: docs/data/`) ⇒ se **nombra** y se propone, no se hace.

### 5.3 EL HALLAZGO QUE EXPLICA POR QUÉ NADIE TIENE EL NÚMERO

`pipeline-integrity.yml` se anuncia a sí mismo como *"the automated guarantee that the cards show REAL opportunities from REAL data in REAL time"* y corre **cada 15 minutos** contra el VPS. **Nunca llegó al VPS desde GitHub Actions.** Log del run más reciente, primer step:

```
$ gh run view 37396682639 --log
2026-10-06T00:57:26.8517963Z ssh: Could not resolve hostname arbx: Temporary failure in name resolution
2026-10-06T00:57:26.8536837Z ##[error]Process completed with exit code 255.
```

El workflow asume un alias SSH `arbx` (el del portátil del operador) y **no tiene ningún step que materialice la clave ni el host** — a diferencia de los workflows que sí funcionan (`auto-deploy-vps.yml:58-71` usa `secrets.VPS_SSH_KEY` + `secrets.VPS_SSH_HOST`).

**Consecuencia, medida:** sus **6 conclusiones `failure` consecutivas** (2026-10-04 → 2026-10-06) son **el fallo del chequeo, no el del pipeline**. Leídas como *"el pipeline lleva dos días roto"* dirían algo que el log no dice. Es exactamente el mismo error de esta orden —una etiqueta leída como otra cosa— en la capa de observabilidad: **el gate que debía detectar el rechazo masivo no puede conectarse, y su rojo es mudo.**

### 5.4 El canal que SÍ mediría esto hoy (para el operador, sin SQL)

La consulta decisiva está expuesta como **endpoint público read-only** del propio api-server (`rejection-breakdown.ts:152`):

```
GET /api/v1/rejections/breakdown?hours=24&chain_id=1
```

Devuelve, por familia: `count`, `share_pct_of_rejected`, **`avg_gross_usd`**, **`avg_net_usd`** y `top_raw` — es decir, **el tally que pide el Verify y el `gross_n` que decide §3.1**, ya normalizado y con timeout propio (`:132-150`). Cualquier máquina que alcance el VPS (o el frontend del DApp) lo mide sin acceso a PostgreSQL.

**Queries literales pendientes de correr** (para t54 o la orden que abra el canal):
```sql
-- 1. Tally + discriminador (la de §3.1, verbatim de rejection-breakdown.ts:174-186)
SELECT split_part(rejection_reason,':',1) AS family, rejection_reason, COUNT(*) n,
       COUNT(expected_profit_usd) gross_n, COUNT(net_expected_profit_usd) net_n,
       AVG(expected_profit_usd) avg_gross, AVG(net_expected_profit_usd) avg_net
  FROM opportunities WHERE detected_at > now() - interval '24 hours' AND rejection_reason IS NOT NULL
 GROUP BY 1,2 ORDER BY n DESC LIMIT 500;

-- 2. La ventana de escritura del ledger legacy (declarada NO COMPUTADA en §1)
SELECT count(*) FROM paper_trade_runs;
SELECT count(*) FILTER (WHERE reason IS NULL) AS accepted, min(created_at), max(created_at) FROM paper_trade_runs;

-- 3. El conteo de G3
SELECT count(*) FROM executions;
```

---

## 6. P/N NO SE MUEVE

**`P/N` sigue en `0/115`.** Este trabajo **no** lo mueve, **no** cierra ningún criterio de negocio y **no** convierte en verificado ningún otro criterio. Explica una causa y separa dos poblaciones que estaban mezcladas; **G3 sigue en FAIL** mientras `COUNT(*) FROM executions` = 0 y el desglose actual no se mida por el canal de §5.4.

---

## 7. RESUMEN PARA LA CÉLULA

| Pregunta | Respuesta | Portador |
|---|---|---|
| ¿Por qué 598K quedaron REJECTED? | Clases **no económicas**: `unscaled_legacy` (81.6%), `cap_clamp_failed` (8.7%), `TokenNotAllowed` (3.4%) | `real-cycles-audit-20260916/GOAL-WORKORDERS.md:18` (citado, no re-medido) + productores verificados por código |
| ¿Mercado o productor? | **PRODUCTOR**. Una condición nominal exige evaluación; la clase dominante es un defecto de escala que la propia API excluye por "económicamente sin sentido" | `paper-history-api.ts:115-117`; §3.1-§3.2 |
| ¿Consistente con 24/24 de t32? | **Tasa: consistente** (100% rechazado). **Composición: no** — la muestra reciente es 56% `spread_zero_equilibrium` (NOMINAL), clase **ausente** del tally de 598K ⇒ los arreglos cambiaron el modo de falla | t32 (citado) + §4 |
| ¿Sigue ocurriendo hoy? | Los arreglos **están en producción** (`built_from_sha=3f00b359` @ 2026-10-06T02:33Z; `main` = +6 commits CI/SRE/docs). La **tasa actual NO está medida** | `runtime-identity-probe.yml` run `37404762561`; §5.2 |
| ¿Y el gate que debía vigilarlo? | **Roto**: `pipeline-integrity` nunca alcanzó el VPS (`ssh ... arbx ... exit 255`); sus 6 `failure` son del chequeo, no del pipeline | `gh run view 37396682639 --log` |
| ¿Mueve P/N? | **NO** | §6 |

---

*PostgreSQL inalcanzable desde esta estación (payload vacío, no cero filas) ⇒ el tally y la ventana actual quedan NO COMPUTADOS con su razón y con el canal que los mediría. Todo lo demás está citado con file:line o medido por comando. Sin firma, sin broadcast, sin capital. No mueve P/N.*
