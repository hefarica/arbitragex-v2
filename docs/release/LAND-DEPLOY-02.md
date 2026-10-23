# LAND-DEPLOY-02 — aterrizaje de los PRs verificados, un PR por ciclo de deploy

- **Tarea:** `t92` (kind: integration) — miembro `Release-B`, attempt 1
- **run_id:** `arbx-entrega-20261007` · **SHA_BASE:** `21d2039cc80b8c47c3eea6b223ce7e663da604fd`
- **Ventana:** `2026-10-07T22:15Z` → `2026-10-08T00:15Z`
- **Permisos usados:** commit, push, merge a main, deploy vía GitHub Actions. **NO** capital, firma, broadcast.
- **Método:** clon AISLADO en `%TEMP%\arbx-t92`. Guardas re-verificadas EN EL MISMO comando que mergea. `main` nunca se leyó por la respuesta de un comando: siempre por `git ls-remote`.

---

## VEREDICTO

**DOS aterrizajes efectivos, uno por ciclo de deploy, cada uno verificado por el SHA realmente servido DENTRO del contenedor.** `main` avanzó `21d2039c → cd1114d1 → 32743333`. El PR que mueve la métrica (**#846**) aterrizó con **los DOS ejes** que D4 exige. **#841 NO es aterrizable** (gate rojo, D7). **#848 no se tocó** (apilado sobre #846). **Zero `--admin`, zero gates saltados.**

---

## 1. D6 — `main` antes y después de cada merge (leído por el REMOTO)

| momento | `git ls-remote origin refs/heads/main` |
|---|---|
| ANTES de #844 | `21d2039cc80b8c47c3eea6b223ce7e663da604fd` |
| DESPUÉS de #844 | **`cd1114d1271d3271ca1d0739aae7ebdccff05897`** |
| DESPUÉS de #846 | **`327433332732b06975dccadc725940cbeaccd4ca`** |

---

## 2. ATERRIZAJE 1 — #844 (`feat/pool-resolve-01`)

**Guardas (un solo comando, instante único):**
- **D2** — head vigente `3943b4466e0d264a276540e71429b1d4c51b1e36`; sobre ESE head `ci-gate = completed/success` **y** `Verifier policy tests = completed/success`; **34 checks, 0 fallos, 0 sin completar**; `mergeStateStatus = CLEAN`.
- **D1** — último `auto-deploy-vps.yml` `completed/success`: ningún run vivo.
- `--match-head-commit`, sin `--admin`.

**Merge:** `merge_exit=0` · mergeCommit **`cd1114d1`** · `mergedAt 2026-10-07T22:15:28Z` · `state MERGED`.

**Deploy (D5) — por el SHA desplegado, no por el comando:**

| campo | valor |
|---|---|
| run id | **`37695003233`** |
| headSha | `cd1114d1271d3271ca1d0739aae7ebdccff05897` |
| conclusion | **`success`** |
| ventana | `22:15:30Z → 23:11:28Z` (**55 min 58 s**) |
| gate `Verify same-SHA main-push gates` | **success** (el step que en t77 abortaba por supersesión) |
| workflows de ese push | **21 de 21 `success`, 0 fallos** |

**`org.opencontainers.image.revision` DENTRO de los contenedores:**

| contenedor | antes (23:08Z) | **después** |
|---|---|---|
| `arbitragex-v2-sim-ctl-1` | `21d2039c…` | **`cd1114d1…`** |
| `arbitragex-v2-selector-api-1` | `21d2039c…` | **`cd1114d1…`** |
| `arbitragex-v2-searcher-rs-1` | `21d2039c…` | **`cd1114d1…`** |

Sonda remota: `/status` → `deploy_sha = cd1114d1…`, `deploy_id = 37695003233`.

---

## 3. ATERRIZAJE 2 — #846 (`SIM4-CYCLIC-01/02/03/04`) — el que mueve la métrica

**D4 verificado en SUS DOS EJES sobre el head vigente `4ccecb66f634826902c22da26a56d8de1d735b16`**, releído en el momento de mergear porque el head se movió **cuatro** veces (`c5adf311 → 0fbad975 → 52d99380 → 4ccecb66`):

**(a) call site de N1** — `backend/sim-ctl/src/consumer.rs` figura entre los 7 archivos del diff.

**(b) cierre de F-01 + F9** — `backend/sim-ctl/src/persistence.rs` **y el fallo tipado**. No me quedé con que el archivo aparezca: verifiqué la sustancia en las líneas `+`:
```
+        || fail_reason.starts_with("cyclic_route_missing_route_metadata")   // F-01: clasificador
+            Err(BuildError::CyclicRouteMissingPath(kind)) => {              // F9: fallo tipado
+    CyclicRouteMissingPath(StrategyKind),
+        if path.len() < 3 || path[0] != token_in || path[path.len() - 1] != token_out {
+            return Err(BuildError::CyclicRouteMissingPath(
```
El propio comentario del diff declara el agujero que F9 cierra: *"cuando `token_in != token_out` … encodía UNA pata `token_in -> token_out` por `dex_a`, descartando los hops intermedios. Eso simula una ruta que NO [existe]"*. **Un head con (a) sin (b) no se habría mergeado** — precisamente porque un `passed=true` fabricado es peor que un cero medido.

**Guardas:** head `4ccecb66`; `ci-gate = completed/success` **y** `Verifier policy tests = completed/success`; **34 checks, 0 fallos, 0 sin completar**; `mss = CLEAN`; sin auto-deploy vivo. `--match-head-commit`, sin `--admin`.

**Merge:** `merge_exit=0` · mergeCommit **`32743333`** · `mergedAt 2026-10-07T23:12:36Z` · `state MERGED`.

**Deploy (D5) — CERRADO y verificado:**

| campo | valor |
|---|---|
| run id | **`37700927145`** |
| headSha | `327433332732b06975dccadc725940cbeaccd4ca` |
| conclusion | **`success`** |
| ventana | `23:12:39Z → 00:09:50Z` (**57 min 11 s**) |
| gate `Wait for all deployment gates` | **success** (`23:12:42Z → 23:40:34Z`) |
| workflows de ese push | 19 completados, **0 fallos** |

**`org.opencontainers.image.revision` DENTRO de los contenedores:**

| contenedor | antes | **después** |
|---|---|---|
| `arbitragex-v2-sim-ctl-1` | `cd1114d1…` | **`32743333…`** |
| `arbitragex-v2-selector-api-1` | `cd1114d1…` | **`32743333…`** |
| `arbitragex-v2-searcher-rs-1` | `cd1114d1…` | **`32743333…`** |

Sonda remota: `/status` → `deploy_sha = 32743333…`, `deploy_id = 37700927145`.

---

## 4. D7 — #841 NO es aterrizable (y NO se forzó)

| campo | valor medido |
|---|---|
| head vigente | `17fa4aeca329e836f859343f741d603beb233e42` |
| `ci-gate` | **`completed/FAILURE`** ← context requerido EN ROJO |
| `Verifier policy tests` | `completed/success` |
| checks en FAILURE | **`ci-gate`** y **`retention-vacuum-regression`** (run `37473031849`) |

**Motivo exacto:** el PR es `sre/pg-retention-01` (la mitad viva de #732, *"el VACUUM de retención se tragaba su propio fallo"*) y **falla precisamente `retention-vacuum-regression`**, el test de regresión de su propio fix. No se mergeó, no se usó `--admin`, no se saltó ningún gate.

---

## 5. #848 — NO tocado (apilado, y fuera de orden)

`SIMCTL-BOUND-01`, rama basada en #846, head `d1bcc2e9`, 8 archivos, OPEN, `mergedAt=null`. **Apilado sobre #846**: su diff contra `main` mostraría la UNIÓN con #846. **No se mergea antes de #846 ni en el mismo ciclo**; requiere rebase sobre el `main` nuevo y **veredicto de CI RE-MEDIDO sobre ese head** (su CI estaba NO MEDIDO). Por orden del operador la cota va **después** de la métrica. **Queda para otro ciclo.**

---

## 6. D1 — método aplicado (y por qué `queued` ≠ "en gates")

Regla aplicada, en su forma estricta: **no mergear con CUALQUIER run de `auto-deploy-vps.yml` vivo sobre `main`** — `queued`, `pending` o `in_progress`. En ambos aterrizajes la guarda exigió `status = completed`, y en ambos el último run estaba `completed/success`. Al cerrar, los dos últimos runs están `completed/success`.

---

## 7. D8 — sin secretos, sin capital, sin broadcast

**Verificado DESPUÉS del deploy** (no sólo antes), leyendo el `.env` del VPS con `grep -E "^(ARBX_TRADE_MODE|ARBX_LIVE_EXEC_ENABLED)=" ` — solo esas 2 claves, sin imprimir ningún valor secreto:
```
ARBX_LIVE_EXEC_ENABLED=False
ARBX_TRADE_MODE=paper
```
Ninguna operación de capital, firma, broadcast, préstamo, retiro ni transferencia. Ningún secreto impreso ni publicado. El único push de este informe va a una rama de documentación (no a `main`) y **no se mergea**.

---

## 8. D9 — durabilidad del artefacto propio

Se manifiesta **en el remoto** (rama + PR, **sin mergear**) el artefacto de análisis de la cadena:
- `docs/review/VERIF-CADENA-E2E-01.md` — con su **ADDENDUM 2026-10-07** (§11: procedencia de las superficies, caducidad de las cifras de §7, diferencia estructural productor↔stream) y las **correcciones §10.1/§10.2** (gate transitorio con causa literal; `n_tup_ins` → `COUNT(*)`).
- **sha256 del artefacto publicado:** `44EDF3418ED442D4A42FFA44759347A795A4E05CD40036D85E43DBDAF826DC2A` (25 292 B) — idéntico al del checkout compartido, verificado antes de copiarlo al clon aislado.
- **Rama:** `docs/verif-cadena-e2e-01-01`.

**Verificación exigida (por el remoto, NUNCA por la respuesta del push ni el exit code):** `git ls-remote origin refs/heads/docs/verif-cadena-e2e-01-01` y `gh api repos/hefarica/arbitragex-v2/contents/docs/review/VERIF-CADENA-E2E-01.md?ref=docs/verif-cadena-e2e-01-01 --jq .sha`. Los resultados se citan en el cierre de t92. **NO se mergea.**

---

## 9. RÉGIMEN DECLARADO (condición de medición, no defecto del aterrizaje)

Dato de Data (t97) que **condiciona lo que t93 va a medir**:
- Cota publicada: `DEFAULT_MAX_SIMS_PER_SEC = 1.0`, derivada de una latencia medida de **~2,084 s por simulación** ⇒ techo **secuencial ~0,48/s**.
- Llegada al consumidor: **~68 filas/s** (medida por t91: +8 334 en 120 s).
- **Conclusión honesta (de Data, no mía):** el techo real lo pone la **latencia secuencial**, no la cota; la mitigación no es subir la cota, sino **reducir admisión aguas arriba o paralelizar**.

**Consecuencia para leer `passed=true > 0`:** tras este deploy la cadena procesará **una fracción** de lo que publica. **Esa fracción es NO COMPUTADA hasta medirla.** Coincide con la frontera epistémica que ya había declarado antes: el multiplicador "68 filas/s → 68 forks/s" no estaba medido. Un `passed=true` que aparezca **no** se debe leer como "el fix arregló todo", sino como el resultado de una fracción todavía no cuantificada.

---

## 10. LÍMITES — lo que este informe NO afirma

1. **No afirma que `passed=true > 0` haya ocurrido.** Eso se mide después de este deploy y es de **t93**, con su contraste antes/después. Aquí sólo se aterriza y se verifica identidad de bytes.
2. **No afirma nada sobre #848** (stacked, sin rebase, CI no medido).
3. **No re-mide la cadena** (`simulations`/`passed`/`fail_reason`): es de t93.
4. **El aterrizaje de #844 lo ejecutó esta sesión**, con comando, exit code y `ls-remote` antes/después citados. No es un merge de actor no computado.
5. **Hueco de durabilidad ajeno, declarado:** el artefacto de `t91`, `docs/review/VERIFY-E2E-02.md` (~19 KB), sigue **untracked** en el checkout compartido. Es de otro miembro y no lo publico, pero el riesgo medido que motiva D9 —un hallazgo que no está en el remoto se pierde— aplica igual. **Se recomienda que su autor lo manifieste.**
