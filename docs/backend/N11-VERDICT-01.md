# N11-VERDICT-01 — Confirmado: `verdict` se pierde, y el 100% de lo que se simula ya viene rechazado

**run_id** arbx-entrega-20261008 · **phase_id** implementation · **SHA_BASE** `8414e51211d0a26d664b7e669af2eacbee8d1dd4` (main medido)
**Rama** `fix/n11-verdict-01` (PR propio, SIN merge) · **dueño** Backend
**Alcance** `backend/sim-ctl/src/consumer.rs` (1 archivo) + `docs/backend/` · **NO se tocó** `shared-rs/`, `frontend/`, `monitoring/`, `docker/`
**Prohibiciones respetadas**: sin merge, sin deploy, sin tocar producción.

---

## 0. INSTRUMENTO

`command -v cargo` → `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` · `GATE1=PASS`.
md5 idéntico WSL↔Windows **antes** de correr: `consumer.rs a94bb9a40e20cacff2df3943676412b0` en ambos lados.

---

## 1. V1 — CONFIRMADO, Y MEDIDO (no aceptado de la auditoría)

### 1.1 El hecho de código: no existe consumidor

Se buscó el **LITERAL de la clave JSON**, no la palabra `verdict` — que en este repo aparece en **prosa** (comentarios del tipo *"a bound is a DEFERRAL, never a verdict"*) y habría dado un falso positivo:

```
git grep -n '"verdict"' origin/main -- backend/sim-ctl/
GREP_EXIT=1  ->  NINGUNA coincidencia
```

Y el contrato, en `shared-rs/src/contracts.rs`, **no declara la clave ni protege contra ella**:

```
git grep -n "deny_unknown_fields\|pub verdict\|verdict_reason" origin/main -- backend/shared-rs/src/contracts.rs
GREP_EXIT=1  ->  SIN COINCIDENCIAS
```

Los campos de `Opportunity` listados uno a uno lo confirman: `id`, `chain_id`, `strategy_kind`, `dex_a`, `dex_b`, `pair_symbol`, `token_in`, `token_out`, `amount_in_wei`, `expected_profit_usd`, `net_expected_profit_usd`, `roi_pct`, `risk_score`, `block_number`, `rejection_reason`, `cartridge_id`, `detector_id`, … **ni `verdict` ni `verdict_reason`**, y **tampoco `deny_unknown_fields`** — que es lo único que convertiría el descarte en un error ruidoso.

**⇒ `serde_json::from_str::<Opportunity>` (consumer.rs:810) descarta ambas claves EN SILENCIO.**

### 1.2 La medición sobre una entrada REAL

El test embebe una entrada **real** capturada con `XREVRANGE arbx:opps:validated + - COUNT 1` (1.534 bytes, verbatim) y mide las tres cosas a la vez:

1. **Control de productor**: el payload tiene `"verdict":"reject"` y `"verdict_reason":"producer_rejected"`. Sin esto el test no probaría nada.
2. **La medida**: `serde_json::from_str::<Opportunity>(REAL_ENTRY)` → **`Ok`**. Las claves desconocidas se **ignoran**. Si hubiera `deny_unknown_fields` sería un `Err` — y ese `Err` sería **ruidoso**, no silencioso. **Que sea `Ok` ES la medida del descarte.**
3. **Control**: el **mismo** string **sí** expone las claves por la vía que no depende del struct (`selector_verdict`) → `("reject", "producer_rejected")`. Prueba que el dato está **en el payload** y que lo que lo pierde es **`Opportunity`**, no el mensaje ni el transporte.

---

## 2. V2 — LA CUANTIFICACIÓN, Y ES 100%

Método: `XRANGE arbx:opps:validated - + COUNT 10000`, ventana de retención completa (`STREAM_MAXLEN = 10_000`). Instante: 2026-10-08.

```
payloads=10000
--- por verdict ---
  10000 "verdict":"reject"
--- por verdict_reason ---
  10000 "verdict_reason":"producer_rejected"
--- CONTROL: cuantos traen la clave verdict (productor probado) ---
10000
--- y cuantos NO la traen (control negativo) ---
0
```

**⇒ 10.000 de 10.000 = el 100% de lo que entra a `sim-ctl` lleva `verdict = reject`.** El control demuestra que el productor **sí** emite la clave (10.000), así que esto no es un cero sin productor: es una medición con productor probado.

**⇒ El daño no es «una fracción de `simulations` son rechazos»: es que `simulations` NO CONTIENE OTRA COSA.** Toda la tabla que la campaña viene midiendo describe oportunidades que el selector ya había descartado.

---

## 3. V3 — PARAR Y DECLARAR: el arreglo correcto está fuera de alcance, y aplicarlo aquí apagaría la simulación

**V3 dice literalmente**: *"Si el arreglo correcto resultara estar en `shared-rs` (la estructura `Opportunity`), **PARAR y declararlo** con el cambio propuesto."*

**Y así es.** El campo se pierde porque `Opportunity` no lo declara. La reparación correcta es **declararlo ahí** — `shared-rs/src/contracts.rs`, **fuera del inScope de esta tarea**. Cambio propuesto, sin aplicar:

```rust
// shared-rs/src/contracts.rs — struct Opportunity
    /// N11: veredicto del selector (#843). `#[serde(default)]` para que las
    /// filas anteriores al campo sigan deserializando (R8: ausente ≠ aceptado).
    #[serde(default)]
    pub verdict: Option<String>,
    #[serde(default)]
    pub verdict_reason: Option<String>,
```

**Y hay una segunda razón, que V2 obliga a declarar y que pesa más**: si el arreglo se aplicara como *"un `reject` no consume trabajo caro"*, con el **100%** de la ventana en `reject` **sim-ctl dejaría de simular por completo** y `simulations` se vaciaría. **V3 prohíbe decidir por preferencia y manda decidir con los hechos de V2** — y los hechos dicen que ese arreglo, tal cual, sería el fallo opuesto al que se busca.

**No se aplicó.** No se cambió la semántica del pipeline, no se descartó nada, no se fabricó ningún veredicto.

---

## 4. Lo que SÍ se hizo en alcance: el consumidor que faltaba

`consumer.rs` — **aditivo y sin efecto sobre ninguna decisión**:

```rust
pub fn selector_verdict(json: &str) -> (Option<String>, Option<String>)
```

Lee las dos claves del **JSON crudo**, antes del parseo que las descarta. Se cuenta por veredicto y se reporta por cadencia (evento `sim_consumer.selector_verdict`, un log cada 500 entradas consumidas — **R9**: ~3 líneas/min contra las 183/s del flooding que motivó R9).

**Qué compra**: que el descarte **deje de ser silencioso**. A partir del deploy, una sola línea de log dice qué fracción de lo consumido viene rechazada, y la decisión sobre el coste podrá tomarse **con datos continuos** en vez de con una ventana de 10.000.

**Qué NO hace**: **no filtra, no descarta, no altera qué se simula.** El veredicto se registra; la puerta sigue exactamente donde estaba.

---

## 5. V4 — FAIL-CLOSED INTACTO

`grep -c 'passed=.true.'` sobre la evidencia viva de métricas → **0**, **con productor probado**: las series `{passed="false",simulator="anvil"}` y `{simulator="revm"}` **se emiten**. Ningún umbral tocado, ningún número mejorado, ninguna entrada descartada.

---

## 6. V5 — LAS 4 PUERTAS

| puerta | resultado | evidencia |
|---|---|---|
| `command -v cargo` | **PASS** | `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` |
| `cargo fmt --all -- --check` | **PASS** | sin diff |
| `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` | **PASS** | `Checking sim-ctl v0.1.0`; `grep -cE '^(error\|warning)'` = **0** |
| `cargo test -p sim-ctl --no-fail-fast` | **PASS** | 0 fallos; tests nuevos por nombre: `v1_serde_discards_the_selector_verdict_silently`, `n11_absent_or_malformed_verdict_is_none_never_a_default` |
| `cargo test -p sim-core --no-fail-fast` | **PASS** | `79 passed; 0 failed` |

`Compiling` **O** `Checking` exigidos y presentes. md5 idéntico WSL↔Windows **antes** de correr. Exit code real por puerta.

---

## 7. V6 — PR PROPIO, Y EL IMPACTO SOBRE LA DAPP

PR propio, **SIN merge**. `git status --porcelain` → solo `M backend/sim-ctl/src/consumer.rs`.

**¿Puede afectar la Dapp?** **No, y por construcción.** El cambio es **aditivo y de solo lectura**: añade una función que lee dos claves del JSON crudo y un contador con su log. **No altera qué entradas se procesan, ni su orden, ni su resultado, ni qué se persiste en `simulations`.** La Dapp lee `simulations` y `opportunities`; ninguna de las dos cambia de forma ni de contenido. **La regresión se evita por la vía más fuerte posible: no hay camino de código nuevo que pueda cambiar un veredicto** — el valor leído se usa solo para contar y loguear.
