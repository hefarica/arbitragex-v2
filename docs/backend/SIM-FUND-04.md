# SIM-FUND-04 — Instrumento NEUTRO: registrar los hechos, no dictaminar la causa

**run_id** arbx-entrega-20261007 · **phase_id** implementation · **SHA_BASE** `80e2f86c` (main medido)
**Rama** `fix/sim-fund-04` (PR propio, SIN merge) · **dueño** Backend
**Alcance** `backend/sim-ctl/src/signer_funding.rs` (1 archivo) · **NO se tocó** `consumer.rs` (X6), `docker/`, `monitoring/`, `sim-core/`
**Prohibiciones respetadas**: sin merge, sin deploy, sin tocar main, sin reiniciar servicios.

---

## 0. INSTRUMENTO

`command -v cargo` → `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` · `GATE1=PASS`.
md5 idéntico WSL↔Windows **antes** de correr: `signer_funding.rs 0d808aab2becff14c373408443546458` en ambos lados.

---

## 1. X1 — QUÉ REGISTRA EL INSTRUMENTO, Y QUÉ SE NIEGA A HACER

Un `warn!` estructurado por **intento de fondeo fallido**, con el evento `sim.funding_probe`, y un JSON con:

| campo | qué es |
|---|---|
| `token`, `signer`, `amount_in` | las direcciones y el monto del intento |
| `candidate_slots` | la lista **completa** probada |
| `probes[]` | una entrada **por slot probado** |
| `probes[].set_storage_at` | resultado literal de `anvil_setStorageAt`: `"true"` / `"false"` |
| `probes[].storage_readback` | `eth_getStorageAt` **después** de escribir (o `read_failed`) |
| `probes[].balance_of` | `balanceOf(signer)` por `eth_call` en ese instante (o `unreadable`) |
| `probes[].signer_keyed_key` | la clave **efectivamente usada** |
| `probes[].token_keyed_key` | la MISMA derivación pero keyeada por el token |
| `probes[].readback_matches_sentinel` | igualdad cruda entre lo escrito y lo leído |
| `all_probes_used_signer_key` | X2 (§3) |

**NO dictamina.** No emite `write_invisible`, `write_not_persisted` ni ningún equivalente interpretativo. **Y eso está asertado sobre la salida real**, no sobre la intención:

```rust
for forbidden in ["write_invisible", "write_not_persisted"] {
    assert!(!s.contains(forbidden), "el instrumento registra, no dictamina: {forbidden} no debe aparecer");
}
```

**R9 (volumen)**: un registro por intento, sin muestrear. Muestrear perdería justo la combinación token/slot que se busca. El volumen es del orden de **~10-11 líneas/min** (el consumidor admite ~11 simulaciones/min), muy por debajo del flooding que motivó R9 (**183 líneas/s**). Se declara el orden de magnitud en vez de asumirlo.

---

## 2. POR QUÉ EL INSTRUMENTO REGISTRA **DOS** DERIVACIONES

El error de t111 no fue un valor de hash equivocado: fue **elegir la clave equivocada** sin que nada lo mostrara. Un slot derivado con la clave equivocada produce un hash de 32 bytes **perfectamente plausible** y su síntoma —`eth_getStorageAt` muestra la escritura, `balanceOf` no cambia— es **indistinguible** del de una escritura que no llega a la ejecución.

Registrando **ambas** columnas, una clave equivocada deja de ser invisible: se ve en el log qué clave se usó y cuál habría dado la otra elección. **Es la instrumentación que habría cazado t111 en la primera lectura.**

---

## 3. X2 — LA PREGUNTA QUE MANDA

`all_probes_used_signer_key` se **recalcula sobre los hechos registrados**, no se asume:

```rust
probe.all_probes_used_signer_key = probe
    .probes
    .iter()
    .all(|p| p.signer_keyed_key == format!("{:?}", balance_slot(p.slot, signer)));
```

**El motivo por el que esto manda**: t111 afirmó que los tokens que fallan son los canónicos, con slots 3/9/2 dentro de `CANDIDATE_SLOTS` — y esa afirmación se computó con una función keyeada por el **TOKEN**. **Queda NULA.** Nadie sabe todavía si el slot real de los tokens que fallan en producción está en `[u64; 4] = [0,2,3,9]`. **El instrumento es lo que permite contestarlo con la próxima lectura de producción.**

### 3.1 El control negativo que faltaba (lo único que se conserva de #859)

```rust
assert_eq!(balance_slot(3, signer), 0x961558ef…);          // valor MEDIDO en el fork vivo
assert_ne!(balance_slot(3, signer), balance_slot(3, token)); // la clave es el SIGNER
assert_eq!(token_keyed.as_bytes().len(), 32, "un slot equivocado sigue pareciendo valido");
```

---

## 4. X3 — LA INSTRUMENTACIÓN **NO** CAMBIA EL COMPORTAMIENTO

- El `?` de `write_balance` en el gate de escritura **queda exactamente igual**: si falla, propaga con su propio mensaje como antes.
- El `continue` por `write_balance == Ok(false)` sigue contando `WRITE_REJECTED` y saltando al siguiente slot.
- La verificación sigue siendo `verified == Some(SENTINEL_BALANCE)` **por `eth_call`** — **no** por `eth_getStorageAt`. Una escritura visible al storage **no** prueba que la ejecución la vea, y al revés.
- **`return Ok(())` sigue exigiendo verificación real.** No se fabrica nada.
- Las únicas llamadas nuevas al nodo son **lecturas** (`eth_getStorageAt`), y el `StorageProbe` no participa en ninguna decisión.
- Ningún umbral tocado; ningún número mejorado.

---

## 5. X4 — RELACIÓN CON #859 (Y CON #860)

**#859 (`fix/sim-fund-02c`, ABIERTO, `mergedAt=null`) NO DEBE MERGEARSE.** Se construyó sobre la premisa de t111 que t112 **retiró**: emite `write_invisible` / `write_not_persisted`, etiquetas que dictaminan la causa. **Etiquetaría como "la EVM no lo ve" lo que en realidad es "escribiste en un slot que no es"** — un instrumento que miente.

**#860 (`fix/sim-fund-03`, ABIERTO, `mergedAt=null`)** retracta t111 y aporta el mismo control negativo de X2.

**ESTA TAREA (#861) REEMPLAZA A #859.** Lo único que se conserva de #859 es **el control negativo de X2** (§3.1), declarado como tal. **Para que ningún release lo recoja: de los tres, el que debe mergearse es este; #859 debe CERRARSE sin merge.**

**#860 y #861 comparten el test de X2.** Si se mergean ambos habrá un conflicto trivial de duplicación; el capitán debe mergear **uno** y cerrar el otro. Se declara en vez de dejar que se descubra en el merge.

---

## 6. X5 — LAS 4 PUERTAS

| puerta | resultado | evidencia |
|---|---|---|
| `command -v cargo` | **PASS** | `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` |
| `cargo fmt --all -- --check` | **PASS** | sin diff |
| `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` | **PASS** | `Checking sim-ctl v0.1.0` + `Finished … in 15.95s`; `grep -cE '^(error\|warning)'` = **0** |
| `cargo test -p sim-ctl --no-fail-fast` | **PASS** | 0 fallos; tests nuevos por nombre: `x2_balance_slot_is_keyed_by_the_signer_and_not_by_the_token` y `simfund04_probe_records_raw_facts_and_both_key_derivations` |
| `cargo test -p sim-core --no-fail-fast` | **PASS** | `79 passed; 0 failed` |

`Compiling` **O** `Checking` exigidos y presentes. md5 idéntico WSL↔Windows **antes** de correr. Exit code real por puerta.

---

## 7. X6 — PR PROPIO, Y **LA INSTRUMENTACIÓN SOLA NO PRODUCE VEREDICTO**

**Sin adornos: instrumentar no contesta la pregunta.** Este cambio **no arregla el fondeo, no mueve `passed=true`, y no determina la causa raíz.** Lo que hace es que **la próxima lectura de producción conteste qué combinación falla y qué se leyó** — cosa que hoy es imposible porque nadie registraba el token, el slot ni el read-back del intento fallido.

**Hace falta**: desplegar esta instrumentación (paso APARTE, y el deploy está prohibido en esta tarea) y leer los intentos reales. Hasta entonces la causa raíz sigue **NO DETERMINADA**.

`grep -c 'passed=.true.'` sobre la evidencia viva de métricas → **0**, y el cero **tiene productor probado** (`{passed="false",simulator="anvil"} 661`, `{simulator="revm"} 1102`): la familia se emite, solo falta `passed=true`.

---

## 8. X7 — LO QUE EL INSTRUMENTO **NO** DEBE HACER, Y CÓMO SE EVITA FORZAR LA HIPÓTESIS

**X7 pide que si aparece que el slot correcto YA estaba entre los probados y SÍ reprodujo, se diga — y que no se fuerce la hipótesis del slot.**

Por construcción, `balance_slot(slot, signer)` **siempre** keyea por el signer, así que el slot «correcto por construcción» **está** entre los probados por definición. Lo que **no** se sabe —y es lo que el instrumento va a exhibir— es **si el ÍNDICE de ese slot es el que el token realmente usa**. Es decir: la hipótesis viva **no** es "la clave está mal" (eso lo cierra §3.1), sino **"el índice `[0,2,3,9]` no cubre el layout del token que falla"** — o, si el read-back mostrara que el slot correcto **sí** reprodujo, entonces la falla está en **otro lado** (el signer concreto, el token concreto, o el momento), y **el registro lo va a decir sin que haya que forzarla**.

**No se elige ninguna de las dos.** El instrumento está diseñado para que la lectura discrimine: `readback_matches_sentinel` y `balance_of` por slot, más las dos derivaciones.
