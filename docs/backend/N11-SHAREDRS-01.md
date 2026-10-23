# N11-SHAREDRS-01 — El cambio al struct SE MIDIÓ Y **ROMPE FUERA DE ALCANCE**: se para y se reporta

**run_id** arbx-entrega-20261008 · **phase_id** implementation · **SHA_BASE** `8414e51211d0a26d664b7e669af2eacbee8d1dd4`
**Rama** `fix/n11-sharedrs-01` (PR propio, SIN merge) · **dueño** Backend
**Alcance tocado**: `backend/sim-ctl/tests/` + `docs/backend/` · **`shared-rs/src/contracts.rs` NO se modificó** (ver §2)
**Prohibiciones respetadas**: sin merge, sin deploy, sin tocar producción, sin tocar `shared-ts/`.

---

## 0. INSTRUMENTO

`command -v cargo` → `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` · `GATE1=PASS`
md5 idéntico WSL↔Windows **antes** de correr cada puerta: `n11_sharedrs_01.rs 2e1e1b09ebf44eaa0ea035cd81defe97` en ambos lados.
(El formateo se aplicó en WSL y **se copió de vuelta**, con md5 re-verificado — no se dio por bueno un archivo sin comprobar.)

---

## 1. AC1 — EL CAMBIO SE ESCRIBIÓ Y **COMPILA**… EN `shared-rs`

El cambio propuesto, **exactamente** el que t115 declaró:

```rust
    #[serde(default)]
    pub verdict: Option<String>,
    #[serde(default)]
    pub verdict_reason: Option<String>,
```

`RUSTFLAGS='-D warnings' cargo check -p shared-rs` con el cambio aplicado → **PASS, 0 errores, 0 warnings**. El struct, por sí solo, está bien.

**NO se deriva `Default`** para `Opportunity`, y es deliberado: un `Opportunity` por defecto sería **una fila fabricada** (RULE 00). Queda escrito en el comentario del cambio.

---

## 2. ★ AC5 — SE ROMPE **FUERA DE ALCANCE**: PARO Y LO REPORT [esto es lo que manda]

AC5 lo exige literal: *"El struct es COMPARTIDO: compilar también los crates que lo consumen y declarar si alguno se rompe; **si se rompe, PARAR y reportarlo**."*

**Se rompe.** `cargo check --workspace` con el cambio aplicado:

```
error[E0063]: missing fields `verdict` and `verdict_reason` in initializer of `Opportunity`
errores totales: 15
```

| crate | errores `E0063` (código no-test) |
|---|---|
| `searcher-rs` | **15** — `workers/triangular_worker.rs` (2), `cartridge_boot.rs` (2), `workers/liquidation_worker.rs`, `workers/flashloan_arb_worker.rs`, `route_discovery/hop_cycle_bridge.rs`, `patterns.rs`, `engines/triangular_engine.rs`, `engines/spanning_tree_engine.rs`, `engines/liquidation_snipe_engine.rs`, `engines/liquidation_engine.rs`, `engines/dex_engine.rs`, `engines/cross_chain_bridge_engine.rs` |
| **todos los demás** | **0** |

**Y el alcance real es mayor**: `searcher-rs` tiene **52 literales `Opportunity {` repartidos en 29 archivos** (`size_optimizer.rs` 6, `hot_path_emitter.rs` 4, `triangular_worker.rs` 3, `opportunity_emitter.rs` 3, `cross_chain_bridge_engine.rs` 3, …). `cargo check` sin `--tests` no ve los que están en `#[cfg(test)]`, así que el número de 15 es **cota inferior**.

**La causa es de Rust, no de serde**: `#[serde(default)]` cubre la **deserialización**, no la **construcción**. Un literal de struct tiene que enumerar **todos** los campos, y `Opportunity` no deriva `Default`, así que no se puede cerrar con `..Default::default()`. **Añadir un campo obliga a editar los 52 literales**, y 29 archivos de `searcher-rs` están **fuera del inScope** de esta tarea.

**⇒ DECISIÓN: el cambio NO se aplicó.** `backend/shared-rs/src/contracts.rs` queda **byte-idéntico a main** (revertido), precisamente para no dejar la rama sin compilar. Evidencia del estado final: `git status --porcelain` → sólo el test nuevo y dos docs sin trackear; `cargo check --workspace` → **PASS, 0 errores**.

**Lo que hace falta para desbloquearlo** (para la unidad siguiente):

1. **Extender el inScope a `backend/searcher-rs/src/`** y actualizar los 52 literales — mecánico, con dos variantes:
   - añadir `verdict: None, verdict_reason: None,` a cada literal (explícito, preferido: no introduce un `Default`), o
   - derivar `Default` y usar `..Default::default()` (**no lo recomiendo**: hace construible un `Opportunity` fabricado).
2. **No tocar `shared-ts/`** (AC6, ver §6).

---

## 3. AC2 — ESTO NO FILTRA NADA

**Ninguna** de las dos variantes descartadas. No se descarta, no se salta, no se altera qué se simula: **lo único que hace el cambio es declarar el campo para que deje de perderse en silencio.** El filtrado exigiría una razón medida de **por qué el productor rechaza el 100%** — y con el 100% de la ventana en `reject` (10.000/10.000 medido por t115), un «skip rejects» **apagaría la simulación entera y vaciaría `simulations`**: el fallo **opuesto** al que se busca. La puerta se mueve cuando haya razón medida, no antes.

---

## 4. AC3 — EL TEST BIDIRECCIONAL, y lo que MIDIÓ al escribirlo

`backend/sim-ctl/tests/n11_sharedrs_01.rs` (in-scope, 4 tests, **todos PASS**):

1. **`v1_the_discard_is_silent_not_an_error`** — sobre una entrada **REAL** del stream embebida verbatim: el payload trae las claves, `serde_json::from_str::<Opportunity>` devuelve **`Ok`**, y el resto del payload sí llega. **Que sea `Ok` ES la medida del descarte silencioso** (con `deny_unknown_fields` sería un `Err` ruidoso). Fija además que si algún día pasa a `Err`, el diagnóstico cambió **a propósito**.
2. **`ac3_present_keys_are_read_verbatim`** — con las claves presentes se leen sus **valores**, tal cual; y un veredicto desconocido se expone sin normalizar.
3. **`ac3_absent_key_is_none_never_a_fabricated_verdict`** — ausente → `None`; `null` → `None`; y se asserta que `None` **no es** `reject` ni `accept`, para que un default accidental en el struct real no pase inadvertido.
4. **`ac3_absent_or_malformed_keys_never_fabricate_a_verdict`** — el que **falló al escribirlo, y el fallo fue un hallazgo**.

### 4.1 El hallazgo: malformado NO es `None`, es `Err`

Supuse que `#[serde(default)]` cubría también el tipo equivocado. **No lo cubre, y el test me lo demostró** (`panicked at n11_sharedrs_01.rs:113`). Medido:

| caso | resultado REAL |
|---|---|
| clave **ausente** | `None` ✓ |
| `null` | `None` ✓ |
| clave presente, **tipo equivocado** (`123`, `true`, `{…}`) | **`Err`** — no `None` |

**El lector se NIEGA en vez de fabricar** — que es la dirección correcta — pero **no es lo mismo que `None`**, y la diferencia tiene consecuencia: en `sim-ctl`, un `Err` de `serde_json::from_str::<Opportunity>` cae en `invalid_msg_parse`, **se ACKea y se descarta el mensaje ENTERO**.

**⇒ Entrada de diseño para el cambio a `shared-rs`**: si un `verdict` mal tipado no debe tumbar la oportunidad completa, el campo necesita `#[serde(default, deserialize_with = …)]` (o un `serde_json::Value` intermedio) en vez de un `Option<String>` pelado. **Se mide y se declara; no se disimula.** El test fija las dos direcciones y la tabla de arriba queda como criterio de aceptación del cambio real.

> Nota de alcance: el test usa un **espejo local** (`VerdictMirror`) con los mismos atributos, porque **`Opportunity` no puede declarar el campo desde este inScope**. El espejo es la **especificación ejecutable** que el cambio a `shared-rs` tiene que satisfacer.

---

## 5. AC4 — ES ADITIVO: **ningún consumidor depende del conjunto exacto de claves**

Verificado leyendo el código, no por frase:

| consumidor | veredicto |
|---|---|
| `deny_unknown_fields` en el workspace | **3 usos, y NINGUNO sobre `Opportunity`**: `ExecutorFlashQuote` (`flashloan_math.rs:51`), `Feed` y `ChainFeeds` (`oracle_snapshot.rs:12,19`) |
| `relays-client` (deserializa `Opportunity` del stream) | **seguro**: usa el mismo struct; con `#[serde(default)]` el campo nuevo es opcional |
| `shared-ts` `OpportunityListItemSchema` (`api-contracts.ts:248`) | **NO es `.strict()`** (zod descarta claves desconocidas por defecto) |
| `frontend` `OpportunityItemSchema` (`runtime.ts:55`) | **ES `.strict()`** — **pero tiene CERO consumidores**: `git grep OpportunityItemSchema -- frontend/` sólo devuelve su definición (L55) y su tipo (L82). No valida nada |
| el endpoint `/opportunities` | se construye por **proyección SQL** (`o.amount_in_wei::text AS amount_in_wei`), **no** serializando el struct |

**⇒ Ningún consumidor se rompería por las claves nuevas.** Lo único que rompe es la **construcción** en Rust (§2), que es un asunto distinto y está fuera de alcance.

---

## 6. AC6 — EL LADO TS, y el impacto en la Dapp

**El contrato TS NO declara esas claves.** Grep del **literal** de la clave (no de la palabra — que sí aparece en prosa: *"a verdict is just produced earlier"* en `tokenlists/index.ts`):

```
git grep -n '"verdict"\|verdict:\|verdict_reason' origin/main -- shared-ts/   → exit 1
git grep -rn 'verdict_reason' origin/main -- frontend/                        → exit 1
```

**⇒ Rust declara lo que el TS no.** Y `OpportunityListItem` **no es un espejo** del struct Rust: tiene `token_in_info`, `leg_symbols`, `status`, `simulated_*` — es una forma **derivada del API**, no la serialización del struct. **NO se tocó `shared-ts/`** (fuera de alcance).

**Impacto en la Dapp: NINGUNO.** El cambio no se aplicó (§2), y aunque se aplicara, la Dapp lee del API (proyección SQL), no del struct serializado. La única serialización de `Opportunity` es el `XADD` a `arbx:opps:simulated` (`sim-ctl/src/consumer.rs:980`) y su consumidor es `relays-client` (Rust) — que es tolerante por construcción.

---

## 7. LAS PUERTAS

| puerta | resultado | evidencia |
|---|---|---|
| `command -v cargo` | **PASS** | `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` |
| `cargo fmt --all -- --check` | **PASS** | sin diff (el primer intento **falló** y se corrigió formateando en WSL + copiando de vuelta con md5 re-verificado) |
| `RUSTFLAGS='-D warnings' cargo check -p shared-rs` | **PASS** | `grep -cE '^(error\|warning)'` = **0** |
| `cargo test -p shared-rs --no-fail-fast` | **PASS** | `284 passed; 0 failed` · `13 passed` · `1 passed` |
| `cargo test -p sim-ctl --no-fail-fast` | **PASS** | el binario nuevo `tests/n11_sharedrs_01.rs` → **4 passed; 0 failed**; el resto 0 fallos |
| **AC5: consumidores** | **PASS** | `cargo check --workspace` → **0 errores** (con el struct revertido) |

---

## 8. LO QUE ESTE ARTEFACTO NO DICE

- **NO dice que el arreglo esté hecho.** No lo está: **está bloqueado por alcance**, medido y con la ruta de desbloqueo en §2.
- **NO dice que el descarte tenga impacto en un veredicto.** Con el 100% de la ventana en `reject`, declarar el campo **no cambia qué se simula** — sólo hace visible lo que hoy se pierde.
- **NO reporta ninguna medición de producción nueva.** No se desplegó nada y no se tocó producción.
