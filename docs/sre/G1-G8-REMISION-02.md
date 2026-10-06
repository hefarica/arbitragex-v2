# G1-G8 REMISIÓN-02 — ¿se sostiene el cierre del cuello contra el SHA NUEVO?

- **Tarea:** t61 · **Intento:** 1 · **attempt_id:** `13c46713-5e6d-45ed-a206-8d6808d542a6`
- **Rol:** SRE · **Rama:** `sre/g1-g8-remision-02` · **In-scope tocado:** `docs/sre/` (este documento).
- **Tipo:** **verificación de no-herencia** de la medición de `t54`. No es una medición nueva de gates.
- **Modo:** `paper`. Sin firma, sin broadcast, sin capital, sin mutar el VPS. Todo son GET read-only.
- **Rondas comparadas:** `t54` midió `3f00b359` (deploy.id `37399887059`). Ésta mide el despliegue que **aterrizó después**.

---

## §0 — VEREDICTO: la conclusión **SE SOSTIENE**

> *"El cuello `v3_quote_unavailable` YA NO ESTÁ VIVO y el embudo muere en ECONOMÍA, no en datos."*
> — **SE SOSTIENE contra `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`**, medido en **dos ventanas posteriores** al flip del deploy.

`v3_quote_unavailable` sigue en **0 apariciones**, verificado ítem por ítem en **cinco lecturas del embudo** repartidas en dos ventanas (y con tres sub-ventanas de parámetro en cada una). Y el resto de los hechos decisivos de `t54` se reproducen: `route_ready` / `verdict=ready` / 500 ciclos / `v3_skipped=0`, y el pricing V3 ocurriendo.

**No se hereda: se midió después del flip.** La sección §1 muestra la espera con su conteo y el momento exacto del cambio.

---

## §1 — El SHA realmente desplegado, y cómo se llegó a él

**El runtime servía el SHA VIEJO al empezar.** No medí antes de tiempo:

```
$ (poll cada ~45 s)  curl -s https://edge-arbx.ape-tv.net/status
poll[1]  03:31:49  /status.sha=3f00b359  id=37399887059   Deploy to VPS=in_progress
...
poll[12] 03:49:32  /status.sha=3f00b359  id=37399887059   run=in_progress
$ (lectura siguiente) 03:50:19  /status.sha=c89d21a3  id=37405962576     ← **FLIP**
```

**El flip ocurrió en el intervalo `(03:49:32Z, 03:50:19Z]`.** Las 12 lecturas previas —que abarcan 18 minutos de espera— vieron `3f00b359`. **Todas las mediciones de este documento son POSTERIORES al flip.**

**SHA desplegado, leído de dos fuentes independientes:**

```
$ curl -s https://edge-arbx.ape-tv.net/status            (ventana A, ts 2026-10-06T03:50:48.027Z)
deploy.sha = c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e
deploy.id  = 37405962576
deploy.at  = 2026-10-06T03:20:27Z
env = production-like     services ok:200 = 7

$ gh api repos/hefarica/arbitragex-v2/actions/runs/37405962576
status=completed conclusion=success
head_sha=c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e   updated=2026-10-06T03:50:29Z
  Wait for all deployment gates  completed/success  02:49:32Z → 03:20:08Z
  Deploy to VPS                  completed/success  03:20:10Z → 03:50:28Z
```

Las dos fuentes coinciden: **el `deploy.sha` que declara el runtime == el `head_sha` del run de auto-deploy que cerró `success`**. El run cerró a las `03:50:29Z`, **10 segundos después** de la primera lectura que ya veía el SHA nuevo — coherente con que los contenedores se recrean antes de que el workflow termine sus pasos finales.

**Nota de higiene:** `origin/main` **no** se usa como fuente (§del contrato). Para el registro, `origin/main` era `c89d21a3437c…` — igual que lo desplegado, pero la identificación se hizo por `/status` y por el run, no por el ref.

---

## §2 — Las mediciones decisivas de `t54`, repetidas

### Ventana A — `2026-10-06T03:50:41Z` → `03:50:51Z` (SHA `c89d21a`)

`deploy.sha` leído **dos veces** en la ventana: `03:50:41Z` y `ts 03:50:48.027Z` → **idéntico** `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`.

**1. Embudo `/api/opportunities/live` — 36 cards, `window_total=36`, ts `03:50:49Z`, 36/36 `rejected`:**

| `rejection_reason` | default | `?window=900` | `?window=3600` |
|---|---|---|---|
| `non_positive_profit` | 14 | 14 | 14 |
| `spread_negative_round_trip` | 10 | 9 | 9 |
| `single_pool_no_spread` | 4 | 4 | 4 |
| `no_price_oracle` | 2 | 2 | 2 |
| `v3_pool_revert` | 2 | 2 | 2 |
| `no_tradable_size` | 1 | 1 | 1 |
| `v3_multileg_budget_exhausted` | 1 | 1 | 1 |
| `v3_pair_no_pools` | 1 | 1 | 1 |
| `v3_pool_not_catalogued` | 1 | 2 | 2 |
| **`v3_quote_unavailable`** | **0 (ausente)** | **0 (ausente)** | **0 (ausente)** |

Verificación explícita, no visual: `v3_quote_unavailable_present = False` en las tres sub-ventanas.

**2. `/api/route-discovery/status`** (updated `03:50:49Z`, `mode=shadow`):

```
multi_hop_status            = route_ready
required_data_gate.verdict  = ready
required_data_gate.tier     = executable
multi_hop_profitable_cycles = 500
pools_total                 = 1128
multi_hop_v3_skipped        = 0
routes_found                = 500
strategy_status_counts      = {route_ready:79, needs_route_data:174, no_compatible_route:3, observe_only:8}
```

**3. `/api/cartridges/status`** (ts `2026-10-06T03:50:50.665Z`) — el pricing V3 OCURRE:

```
v3_source_priced cartridge=dex_arb chain=1 source_pool=0xc1cd3d0913f4633b43fcddbcd7342bc9b71c676f
  sqrt_price_x96=2137984667807226078900264083 liquidity=19684570791500004186960
  source_price=0.0007281988778372404 mode=shadow
```

### Ventana B — `2026-10-06T03:56:04Z` → `03:56:14Z` (mismo SHA, 5,5 min después)

`deploy.sha` leído **dos veces**: `ts 03:56:07.427Z` y `ts 03:56:12.096Z` → **idéntico** `c89d21a3437c…`, `deploy.id=37405962576`.

**1. Embudo — 19 cards, `window_total=19`, ts `03:56:12Z`, 19/19 `rejected`:**

| `rejection_reason` | default | `?window=900` |
|---|---|---|
| `non_positive_profit` | 8 | 7 |
| `spread_negative_round_trip` | 4 | 3 |
| `single_pool_no_spread` | 2 | 2 |
| `v3_pool_not_catalogued` | 2 | 2 |
| `no_tradable_size` | 1 | 2 |
| `v3_multileg_budget_exhausted` | 1 | 1 |
| `v3_pool_revert` | 1 | 1 |
| `v3_pair_no_pools` | — | 1 |
| **`v3_quote_unavailable`** | **0 (ausente)** | **0 (ausente)** |

**2. `/api/route-discovery/status`** (updated `03:56:12Z`): `multi_hop_status=route_ready`, `verdict=ready`, tier `executable`, `profitable_cycles=500`, `pools_total=1134`, `v3_skipped=0`.

**3. `/api/cartridges/status`** (ts `2026-10-06T03:56:14.828Z`): `v3_source_priced cartridge=dex_arb chain=1 source_pool=0xdeaf677af91ec0655124860e1a15177aee86303e … mode=shadow`.

**4. `/api/v1/health`**: `healthy`, 7/7 servicios `running`.

---

## §3 — Comparación cruzada: la conclusión no se hereda, se re-mide

| ronda | SHA servido | `deploy.id` | ventana | cards | `v3_quote_unavailable` | `route_ready`/`verdict=ready` | pricing V3 |
|---|---|---|---|---|---|---|---|
| `t54` | `3f00b359…` | 37399887059 | 03:22–03:27Z | 26 | **0** | sí / sí | sí |
| **t61-A** | **`c89d21a…`** | **37405962576** | **03:50:41–03:50:51Z** | **36** | **0 (ausente)** | **sí / sí** | **sí** |
| **t61-B** | **`c89d21a…`** | **37405962576** | **03:56:04–03:56:14Z** | **19** | **0 (ausente)** | **sí / sí** | **sí** |

**SHA distinto, resultados iguales** — y no porque se haya copiado: la identificación del SHA se hizo en cada ventana contra `/status` (dos lecturas por ventana) y contra el run de deploy. El `deploy.id` cambió (`37399887059` → `37405962576`), el `deploy.sha` cambió (`3f00b359` → `c89d21a`) y **el hecho medido no cambió**.

**Contraste con la lección de `t46`:** allí el hecho medido (la identidad horneada) **sí** cambió con el despliegue (`a38e6779` → `3f00b359`), y por eso hubo que re-medir. Aquí el hecho es un **comportamiento del pipeline**, y sobrevive al cambio de SHA. Ésa es precisamente la diferencia entre una medición que se hereda y una que no — y **sólo se sabe midiendo**: no se habría podido afirmar sin esta ronda.

---

## §4 — Un matiz que fortalece la lectura (y que no se debe perder)

En las cinco lecturas del embudo aparecen etiquetas V3 **granulares** que antes no se veían: `v3_pool_revert` (1–2), `v3_pair_no_pools` (0–1), `v3_pool_not_catalogued` (1–2), `no_price_oracle` (2). Eso es exactamente lo que los audits describían como el arreglo de la **clase aplanada**: antes esas causas se fundían en un único bucket de apariencia-transporte (`v3_quote_unavailable`) y ahora **se declaran con su etiqueta exacta** (`state_projector` / `dex_engine` separan catálogo, par sin pools, revert de pool y fallo de proveedor).

Lectura correcta, sin sobrevender:
- **El cuello de transporte/catálogo NO domina**: en la ventana A las razones V3-granulares suman **4 de 36 (11 %)** y las económicas (`non_positive_profit` + `spread_negative_round_trip`) suman **24 de 36 (67 %)**. En la ventana B: V3-granulares **4 de 19 (21 %)** y económicas **12 de 19 (63 %)**.
- **No digo "no hay problemas V3"**: hay 4 casos por ventana, y ahora están **nombrados con precisión** en vez de escondidos en un bucket. Eso es un sistema *más* honesto, no un sistema sin defectos.
- **La conclusión que se sostiene es la de `t54`, literal**: el cuello `v3_quote_unavailable` ya no está vivo y **el embudo muere en economía, no en datos**.

---

## §5 — Lo que esta verificación NO hizo (fronteras declaradas)

- **NO re-midió G2 ni G3.** Siguen **NO COMPUTADO** y **FAIL** respectivamente: son de `t58`/`t60`, por instrucción explícita. Este documento **no los toca ni los mueve**.
- **NO usó el canal SQL.** Sigue mudo (establecido en `t7` y re-verificado en `t54`): no se heredó ningún `0` de ahí, y no se intentó de nuevo porque el canal no cambió.
- **NO midió `simulations`/`executions`/ledger** por ninguna vía.
- **NO tocó** `backend/`, `frontend/`, `.github/workflows/`, `implementation-state/`, `docs/data/`, `docs/backend/`, `docs/release/` (fuera de in-scope). **NO mutó el VPS**: todo GET read-only.
- **NO empujó a `main`**: rama + PR sin merge. El push se hizo **después** de que el run de auto-deploy cerrara (`completed/success` a las `03:50:29Z`), respetando §22.20.
- **NO declara nada sobre el runbook de activación** (§contexto de `t57`): que el runbook no sea ejecutable tal cual **no cambia esta medición**, pero sí cambia qué significa "listo para activar" — y eso no es mío en esta ronda.

---

## §6 — Alcance

**Esto NO mueve P/N (0/115)** y **no acredita ningún otro criterio**. Es una **medición, no una activación**: `ARBX_TRADE_MODE=paper`, sin firma, sin broadcast, sin capital. Lo único que agrega al acervo es que **un hecho medido sobrevive a un cambio de despliegue** — con las dos ventanas y el flip documentados.

---

## §7 — Procedencia y reproducción

| # | Comando (crudo) | Resultado |
|---|---|---|
| 1 | `curl -s https://edge-arbx.ape-tv.net/status` ×(12 polls + 2 lecturas/ventana) | viejo `3f00b359` hasta `03:49:32Z`; **`c89d21a3437c…` / id `37405962576`** desde `03:50:19Z`; `at=2026-10-06T03:20:27Z`; `services ok:200 = 7` |
| 2 | `gh api repos/.../actions/runs/37405962576` | `completed/success`, `head_sha=c89d21a…`, gates `02:49:32→03:20:08Z`, Deploy `03:20:10→03:50:28Z` |
| 3 | `curl -s .../api/opportunities/live[?window=900\|?window=3600]` | ventana A: 36 cards; ventana B: 19 cards; `v3_quote_unavailable_present=False` en las 5 lecturas |
| 4 | `curl -s .../api/route-discovery/status` | `route_ready` · `ready` · tier `executable` · 500 ciclos · `pools_total` 1128 (A) / 1134 (B) · `v3_skipped=0` |
| 5 | `curl -s .../api/cartridges/status` | `v3_source_priced … mode=shadow` en A y B |
| 6 | `curl -s .../api/v1/health` | `healthy`, 7/7 running |

**Método reusado de `t54` §5** (declarado allí y no re-derivado): identificación del SHA por `/status` + run de deploy; embudo por tallies de `rejection_reason`; repetición en ventanas distintas; frontera declarada para todo lo no computable.
