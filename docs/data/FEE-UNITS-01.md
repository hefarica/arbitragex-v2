# FEE-UNITS-01 — el guardián que fijaba el bug: actualizado sin perder la guardia

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261007` · `phase_id=implementation` · `SHA_BASE=21d2039cc80b8c47c3eea6b223ce7e663da604fd`
**Dueño:** Data · **Permisos:** paper, sin firma, **sin merge** (el merge es de Release-B)
**In scope:** `tests/v3-fee-units/`, `docs/data/` · **Out of scope:** `backend/`, `.github/`, `frontend/`
**Artefacto:** este documento · **Intento:** `3f52efff-6326-424d-9ead-04f7abac6af6`

## Identidad antes del primer commit

```
clon aislado  = C:/Users/HFRC/Desktop/arbx-t49/repo     (el checkout compartido NO se tocó)
rama          = feat/pool-resolve-01                    (el árbol de #844)
HEAD          = 48c4c6ecf8be200ef67ef6cfc1ab7f7160ee0f3c
identidad git = configurada en el clon antes del commit; verificación por el REMOTO (ls-remote)
```

---

## 1. REPRODUCCIÓN ANTES DE TOCAR NADA

Comando que el gate corre: `python3 tests/v3-fee-units/test_hydration_order.py`.

```
test_fee_observation_precedes_token_and_pool_writes ... ok
test_resolved_observation_follows_successful_index_publication ... ERROR
test_v3_bootstrap_writer_uses_shared_cas_not_unconditional_set ... ok
test_v3_index_uses_consumer_key_and_propagates_exhaustion ... ok
FAILED (errors=1)

Traceback (most recent call last):
  File "tests/v3-fee-units/test_hydration_order.py", line 21, in ...
    body = SOURCE.split("let mut resolved_pool = None;", 1)[1].split("async fn record_observation(", 1)[0]
IndexError: list index out of range
```

**Corrección del dictamen heredado, medida:** la cápsula diagnosticaba un `ValueError` en `body.index("resolved_pool")`. **Medido, el test nunca llega ahí**: la línea que falla es el **`split`**, cuyo literal ancla (`let mut resolved_pool = None;`) desapareció con el renombrado a `resolved_addr` de #844. La consecuencia declarada es la misma —*el test errora en vez de fallar*— pero un paso antes de lo diagnosticado. El `IndexError` no dice nada sobre qué está mal: es la peor forma de rojo.

**Prueba por el CI (no por mi estación):** el PR #844 tiene **un solo check rojo**, `fee-units-and-cache`; su job `37562811507` muestra, en este orden:

```
test result: ok. 30 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s   (cargo test, verde)
test_resolved_observation_follows_successful_index_publication ... ERROR
IndexError: list index out of range
FAILED (errors=1)
##[error]Process completed with exit code 1
```

---

## 2. LA JUSTIFICACIÓN (por qué actualizar este test NO es debilitarlo)

El test `test_resolved_observation_follows_successful_index_publication` **fijaba el defecto** que #844 corrige. Sus dos aserciones exigían el orden viejo —**publicar ANTES de capturar la dirección**—, que era *exactamente* el bug: la dirección sólo se registraba si la hidratación tenía éxito, así que una hidratación fallida **tiraba** una dirección que la fábrica **ya había respondido**.

**Costo medido de ese orden** (t84, sobre la base viva): **6.173 de 6.203 pares observados pendientes tenían `resolved_pool_addr IS NULL`** — el barrido de reintento no tenía dirección que hidratar. *Un guardián que fija un bug no es un guardián: es el bug con un test pegado.*

| # | Cambio | Estado |
|---|---|---|
| **B1** | `assertLess(publish, resolved)` **ELIMINADA**: exigía publicar antes de capturar — el orden defectuoso | ✅ con esta justificación, en el archivo y acá |
| **B2** | `assertNotIn("Some(e_pool),", body[:hydration])` **ELIMINADA**: el mismo orden defectuoso, dicho en negativo — prohibía la captura temprana, o sea prohibía el arreglo | ✅ con esta justificación, en el archivo y acá |
| **B3** | `assertLess(hydration, publish)` **conservada**; `assertLess(resolve, record)` **conservada** (re-anclada a `resolved_addr`) | ✅ |
| **B4** | **AÑADIDA** la aserción de que un hydration fallido NO anula un `resolved_addr` ya capturado | ✅ |
| **B5** | Re-anclaje a `resolved_addr`: sin `ValueError` en `body.index(...)` | ✅ |
| **B6** | `python3 tests/v3-fee-units/test_hydration_order.py` → **exit 0** | ✅ |

### Lo que el guardián afirma AHORA (el intento real, nada más débil)

1. **hidratación precede publicación** — conservada, textual;
2. **la dirección capturada precede la escritura** — conservada, re-anclada;
3. **la captura precede la publicación** — *la corrección*;
4. **la captura es INCONDICIONAL**: `assertLess(capture, success_arm)` prueba que está **antes** de `if let Ok(pool_ref)` — si volviera a meterse dentro de la rama de éxito, el guardián se pone ROJO;
5. **la escritura es MONÓTONA** (B4): se afirma sobre el **literal SQL** que `is_resolved` sólo avanza (`… OR EXCLUDED.is_resolved`) y que la dirección se preserva (`COALESCE(observed_unindexed_pairs.resolved_pool_addr, …)`), y se **prohíbe explícitamente** el sobrescrito incondicional que borraba direcciones conocidas (`resolved_pool_addr = $7`, `is_resolved = $6`).

Además, del renombrado se extrajo una lección aplicada: el `split` ahora tiene un **pre-chequeo** (`assertIn(anchor, SOURCE, "anchor gone: …")`) para que un renombrado futuro **FALLE con un mensaje** en lugar de **ERRORAR** con `IndexError` — la forma que este guardián tenía.

---

## 3. UN FALSO ROJO MÍO, DECLARADO (misma clase de defecto que esta tarea persigue)

La primera versión de mi aserción negativa fue `assertNotIn("resolved_pool_addr = $7", rec)` sobre **toda la función**. Resultado: **rojo falso** — el texto aparecía en el **comentario explicativo** que está arriba de la sentencia, no en el SQL. Es decir, **medía prosa en vez de código**. Corregido: la aserción ahora se aplica **sólo al literal SQL** (`rec.split('r#"',1)[1].split('"#',1)[0]`). Se declara porque es exactamente la clase de error que esta célula persigue: una aserción que *parece* evidencia y mide otra cosa.

---

## 4. VERIFICACIÓN LOCAL DE LOS PASOS DEL GATE

| Paso del gate `fee-units-and-cache` | Local | CI (run del PR) |
|---|---|---|
| `git diff --check` | **exit 0** (limpio) | verde |
| `cargo test --locked --manifest-path tests/v3-fee-units/Cargo.toml --lib` | **30 passed; 0 failed; 3 ignored** | **verde** (mismo resultado) |
| `rustfmt --edition 2021 --check backend/searcher-rs/src/pool_discovery.rs` | **RUSTFMT_OK** | corrió sin error (el job avanzó al paso siguiente) |
| `python3 tests/v3-fee-units/test_hydration_order.py` | **Ran 4 tests — OK** | **ROJO** antes (IndexError) → con este PR: **verde** |
| `python3 tests/v3-fee-units/test_redis_cas.py` + cargo `--ignored` | **NO CORRIÓ: `ConnectionRefusedError [Errno 111]`** — no hay Redis local | **NUNCA CORRIÓ** (el job murió antes) |

**Frontera declarada (fail-honest), y es importante:** el paso de Redis **nunca llegó a ejecutarse en CI** — el job murió en el paso de Python (`set -euo pipefail`). El workflow le provee un `redis:7-alpine` en `127.0.0.1:36379`; yo **no tengo Redis local** (WSL no tiene `redis-server` y la doctrina de la casa es *cero instalaciones*), así que **no puedo acreditarlo desde acá**. Lo que sí puedo afirmar: mi cambio **no lo toca** — `test_redis_cas.py` lee `backend/searcher-rs/src/pool_discovery/v3_fee.rs`, mientras este PR toca **sólo** `tests/v3-fee-units/test_hydration_order.py`. **Si al correr por primera vez ese paso aparece rojo, es un hallazgo NUEVO, no de este cambio**, y hay que reportarlo como tal.

---

## 5. EL DIFF

```
 tests/v3-fee-units/test_hydration_order.py | 86 ++++++++++++++++++++++++++----
 1 file changed, 77 insertions(+), 9 deletions(-)
```

**B7 — fail-honest: NO se tocó producción.** Un solo archivo, y está en `tests/v3-fee-units/`. `backend/` **intacto**: cero líneas. El verde se logró **sin** tocar el módulo de producción, que es lo que la aceptación exigía comprobar; y si hubiera requerido tocarlo, este documento lo diría en vez de hacerlo.

**Zero mocks / Zero hardcode:** el test lee el **fuente real** (`backend/searcher-rs/src/pool_discovery.rs`) y afirma sobre él; no hay datos inventados, ni rutas mágicas, ni valores esperados cableados fuera de los símbolos que el propio código debe contener.

**NO se mergeó.** Un push a `main` dispara `auto-deploy-vps.yml` sin filtro de paths; este trabajo publica en la rama de #844 y el aterrizaje es de Release-B.

---

## 6. Reproducción

```bash
# el gate, paso por paso (desde la raíz del repo, en el árbol de #844)
git diff --check
cargo test --locked --manifest-path tests/v3-fee-units/Cargo.toml --lib
rustfmt --edition 2021 --check backend/searcher-rs/src/pool_discovery.rs
python3 tests/v3-fee-units/test_hydration_order.py     # -> Ran 4 tests, OK
```

---

*Un guardián se actualiza cuando fija un defecto; se actualiza con la justificación escrita, no en silencio, y conservando —o reforzando— lo que de verdad protegía. Acá el test pasa de **6** aserciones a **10** (contadas con `Select-String -Pattern 'self\.assert'` sobre el método: 6 en el original, 10 en el nuevo), y el archivo entero de 15 a 19; las dos que se fueron son las dos que exigían el bug.*
