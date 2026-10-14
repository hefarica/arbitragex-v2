# RUNBOOK-VALIDACION-01 — ¿sigue siendo ejecutable el runbook de activación mainnet (2026-09-17)?

**Veredicto global: el runbook NO es ejecutable tal cual.** Su **doctrina sigue vigente y sus citas de
código son exactas**, pero contiene **1 referencia que no resuelve**, **1 paso cuya condición ya cambió** y
**1 comando que apunta a un archivo que el deploy real no usa**.

| | Valor |
|---|---|
| Objeto validado | `audits/live-activation-package-20260917/RUNBOOK-ACTIVACION-MAINNET.md` (53 líneas, 3099 b) |
| Emitido | 2026-09-17 |
| `origin/main` al validar | **`c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`** |
| Cómo se leyó | desde **`origin/main`**, no del disco (el checkout compartido está 174 commits atrás; se verificó igualmente que el archivo del disco tiene el mismo blob `2122fe1cce7e`) |
| Pasos ejecutados | **NINGUNO** — esto es una validación de vigencia, no una corrida |

---

## 1. Paso por paso: veredicto y evidencia

### Fase 0 — Prerrequisitos

| # | Paso del runbook | Veredicto | Evidencia medida |
|---|---|---|---|
| 0.1 | Cerrar cobertura V3 (`fix/v3-slot0-coverage-20260917` **mergeado + deployado**) | **REFERENCIA NO RESUELVE** | `git ls-remote origin refs/heads/fix/v3-slot0-coverage-20260917` → **vacío** (exit 0, 0 líneas). La rama **no existe en el remoto**. Ver §3. |
| 0.1b | Verificación: ≥1 `simulations.passed=true` en PG | **NO COMPUTADO** | §4: la herramienta SQL disponible devuelve `ok:true` con `data` **vacío** incluso para un `SELECT COUNT(*)` trivial ⇒ no produjo medición. |
| 0.2 | Primer ciclo real en Sepolia (tx hash + receipt chainId 11155111) | **NO COMPUTADO** | misma razón: `executions` no se pudo medir. |
| 0.3 | "Deploy veraz hasta `dcfe890c`+ (**hoy VPS=a06a968d**)" · "❌ 3 commits sin deploy" | **OBSOLETO — la condición cambió** | Los **4** commits citados resuelven y son **ancestros de `main`**: `fdb40401`→`fdb404018508`, `125b1e0b`→`125b1e0b4b34`, `dcfe890c`→`dcfe890cac94`, `a06a968d`→`a06a968d044a` (`git cat-file -e` exit 0 en los 4; `git merge-base --is-ancestor` → en main). La deuda "3 commits sin deploy" **ya no existe**. |
| 0.4 | `FLASHBOTS_SIGNER_KEY` en VPS `.env` | **NO COMPUTADO** | Estado del VPS; esta validación no lo toca. |
| 0.5 | Semántica kill-switch + drill trip/untrip | **SIGUE PENDIENTE, y es coherente** | No se localizó documento de drill en `docs/` ni `audits/`. El compañero `GATES-G1-G8-ESTADO.md` ya lo declara "drill trip/untrip NO documentado" ⇒ el runbook **no se contradice a sí mismo**: el paso sigue siendo un prerrequisito abierto. |
| 0.6 | Firma canary fondeada (gas ≥10 tx) | **NO COMPUTADO** | Estado on-chain; no se toca. |

### Fase 1 — Variables exactas

| Paso | Veredicto | Evidencia medida |
|---|---|---|
| `ARBX_LIVE_EXEC_ENABLED=true` **exacto en minúscula**; "True"/"False"/"1" son no-ops (test `exact_true_only`) | **SIGUE VÁLIDO — y la cita es EXACTA** | `backend/relays-client/src/live_exec_policy.rs:86` = `fn exact_true_only()`, cuerpo **L87-89**: `for s in ["", "false", "1", "TRUE", "true "] { assert!(!…enabled) }`. El runbook (y el ACTA, que cita `:87-89`) apuntan exactamente a esas líneas. |
| `ARBX_LIVE_EXEC_CHAINS=1` (mainnet); "1,11155111" incluye Sepolia | **SIGUE VÁLIDO — cita EXACTA** | `live_exec_policy.rs:71` = `fn explicit_mainnet_is_supported()`, cuerpo **L71-76**: assert OK para chain `1` y `11155111`, `is_err()` para `137`. |
| `ARBX_TRADE_MODE=live` (hoy paper) | **SIGUE VÁLIDO** | `ARBX_TRADE_MODE` presente en 33 archivos de `origin/main`. |
| `FLASHBOTS_SIGNER_KEY` ya debe existir (Fase 0.4) | **SIGUE VÁLIDO** (remite a 0.4, que sigue abierto) | — |

### Fase 2 — Aplicación

| Paso | Veredicto | Evidencia medida |
|---|---|---|
| `ssh arbx` / `cd /opt/arbitragex-v2` / editar `.env` | **NO COMPUTADO** | Estado del VPS, fuera de esta validación. |
| **`docker compose --env-file .env -f docker/compose.dev.yml up -d relays-client`** | **OBSOLETO RESPECTO DEL DEPLOY REAL** | El pipeline de producción usa **`compose.prod.yml`**: `.github/workflows/auto-deploy-vps.yml:211` → `COMPOSE_FILE="docker/compose.prod.yml"`. Ese workflow cita `compose.prod.yml` **3 veces** y **`compose.dev.yml` 0 veces**. El runbook manda reiniciar un servicio **de producción** con el compose **de desarrollo**. Ver §2. |
| `docker logs relays-client --tail 50` | **SIGUE VÁLIDO** | comando genérico, no depende de rutas. |
| "El binario Rust lee env en runtime (`live_exec_policy.rs:19-24`) — restart basta" | **SIGUE VÁLIDO — cita EXACTA** | `live_exec_policy.rs` tiene 97 líneas hoy y **L19-24 es literalmente** `pub fn from_env() { Self::from_raw(std::env::var("ARBX_LIVE_EXEC_ENABLED")…, std::env::var("ARBX_LIVE_EXEC_CHAINS")…) }`. El archivo creció pero **esas líneas no se movieron**. |
| NOTA RULE 03 (`NEXT_PUBLIC_*` sí requiere rebuild) | **SIGUE VÁLIDO** | coherente con RULE 03 y con Fase 0.3. |
| `docker/compose.dev.yml` existe y declara el servicio | **VÁLIDO como archivo** | `git cat-file -e origin/main:docker/compose.dev.yml` exit 0; `relays-client:` es servicio en L219. (Existe ≠ es el correcto: ver la fila del comando.) |

### Fase 3 — Canary

| Paso | Veredicto | Evidencia medida |
|---|---|---|
| Capital en riesgo ≤ **$350** · principal TLS ≤ **5 WETH** | **SIGUE VÁLIDO** | `CLAUDE.md` §34.5 (`L473` = `### 34.5 AUTORIZACIÓN PERMANENTE CONDICIONADA DEL OPERADOR (2026-09-15)`) y `L482` cita el canary "capital en riesgo ≤ $350, principal TLS 5 WETH". Coincide con el runbook. |
| Abort si kill-switch trip / gas > floor / revert / divergencia | **SIGUE VÁLIDO** | doctrina, sin artefacto que invalidar. |

### Fase 4 — Parada / Rollback

| Paso | Veredicto | Evidencia medida |
|---|---|---|
| `redis-cli SET arbx:killswitch '{"enabled":true,…}'`; "check 1 del pre-execute (`pre_execute_checklist.rs:260`) lo bloquea antes de firmar" | **SIGUE VÁLIDO — cita EXACTA** | `arbx:killswitch` presente en 43 archivos. El archivo único que termina en `pre_execute_checklist.rs` es **`backend/shared-rs/src/pre_execute_checklist.rs`** (1219 líneas) y su **L260** es hoy literalmente `/// Check 1: Kill switch — reads arbx:killswitch JSON from Redis.`, con `check_kill_switch` en L263 y `KILLSWITCH_KEY` en L264. |
| `ARBX_LIVE_EXEC_ENABLED=false` + `up -d relays-client` (<10s) | **SIGUE VÁLIDO** (arrastra el hallazgo del compose, §2) | — |
| "Irreversibles: tx ya minadas + gas quemado" | **SIGUE VÁLIDO** | doctrina del canary. |

---

## 2. Pasos que YA NO APLICAN porque su condición cambió

**(a) Fase 0.3 — la deuda de deploy está saldada.** El runbook y `GATES-G1-G8-ESTADO.md:7` declaran "VPS
`a06a968d` healthy; **PERO 3 commits locales sin deploy** (`fdb40401`,`125b1e0b`,`dcfe890c`)". Hoy los tres
—y `a06a968d`— son **ancestros de `main`** (`c89d21a3`). No hay 3 commits sin deploy: están integrados. El
**estado empotrado en el paquete está viejo al menos en G1**, que pasó de "⚠️ PARCIAL" a un hecho distinto.

**(b) Fase 2 — el compose apunta al archivo equivocado.** El comando del runbook usa
`-f docker/compose.dev.yml` sobre el **VPS de producción**, mientras el deploy real usa
`COMPOSE_FILE="docker/compose.prod.yml"` (`auto-deploy-vps.yml:211`). Es la clase de paso que envejece en
silencio: el comando **sigue siendo sintácticamente válido** y por eso se ejecutaría con confianza.
**No se afirma cuál corresponde al VPS** —eso exigiría medirlo, y esta orden no toca el VPS—: se afirma que
**el runbook y el pipeline real discrepan**, y que ejecutar Fase 2 tal cual reiniciaría el servicio con el
compose de desarrollo en un entorno de producción.

**(c) La tabla de "Estado hoy" del propio runbook (Fase 0) y `GATES-G1-G8-ESTADO.md`** son fotografías del
2026-09-17. Al menos la columna de 0.3 ya es falsa.

---

## 3. Referencias que NO resuelven (hallazgo)

**1 referencia de rama no resuelve.**

| Referencia | Dónde se cita | Medición | Resultado |
|---|---|---|---|
| `fix/v3-slot0-coverage-20260917` | RUNBOOK Fase 0.1; GATES L19 ("El trabajo YA está en curso en …"); ACTA L21 y L29 | `git ls-remote origin refs/heads/fix/v3-slot0-coverage-20260917` | **vacío — no existe en el remoto** |

**Caracterización honesta, sin inflar el hallazgo:** la rama **está ausente del remoto**; eso es un hecho
medido. **No se afirma que sea una cita fabricada** como `codex/567` + `9a10350` (que se verificaron
inexistentes en 2026-09-15): una rama mergeada puede haberse borrado, y el trabajo que se le atribuye
(`dcfe890c`) **sí está en `main`**. Lo que sí se afirma es que **hoy la referencia no resuelve**, que el
runbook la presenta como el prerrequisito pendiente y que GATES la describe como trabajo "en curso" — dos
afirmaciones que ya no se pueden verificar contra el remoto.

**Todo lo demás resuelve.** Medido, y es el contraste que importa contra el precedente:

| Referencia | Medición | Resultado |
|---|---|---|
| `fdb40401`, `125b1e0b`, `dcfe890c`, `a06a968d` | `git cat-file -e <sha>^{commit}` | **exit 0 en los 4**, y los 4 son ancestros de `main` |
| `backend/relays-client/src/live_exec_policy.rs` | `git cat-file -e origin/main:<ruta>` | **exit 0** |
| `docker/compose.dev.yml` | idem | **exit 0** |
| `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` (fuente de G1-G8) | idem | **exit 0** |
| `CLAUDE.md` §34.5 | idem + `git grep` | **exit 0**, 7 ocurrencias de `34.5` |
| `live_exec_policy.rs:19-24` · `:71-76` · `:87-89` | volcado de líneas | **las tres EXACTAS** |
| `pre_execute_checklist.rs:260` | volcado de líneas | **EXACTA** |

**Ninguna cita de código o de commit del runbook está fabricada.** La única que no resuelve es una rama.

---

## 4. Lo que NO se pudo computar, y por qué

La herramienta SQL disponible para leer PostgreSQL devuelve `{"ok": true, "data": "", "rows_affected": 0}`
**incluso para `SELECT COUNT(*) AS simulations_total FROM simulations;`**. Es decir: reporta éxito **sin
devolver dato**. Eso **no es una medición**, y en particular **un resultado vacío no se lee como cero** —
`ok:true` no prueba que la consulta corriera ni que el conteo sea 0.

Por eso quedan **NO COMPUTADOS**, con esta razón: los conteos de `simulations.passed` y `executions` que
sostienen G2 y G3, y por tanto **si la precondición `G1-G8` del runbook cambió o no**. El runbook declara en
su `L4`: *"Estado actual: **NO PASS** — este runbook NO se ejecuta hoy"*. **Esa afirmación no se pudo
re-verificar**, ni para confirmarla ni para refutarla. No se reemplaza por una suposición.

---

## 5. ¿Se puede ejecutar hoy sin contradecir §34.5?

**Sí en cuanto a doctrina: el runbook NO contiene pasos de autorización obsoletos.**

- Su `L3` fija la precondición: *"PRECONDICIÓN INQUEBRANTABLE (§34.5): gates G1-G8 PASS con evidencia
  reproducible"*, y su `L4` agrega *"Estado actual: NO PASS — este runbook NO se ejecuta hoy"*. Eso **es**
  el camino de §34.5, no una ceremonia paralela.
- §34.5 existe y dice lo que el runbook supone: `CLAUDE.md:473` = *"34.5 AUTORIZACIÓN PERMANENTE
  CONDICIONADA DEL OPERADOR (2026-09-15)"*, y `CLAUDE.md:482` = *"…proceden **SIN nueva ceremonia de
  autorización**"*.
- El runbook **no pide** ninguna autorización adicional, ni reunión de aprobación, ni firma del operador
  distinta de la que ya está registrada. Su Fase 0.4 (`FLASHBOTS_SIGNER_KEY` "la pone el operador, NUNCA por
  chat") **coincide** con §34.5.5: la confirmación en el momento del broadcast es notificación de ejecución,
  no pregunta.
- **Conclusión:** el runbook es **compatible con §34.5**; lo que lo frena es su **propia precondición
  G1-G8**, cuyo estado hoy **no se pudo re-verificar** (§4) y cuyo fragmento medible (G1) **ya cambió**
  (§2a). No hay paso de autorización obsoleto que borrar.

---

## 6. Declaraciones exigidas

- **NO se ejecutó ningún paso del runbook.** Cero `ssh`, cero `docker compose`, cero `redis-cli`, cero
  lectura de `.env` del VPS. Es una validación de vigencia.
- **Sin firma, sin broadcast, sin capital.** No se tocó ninguna wallet ni se emitió transacción.
  `ARBX_TRADE_MODE=paper` intacto.
- **Esto NO mueve P/N: sigue `0/115`.** No cambió el SSOT, no cambió el código, no cambió ningún estado de
  ejecución: es un documento de análisis.
- **No se modificó el runbook ni el paquete de activación.** Viven en `audits/`, fuera del alcance de esta
  tarea (`docs/release/`). Se reportan; no se reescriben.

---

## 7. Evidencia reproducible

```
git rev-parse origin/main                                   -> c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e
git cat-file -e origin/main:audits/live-activation-package-20260917/RUNBOOK-ACTIVACION-MAINNET.md   -> exit 0
git hash-object <disco>/RUNBOOK-ACTIVACION-MAINNET.md = 2122fe1cce7e = origin/main:<ruta>            -> iguales
git ls-remote origin refs/heads/fix/v3-slot0-coverage-20260917                                       -> vacío
git cat-file -e fdb40401^{commit} / 125b1e0b / dcfe890c / a06a968d                                   -> exit 0 (4/4)
git merge-base --is-ancestor <cada uno> origin/main                                                  -> en main (4/4)
git show origin/main:backend/relays-client/src/live_exec_policy.rs   -> L19-24 from_env(); L71-76 explicit_mainnet_is_supported; L86-90 exact_true_only
git show origin/main:backend/shared-rs/src/pre_execute_checklist.rs  -> L260 "Check 1: Kill switch — reads arbx:killswitch JSON from Redis"
git grep -n --fixed-strings 'compose.prod.yml' origin/main -- .github/workflows/auto-deploy-vps.yml  -> L211 COMPOSE_FILE="docker/compose.prod.yml" (3 citas)
git grep -n --fixed-strings 'compose.dev.yml' origin/main -- .github/workflows/auto-deploy-vps.yml   -> 0 citas
git show origin/main:CLAUDE.md | grep -n '34.5'   -> L473 (encabezado §34.5) ; L482 ("SIN nueva ceremonia")
git grep -l 'arbx:killswitch' origin/main         -> 43 archivos
SELECT COUNT(*) AS simulations_total FROM simulations;  -> {"ok":true,"data":"","rows_affected":0}  (NO computa)
```
