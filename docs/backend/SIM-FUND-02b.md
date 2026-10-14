# SIM-FUND-02b — Partir una familia de razones deja AMBAS afuera del clasificador

**run_id** arbx-entrega-20261007 · **phase_id** implementation · **SHA_BASE** `327433332732b06975dccadc725940cbeaccd4ca`
**Rama** `fix/sim-fund-02` (PR **#850**, MISMO PR — no se abrió uno nuevo) · **dueño** Backend
**Alcance** `backend/sim-ctl/src/persistence.rs` · **NO se tocó** `sim_engine.rs` (en vuelo en #848)

---

## 1. La premisa de la tarea era incorrecta — y hay que decirlo (K4)

La tarea afirma: *"#850 introduce dos razones nuevas al partir `slot_unresolved`"*. **No es así.**

`#850` añade tres constantes al módulo `outcome` de `signer_funding.rs` y tres llamadas
`count(outcome::X)`. El propio diff lo dice sin ambigüedad:

- `signer_funding.rs:161` → `count(outcome::WRITE_REJECTED);` (rama `Ok(false)` de `write_balance`)
- `signer_funding.rs:195` → `count(outcome::VERIFY_MISMATCH);` (rama `else` de la verificación)
- `signer_funding.rs:285` → `count(outcome::BALANCE_UNREADABLE);` (brazo `Err(_) => Ok(None)`)

`count()` escribe el label `outcome` de la métrica **`arbx_sim_funding_total`**. **Ninguna de las
tres se devuelve como `Err(...)`**, y el comentario del propio autor lo declara en el código:

> `// SIM-FUND-02: NO LONGER SILENT. ... Behaviour is unchanged (still Ok(None)): this adds visibility, not a verdict.`

El **único** `fail_reason` del agotamiento del loop sigue siendo `sim_signer_funding_slot_unresolved`
(`signer_funding.rs:203-204`). Y `count(outcome::SLOT_UNRESOLVED)` **sigue emitiéndose** en esa misma
línea, así que la alerta de Prometheus
(`monitoring/alerts.rules.yml:120`, `increase(arbx_sim_funding_total{outcome="slot_unresolved"}[30m]) > 5`)
**no se rompió**: su sensibilidad es la misma que antes de #850.

**Conclusión K1/K4:** la partición ocurrió en la capa de **MÉTRICA**, no en la de **`fail_reason`**.
`is_sim_capability_gap` **nunca es llamado** con `write_rejected` ni con `verify_mismatch`: recibe
`r.fail_reason` (`persistence.rs:73-77`), que no puede valer eso. Reconocerlas es, hoy, un no-op
demostrable — y se hizo igual, por el motivo del §2.

---

## 2. El hueco REAL: la familia de fondeo nunca estuvo clasificada

Independiente de #850, y medido: **ningún** `fail_reason` de `ensure_funded` estaba reconocido por el
clasificador. Los ocho que el código devuelve hoy:

| `fail_reason` | sitio | ¿reconocido antes? |
|---|---|---|
| `funding_balanceof_timeout` | `signer_funding.rs:117` | **NO** |
| `funding_balanceof_rpc: {e}` | `:121` | **NO** |
| `funding_setstorage_rpc: {e}` | `:136`, `:183` | **NO** |
| `funding_verify_timeout` | `:169` | **NO** |
| `funding_verify_rpc: {e}` | `:173` | **NO** |
| `sim_signer_funding_slot_unresolved` | `:204` | **NO** |
| `anvil_setStorageAt_timeout` | `:252` | **NO** |
| `anvil_setStorageAt: {e}` | `:256` | **NO** |

Medir el balance, verificar el centinela o escribir en el fork es el **ARNÉS**, no el mercado: un
`eth_call` que no responde no dice nada del spread. Que sigan fuera es el mismo modo de fallo que la
tarea describe — **F-01**: `!passed && !gap` ⇒ `UPDATE ... SET status='rejected'` (`persistence.rs:88-109`).

### 2.1 Es LATENTE, no vivo — y no se va a afirmar lo contrario

Prueba discriminante, contra PostgreSQL de producción:

```
SELECT o.status, o.rejection_reason, count(*)
  FROM simulations s JOIN opportunities o ON o.id=s.opportunity_id
 WHERE s.fail_reason LIKE '%funding%' GROUP BY 1,2;

 rejected | spread_negative_round_trip   | 4534
 rejected | non_positive_profit          |  678
 rejected | single_pool_no_spread        |  143
 rejected | StrategyDisabled:triangular_arb | 103
```

**Las 5.458 filas unen a oportunidades YA `rejected` con razón del DETECTOR.** El `UPDATE` está
gateado `WHERE status IN ('validated','scored','detected')` (`persistence.rs:101`), así que **no tocó
ninguna fila**. Y coherente: `SELECT count(*) FROM opportunities WHERE rejection_reason LIKE '%funding%'`
= **0** sobre 8.063.209 filas.

⇒ **El hueco no causó daño observable.** Se cierra *antes* de que lo haya, no porque ya lo haya. La
afirmación contraria — "F-01 vivo" — **no está soportada por la evidencia y no se hace.**

---

## 3. El arreglo, y por qué el prefijo va NAMESPACED

```rust
|| fail_reason.contains("write_rejected")
|| fail_reason.contains("verify_mismatch")
|| fail_reason.contains("balance_unreadable")
|| fail_reason.starts_with("sim_signer_funding")
|| fail_reason.starts_with("funding_balanceof_")
|| fail_reason.starts_with("funding_verify_")
|| fail_reason.starts_with("funding_setstorage_")
|| fail_reason.starts_with("anvil_setStorageAt")
```

Un `starts_with("funding_")` a secas sería **el fallo opuesto**. Grep sobre el repo: los cartuchos de
`searcher-rs` producen veredictos de mercado que empiezan con `funding_` —
`funding_edge_negative`, `funding_differential_within_band`, `funding_direction_unfavorable`. Un prefijo
amplio los tragaría y convertiría un **rechazo legítimo en silencio**. Los tres prefijos elegidos
están namespaced a la ruta de fondeo y no colisionan con ninguno. **El CONTROL del test lo fija.**

Las tres etiquetas nuevas se reconocen con `contains` (no `starts_with`) para cubrir la forma sufijada
`<reason>:<causa>`, que es como este codebase ya compone `multistep_*` y
`cyclic_route_missing_route_metadata:<kind>` — el modo natural de promoción si la métrica se hace
accionable. Ese es el motivo **prospectivo**: si se promueven a `fail_reason` sin esta cláusula,
caerían fuera de un clasificador **sin catch-all**.

---

## 4. K3 — CLASIFICACIÓN DE LECTORES (lista, no solo conclusión)

Escritores de cada string nuevo (§1, §2). Lectores **enumerados**:

| lector | archivo:línea | ¿enumera valores EXACTOS? | impacto |
|---|---|---|---|
| `is_sim_capability_gap` | `sim-ctl/src/persistence.rs:130` | **NO** — prefijos/`contains` | **este cambio** |
| `insert_simulation` (escribe `rejection_reason`) | `sim-ctl/src/persistence.rs:88-109` | NO | ninguno |
| alerta Prometheus | `monitoring/alerts.rules.yml:120` | **SÍ: `outcome="slot_unresolved"`** | **ver §4.1** |
| doc de métrica | `shared-rs/src/metrics.rs:65-67` | NO (comentario) | ninguno |
| docs / auditorías | `docs/`, `audits/` | NO (prosa) | ninguno |

**No hay ningún lector que haga `match` exhaustivo ni `==` sobre `sim_signer_funding_slot_unresolved`.**
No es un cambio de interfaz en el sentido de t99 (renombrar una familia que un lector enumera).

### 4.1 Hallazgo de interfaz: la alerta enumera UN solo valor

`monitoring/alerts.rules.yml:120` enumera **exactamente** `outcome="slot_unresolved"`. Tras #850 la
métrica emite además `write_rejected`, `verify_mismatch` y `balance_unreadable`. La alerta **no se
rompe** (`slot_unresolved` sigue contándose en `:203`), pero es **ciega a las tres series nuevas** —
que son justamente las que #850 creó para diagnosticar. **Declarado, NO reparado**: `monitoring/` está
fuera del `inScope` de esta tarea. Es un cambio de interfaz pendiente y debe tratarse como tal.

---

## 5. K7 — Qué compra este cambio, sin adornos

**Sin deploy, no mueve ninguna métrica.** `sim_signer_funding_slot_unresolved` sigue en **1731 / 24 h**
y `passed=true` sigue en **0**. Lo que compra es que la próxima medición pueda discriminar (a) `write`
rechazado vs (b) centinela no reproducido **sin mutar el fork de producción**, y que la familia que
hoy sí llega al clasificador deje de estar a un `UPDATE` de convertirse en rechazo silencioso.

Un push a rama **no despliega** (`auto-deploy-vps.yml` dispara sólo en `push: branches: [main]`).
**PROHIBIDO mergear**: merge = deploy.
