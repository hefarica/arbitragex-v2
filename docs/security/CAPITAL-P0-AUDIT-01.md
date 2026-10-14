# CAPITAL-P0-AUDIT-01 — Locus e invariante de los 8 P0 de capital

- **run_id**: `arbx-entrega-20261008` · **fase**: `implementation`
- **SHA_BASE**: `8414e51211d0a26d664b7e669af2eacbee8d1dd4`
- **Dueño**: Security · **Permisos ejercidos**: SOLO LECTURA (runtime, VPS, repo). Único path escrito: `docs/security/`.
- **Artefacto**: locus `archivo:línea` + invariante faltante + cambio mínimo + veredicto de bloqueo, para F25, F26, F27, F28, F30, F32, F33 y F37.
- **CERO VALORES DE SECRETO.** Se documenta clase, ruta y mecanismo. Ningún comando de este relevamiento imprimió un valor (Anexo B).

---

## 0. Veredicto

**MAINNET LIVE CON CAPITAL = NO-GO.** De los 8 P0, **siete** bloquean un canary acotado por sí solos, y el octavo (F27) **sólo podría convivir con un canary si F25 y F26 estuvieran cerrados, y no lo están** — de modo que hoy **los 8 bloquean**. El dictamen del operador ("aunque mañana G-SIM pasara, LIVE todavía no estaría autorizado automáticamente") queda **confirmado con locus y línea**.

Y hay un hallazgo transversal que **no es un P0 del catálogo pero condiciona a cuatro de ellos**: el archivo que contiene **los fixes ya redactados de F25, F26, F27 y F28** (`docs/contracts/proposed/FlashLoanExecutor.fixed.sol`) **no está en el remoto, no está trackeado y no está en el árbol desplegado**: existe **sólo como archivo suelto en una estación de trabajo**. Ver §9.

---

## 1. Procedencia: sobre qué árbol se afirma cada línea

Un auditor de capital responde por el árbol **desplegado**. Medido:

| Sujeto | Identidad | Comando |
|---|---|---|
| `main` en el remoto | `8414e51211d0a26d664b7e669af2eacbee8d1dd4` | `git ls-remote origin refs/heads/main` |
| Árbol desplegado en el VPS | `8414e512…` en rama `main`, último commit `Merge pull request #861 from hefarica/fix/sim-fund-04` | `ssh arbx "cd /opt/arbitragex-v2 && git rev-parse HEAD"` |
| Checkout compartido local | `858b943fd80c8b5e1606d220aa87d63b86f4ba15` en rama `fix/perhop-reserves-01` | `git -C <checkout> rev-parse HEAD` |
| ¿Existe esa rama en el remoto? | **NO** — `No commit found for the ref fix/perhop-reserves-01` | `gh api .../contents/<path>?ref=fix/perhop-reserves-01` |

Por lo tanto: **el árbol desplegado ES `main@8414e512`**, y el checkout local (que no está en el remoto) no es autoridad. Todos los `archivo:línea` de este informe están medidos sobre **copias de `main@8414e512`** descargadas por API (`Accept: application/vnd.github.raw`) y grepeadas localmente.

**Control de identidad del contrato principal** — byte a byte, tres árboles:

| Árbol | Blob de `contracts/src/FlashLoanExecutor.sol` |
|---|---|
| `main` (API, `?ref=main`) | `96be27838d149d3a3f1b74b5bd575c89d25017a9` (22807 B) |
| Checkout local (`HEAD`) | `96be27838d149d3a3f1b74b5bd575c89d25017a9` |
| **VPS desplegado** (`git rev-parse HEAD:<path>` **y** `git hash-object <worktree>`) | `96be27838d149d3a3f1b74b5bd575c89d25017a9` — `git status --porcelain` para ese path: **vacío** (sin modificaciones locales) |

⇒ El contrato auditado es **idéntico** en los tres árboles. Para los demás archivos cité la línea de `main`; **tres** números difirieron de mi checkout y usé los de `main` (`runner.rs` 631→**669**, `killswitch.rs` 49/82→**50/83**).

### 1.1 Defecto de medición propio, declarado

Mi primer control de blobs devolvió **404 para los 12 archivos**, incluidos `contracts/src/FlashLoanExecutor.sol` y `backend/`. **No era un hecho: era un artefacto de mi propio quoting** (`"$f?ref=main"` en PowerShell). Lo detecté porque el 404 era universal y el listado de raíz probaba lo contrario (`contracts/` y `backend/` **sí** existen en `main`). Corregido con quoting correcto: el archivo existe y su blob coincide. **Ninguna afirmación de este informe se apoya en aquel 404.**

---

## 2. F25 — Autorización del callback de Balancer

- **Locus**: `contracts/src/FlashLoanExecutor.sol:336` `function receiveFlashLoan(`; guardas en `:346` (`revert FL_BalancerVaultNotSet`), `:350` (`if (msg.sender != balancerVault) revert FL_UnauthorizedCaller();`), `:353` (`revert FL_NoProviderConfigured`); ejecución en `:360` `asset.forceApprove(arbitrageExecutor, amount);` y `:363` `(bool success,) = arbitrageExecutor.call(userData);`. Contraste: la ruta Aave **sí** ata la petición — `:310` `executeOperation(..., address initiator, ...)` y `:316` `if (initiator != address(this)) revert FL_InvalidInitiator();`.
- **Invariante que falta**: **el callback debe estar atado a una petición que ESTE contrato inició.** Las tres capas existentes prueban sólo una cosa: que `msg.sender` es el Vault configurado. No prueban que el préstamo lo pidiera este contrato. La interfaz de Balancer V2 no entrega `initiator`, así que la única forma de atarlo es un **digest de petición** registrado al pedir y consumido al atender. Medido: `grep` sobre la copia de `main` de ese archivo → `pendingRequest` **0 hits**, `requestDigest` **0 hits** (ausencia medida sobre líneas de código, no sobre prosa).
- **Cambio mínimo**: portar `FIX A` del borrador propuesto: escribir `pendingRequest = keccak256(...)` en `requestFlashLoan` (`:284`) y exigir+limpiar ese digest en la primera línea útil de `receiveFlashLoan` (`:341`), revirtiendo si no coincide. La atomicidad de la EVM garantiza que el digest se limpia si el callback revierte.
- **Veredicto**: **NO-GO BLOQUEANTE.** Cualquier tercero puede abrir un flash loan de Balancer con `recipient = FlashLoanExecutor` y hacer que el Vault **real** invoque el callback con `userData` de su elección; las tres guardas pasan y el ejecutor queda convertido en un títere que aprueba y llama a `arbitrageExecutor` (`:360`/`:363`) con calldata ajena. Coste para el atacante: la comisión de Balancer V2, que es 0.

## 3. F26 — Utilización de saldo previamente depositado en el wrapper

- **Locus**: `contracts/src/FlashLoanExecutor.sol:374` `if (asset.balanceOf(address(this)) < amountOwed) revert FL_RepaymentShortfall();` — y el mismo patrón en la ruta Aave, `:400`. El gate mide el **balance TOTAL** del contrato.
- **Invariante que falta**: **el repago debe medirse contra el INFLUJO DE ESTA OPERACIÓN (delta), no contra el saldo acumulado.** Cualquier saldo previamente depositado en el wrapper cuenta como repago, es decir: el dinero de la víctima puede satisfacer la obligación del atacante.
- **Cambio mínimo**: portar `FIX B`: tomar `balBefore` **antes** de la llamada al ejecutor y sustituir la comparación por `balanceOf(this) >= balBefore + amountOwed` (más el piso de beneficio del `FIX C` si el operador lo fija).
- **Veredicto**: **NO-GO BLOQUEANTE**, y es el que convierte F25 en robo: **F25 + F26 juntos son una primitiva de extracción** — un tercero sin capital propio dispara el callback, y su obligación de repago puede quedar cubierta por el saldo que el wrapper ya tenía. F25 sin F26 permitiría "ejecutar calldata ajena"; F26 sin F25 permitiría "contar lo ajeno como propio"; juntos, **el wrapper paga**. **NO VERIFICADO** (y hay que verificar antes de dimensionar el daño): hasta dónde puede llevar `ArbitrageExecutor` el valor extraído — su allowlist de routers y el gasto por router (`contracts/src/ArbitrageExecutor.sol:347` menciona SC-12 y A5) acotan a qué destinos puede salir el dinero, pero **no medí esa allowlist** en esta tarea.

## 4. F27 — Retiro de beneficios a tesorería

- **Locus**: `contracts/src/FlashLoanExecutor.sol` — ausencia medida sobre código: `treasury` **0 hits**, `sweepProfit` **0 hits**. El beneficio retenido vive en el balance del wrapper sin función de salida. El borrador propuesto sí la trae (`setTreasury` + `sweepProfit`).
- **Invariante que falta**: **el beneficio retenido debe tener una salida con control de rol hacia una tesorería fría; el wrapper NO debe ser la tesorería.** Hoy el wrapper acumula y no evacúa: es simultáneamente el punto de pérdida y el depósito.
- **Cambio mínimo**: portar `FIX D`: `treasury` (seteable por rol admin, revirtiendo si es `address(0)`) + `sweepProfit()` restringido a admin, que transfiere el excedente sobre el principal operativo.
- **Veredicto**: **no bloquea por sí solo un canary acotado** — con notional pequeño y F25/F26 cerrados, un wrapper que retiene sería un riesgo de diseño, no de extracción. **Pero hoy no puede convivir**: sin F25/F26, F27 es el agravante que hace que lo acumulado sea robable, y sin salida no se puede vaciar el wrapper a un lugar seguro. Por eso, en el estado actual: **NO-GO**.

## 5. F28 — Pause guardian on-chain inmediato

- **Locus**: `contracts/src/FlashLoanExecutor.sol` — ausencia medida: `GUARDIAN_ROLE` **0 hits**, `_pause` **0 hits**, `Pausable` **0 hits** (el wrapper **no tiene pause**). La única pausa on-chain de la familia es la del ejecutor: `contracts/src/ArbitrageExecutor.sol:601` `function pause() external onlyRole(ADMIN_ROLE) {` (y `:606` `unpause`). Y `contracts/src/AdminTimelock.sol:60` `contract AdminTimelock is Initializable, TimelockControllerUpgradeable {` con `import .../TimelockControllerUpgradeable.sol` en `:46`: las operaciones de admin (incluida `pause`) pasan por el timelock con `minDelay`.
- **Invariante que falta**: **una parada on-chain inmediata para el wrapper que no dependa del timelock.** Hoy la única parada inmediata es la de **fuera** de la cadena (kill-switch del backend), y el contrato sigue aceptando callbacks aunque el backend esté caído o comprometido. Con `minDelay` de horas, "parar" no es una respuesta a un incidente en curso.
- **Cambio mínimo**: portar `FIX E`: un `GUARDIAN_ROLE` acotado que pueda `pause()` y **no** pueda `unpause()`, ni upgradear, ni mover fondos.
- **NO VERIFICADO**: si `ADMIN_ROLE` fue transferido efectivamente al timelock en el deploy. Es un hecho on-chain de tiempo de despliegue, no verificable leyendo el repo; se cierra con la dirección desplegada + `hasRole(ADMIN_ROLE, <timelock>)`. Si NO se transfirió, F28 es **peor** de lo que dice el catálogo (pausa en manos de una EOA caliente).
- **Veredicto**: **NO-GO BLOQUEANTE.** Sin parada on-chain inmediata no hay forma de detener una sangría originada en una clave caliente.

## 6. F30 — Secretos raw proyectados hacia Redis/AOF

- **Evidencia de primera mano (de `t90`, no re-descubierta)**: `docs/security/SEC-ENV-EXPOSURE-01.md` — **14 de 25** contenedores con material de secreto en claro, **129 slots**, **14 variables distintas**; frontera **no es root sino el grupo `docker`** (`docker:x:988:arbx,deploy`, ambos NO-ROOT; `auditor` excluido); `Vault` **SELLADO** (`Unseal Progress 0/2`) y **sin cablear**; Docker secret **FANTASMA** en `compose.prod.yml:1011-1013` (archivo inexistente, 0 consumidores); `env_file: - ../.env` en 10 servicios (`compose.prod.yml:138,177,209,251,286,325,368,407,486,713`).
- **AOF, verificado hoy por ejecución** (solo lectura): `CONFIG GET appendonly` → `yes` · `CONFIG GET appendfsync` → `everysec` · `CONFIG GET dir` → `/data` · `CONFIG GET dbfilename` → `dump.rdb` · en `/data` existe `appendonlydir` en modo `drwx------ redis:redis` · `redis-cli PING` → **`PONG` sin `AUTH`** ⇒ `nopass` **recorroborado hoy**.
- **Cifrado de sobre en la base**: `database/migrations/120_service_credentials_envelope_encryption.sql:71` `ADD COLUMN secret_ciphertext BYTEA`, `:78` `secret_salt`, `:85` `secret_key_version`, `:92` `secret_hint`; el descifrado existe como SQL en la propia migración (`:168` `pgp_sym_decrypt(secret_ciphertext, …)`).
- **Invariante que falta**: **ninguna copia en claro debe sobrevivir a la frontera del sobre.** Tal como está, un secreto proyectado a Redis adquiere una **segunda copia, durable en disco** (AOF `everysec`), **fuera** del cifrado de sobre de Postgres, en una instancia **sin autenticación**. La opción A del cifrado queda anulada por la proyección.
- **Cambio mínimo**: (1) **no proyectar secretos raw** a Redis — proyectar un handle/ID y resolverlo en el punto de uso; (2) `requirepass` en Redis + saneado del AOF (`BGREWRITEAOF` tras purgar) — porque **la rotación no borra lo ya escrito**; (3) inyección por Docker secrets y eliminación del `env_file` en claro.
- **NO ENCONTRADO (declarado, no inferido)**: el código que **lee/descifra** `secret_ciphertext` y lo **escribe en Redis**. Búsquedas ejecutadas: `secret_ciphertext|decrypt_secret|project.*redis|redis.*credential` en `backend/**/*.rs` y `secret_ciphertext|decryptSecret|createDecipheriv` en `frontend/**` → **sin coincidencias**. «No encontrado» **no** es «no existe»: puede vivir fuera de esos árboles o usar otro nombre. Ésa es exactamente la evidencia que falta para cerrar la vía completa.
- **ENMIENDA A LA HOJA DE ROTACIÓN DE `t90`** (declarada): la hoja sigue vigente en sus 6 fases y su paso irreversible, **pero le falta un paso que hoy medí**: rotar en `.env` **no purga el AOF**; mientras el AOF no se reescriba, el valor viejo sigue en disco en `/data/appendonlydir`. La hoja queda **ampliada**, no reemplazada.
- **Veredicto**: **NO-GO BLOQUEANTE.** No es un riesgo teórico: es una exposición **activa** medida hoy.

## 7. F32 — Breakers como último gate pre-broadcast

- **Locus**: `backend/relays-client/src/submit_engine.rs:330` `match pre_execute_checklist(&mut ctx).await {` — y el gate real está dentro: `backend/shared-rs/src/pre_execute_checklist.rs:214` `check_kill_switch(ctx.redis).await?;`, `:217` `check_paper_mode(...)`, `:251` `check_circuit_breaker_off(ctx.redis).await?;` (clave `:32` `CIRCUIT_BREAKER_KEY = "arbx:circuit_breaker:state"`, leída en `:557`). La matemática del breaker: `backend/shared-rs/src/risk_ledger.rs:138` `pub fn compute_breakers(` con `:49` `pub enum BreakerLevel` (Kill/HardPause/Pause/Warn/Ok). El broadcast: `submit_engine.rs:767` `let broadcast_result = multi_relay.broadcast(&bundle, signer.as_ref()).await;`.
- **Distancia medida entre el gate y el broadcast**: `:330` → `:767` = **437 líneas**, con `:487` `let bundle = match build_and_sign(`, `:615` y `:750` `assert_bundle_head(...)`, `:624` `.call_bundle(signer.as_ref(), &bundle.tx_raw_hex, target_block)` (round trip HTTP a Flashbots) en el medio.
- **Invariante que falta**: **el último gate antes del broadcast debe ser el gate.** Un breaker que se evalúa al principio no protege el envío: protege la intención.
- **Cambio mínimo**: re-ejecutar el subconjunto barato (`kill_switch`, `circuit_breaker`, `paper_mode`) inmediatamente antes de `:767`, fail-closed, y **sin pasar por caché** (ver F33).
- **Veredicto**: **NO-GO BLOQUEANTE** para capital. Para paper es irrelevante (no hay broadcast).

## 8. F33 — Ventana sin relectura fresca de autoridad (el que separa un bug de un robo)

- **Locus de las lecturas de autoridad**: `submit_engine.rs:330` (checklist: `:214` kill-switch, `:217` paper-mode, `:251` breaker) · `submit_engine.rs:393` `if self.kill_switch.is_enabled().await {` · `backend/relays-client/src/bundle_builder.rs:57` `crate::live_exec_policy::LiveExecPolicy::from_env()` (`.assert_broadcast_allowed`). Después de eso, la única verificación antes de emitir es `:750` `assert_bundle_head(...)` — que relee **la cabeza de la cadena**, **no la autoridad**.
- **Suelo de obsolescencia, medido en código**: `backend/shared-rs/src/killswitch.rs:50` `cache_ttl: Duration,` · `:61` `cache_ttl: Duration::from_secs(1),` · `:83` `if at.elapsed() < self.cache_ttl {` ⇒ **la lectura de `:393` puede ya estar hasta 1 s vieja cuando ocurre**, y nada la vuelve a leer. Lo mismo vale para paper-mode: `backend/shared-rs/src/paper_mode.rs:81`, `:92` `cache_ttl: Duration::from_secs(1)`, `:174` `if at.elapsed() < self.cache_ttl {`.
- **Qué puede cambiar la autoridad dentro de la ventana** (todo medido): (a) el operador; (b) **`recon` la dispara solo**: `backend/recon/src/anomaly.rs:74` `if cfg.auto_trip_on_high_revert_rate {` → `:82` `info!(event = "anomaly.kill_switch_tripped", reason);`; (c) el breaker, por la clave `arbx:circuit_breaker:state`. Las tres son claves de Redis **leídas una vez** antes de la ventana.
- **Cuánto dura la ventana: NO COMPUTADO (empírico).** Suelo **estático** probado: **1 s** (TTL de caché, código citado) **+** la duración de ≥6 round trips de red entre `:393` y `:767` (`build_and_sign` en `:487`, dos `assert_bundle_head` `:615`/`:750`, `call_bundle` `:624`, más nonce/RPC). **No enumeré los timeouts por llamada**, así que la cota superior no está medida, y **no existe hoy un instrumento que dé la duración real**: en el camino que llega a `:767` no hay ningún par de eventos con timestamp (`checklist_pass` → `broadcast_start`) ni histograma, y en paper nunca se alcanza el broadcast.
- **Evidencia que cerraría F33** (concreta, falsable): **(1)** un test que (i) lea autoridad, (ii) **mute la clave del kill-switch en Redis**, (iii) afirme que el broadcast **aborta** — hoy ese test no existe; y **(2)** el par de timestamps del mismo envío para cuantificar la ventana.
- **Cambio mínimo**: un `assert` de autoridad **inmediatamente antes de `:767`**, con lectura que **evite la caché** (un `state_fresh()` que saltee `cache_ttl`), fail-closed; y el test de (1). Ojo con el atajo: re-leer a través del caché de 1 s **no** cierra la ventana, sólo la acorta.
- **Veredicto**: **NO-GO BLOQUEANTE — el más importante de los ocho.** Es la diferencia entre que un breaker inexacto cause una pérdida y que un botón de pánico no detenga nada.

## 9. F37 — Canon v4 de las 264 sin recorrido integral acreditado

- **Locus del canon**: `backend/searcher-rs/src/cartridge/runner.rs:669` `.is_some_and(|cv| cv == "arbx.cartridge.agent/4");` (en `main`; en mi checkout era `:631`). **Locus del lote**: `backend/searcher-rs/src/cartridge/manifest_test.rs:19` `const EXPECTED_ENTRIES: usize = 264;` y `:65` `fn manifest_has_exactly_264_entries()` con la aserción de `:66` `assert_eq!(load_entries().len(), EXPECTED_ENTRIES);`.
- **Invariante que falta**: **«canon v4 correcto» y «264 entradas en el manifiesto» NO equivalen a «las 264 recorren el camino hasta ejecución y settlement».** Lo acreditado hoy es el **manifiesto matemático** (264 filas) y una comprobación de versión en tiempo de evaluación. Lo que **no** está acreditado es el recorrido **integral** cartridge → propuesta → plan → ejecución → settlement con atribución por `cartridge_id`.
- **Cambio mínimo**: un barrido **parametrizado por las 264 claves** que lleve cada una hasta un resultado de settlement, con **tally por `cartridge_id`** como artefacto (es el benchmark propio del proyecto: oportunidades **ATRIBUIDAS** por `cartridge_id`), y su run commiteado como evidencia. Sin ese artefacto, «las 264» es una afirmación de catálogo, no un hecho medido.
- **Veredicto**: **NO-GO BLOQUEANTE** para capital dimensionado al lote. Un canary acotado podría convivir **sólo** si se restringe a cartuchos con recorrido acreditado individualmente — y hoy no hay ninguno acreditado así, de modo que en la práctica bloquea.

---

## 10. Hallazgo transversal — la remediación de F25–F28 no es durable

El archivo que contiene **los fixes ya redactados** de F25 (`FIX A`), F26 (`FIX B`), F27 (`FIX D`) y F28 (`FIX E`) es `docs/contracts/proposed/FlashLoanExecutor.fixed.sol`. Medido:

| Dónde | Estado | Comando |
|---|---|---|
| Remoto `main` | **NO está** | `gh api .../contents/docs/contracts/proposed/FlashLoanExecutor.fixed.sol?ref=main` → `404 Not Found` (quoting verificado) |
| Checkout local | **NO trackeado** — `??` | `git ls-files --error-unmatch <path>` → `did not match any file(s) known to git`; `git status --porcelain` → `?? …` |
| Árbol desplegado en el VPS | **NO existe** | `ssh arbx "ls -la /opt/arbitragex-v2/docs/contracts/proposed/"` → `No such file or directory` |

⇒ **El diseño de remediación de la mitad de los P0 de capital existe únicamente como archivo suelto en una estación de trabajo.** Un `git clean`, un cambio de máquina o un `%TEMP%` reciclado, y se pierde. Es exactamente el modo de fallo que la propia doctrina llama «trabajo editado y nunca PR-eado».

**Acción mínima**: publicarlo por el mismo procedimiento que ya se usó para el artefacto de `t90` (clon aislado + rama + PR **sin merge**, con el diff escaneado antes de empujar). **No lo hice en esta tarea**: el path escribible de este encargo es `docs/security/` y `contracts/` es solo-lectura. Queda como pedido explícito.

---

## 11. Z2 — Tabla de bloqueo para un canary

| P0 | Bloquea un canary acotado? | Por qué |
|---|---|---|
| **F25** callback de Balancer | **SÍ — bloqueante** | Tercero no autenticado induce calldata ajena con allowance fresca; coste 0 |
| **F26** saldo pre-depositado | **SÍ — bloqueante** | El repago se mide contra el balance total ⇒ lo ajeno paga lo propio; con F25 es extracción |
| **F27** tesorería | **Sí en el estado actual** | Sin salida del wrapper, lo acumulado es robable (F25+F26) y no evacuable |
| **F28** pause guardian | **SÍ — bloqueante** | No hay parada on-chain inmediata: el timelock no responde a un incidente en curso |
| **F30** secretos → Redis/AOF | **SÍ — bloqueante** | Exposición **activa** medida hoy (`appendonly yes`, `nopass`, 129 slots en claro) |
| **F32** breakers como último gate | **SÍ — bloqueante** | El gate está a 437 líneas y ≥2 round trips del broadcast |
| **F33** ventana sin relectura | **SÍ — bloqueante, el principal** | Suelo de 1 s de caché + ≥6 round trips sin relectura; el pánico on-chain/off-chain no detiene el envío en vuelo |
| **F37** canon v4 de las 264 | **SÍ — bloqueante** | Acreditado el manifiesto, **no** el recorrido integral a settlement |

**No es una lista de mejoras: es una lista de puertas.** Ninguna se abre con documentación; las ocho exigen código (F25–F28, F32, F33, F37) o acción del operador (F30).

---

## 12. Lo NO VERIFICADO (declarado, no asumido)

1. **Hasta dónde puede llevar `ArbitrageExecutor` el valor** bajo calldata ajena (su allowlist de routers y el gasto por router). Acota el daño de F25/F26; **no medido**.
2. **Si `ADMIN_ROLE` fue transferido al timelock** en el deploy, y con qué `minDelay`. Hecho on-chain de despliegue; cierra F28 empíricamente.
3. **El código que descifra `secret_ciphertext` y lo escribe en Redis**: **no encontrado** con los patrones declarados en §6. «No encontrado» ≠ «no existe».
4. **Duración empírica de la ventana de F33**: NO COMPUTADO; instrumento inexistente (ver §8).
5. **Timeouts por llamada** de las ≥6 llamadas de red entre la autoridad y el broadcast: no enumerados ⇒ la cota superior de F33 no está medida.
6. **Si el AOF contiene hoy material de secreto**: no se leyó su contenido (prohibido). Lo medido es que el AOF está **activo** y la instancia **sin auth**.
7. **El resto de la frontera de `t90`** sigue vigente sin cambios; este informe **no** re-midió los 14 contenedores ni los 129 slots, los **usa** como evidencia de primera mano ya existente.

---

## Anexo A — Comandos de este relevamiento (todos de lectura o de conteo)

```bash
command -v gh                      # -> /c/Program Files/GitHub CLI/gh   (exit 0)
git ls-remote origin refs/heads/main
ssh arbx "cd /opt/arbitragex-v2 && git rev-parse HEAD && git remote -v"
ssh arbx "cd /opt/arbitragex-v2 && git rev-parse HEAD:contracts/src/FlashLoanExecutor.sol"
ssh arbx "cd /opt/arbitragex-v2 && git hash-object contracts/src/FlashLoanExecutor.sol"
gh api 'repos/hefarica/arbitragex-v2/contents/contracts/src/FlashLoanExecutor.sol?ref=main' --jq '.sha,.size'
gh api 'repos/hefarica/arbitragex-v2/contents/?ref=main' --jq '.[].name'
gh api -H "Accept: application/vnd.github.raw" 'repos/hefarica/arbitragex-v2/contents/<path>?ref=main'   # copia de main para grepear
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli CONFIG GET appendonly"
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli CONFIG GET appendfsync"
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli PING"
git -C <checkout> ls-files --error-unmatch docs/contracts/proposed/FlashLoanExecutor.fixed.sol
```
Los `grep` de verificación se corrieron **sobre las copias de `main`**, no sobre la prosa: los loci citados son líneas de código.

## Anexo B — Z6: verificación de CERO valores ejecutada sobre ESTE artefacto

Escaneo corrido sobre este mismo archivo (24660 B, 180 líneas) **antes de entregarlo**, contando y clasificando por forma, sin imprimir jamás el texto encontrado:

| Comprobación | Resultado |
|---|---|
| Líneas de la forma `NOMBRE=valor` (`^[A-Z][A-Z0-9_]{2,}=`) | **0** |
| Runs alfanuméricos ≥20 caracteres | **34**, clasificados: `hex40_sha`=**6** · `path`=**26** |
| Runs de forma `hex64` (huella de un valor) | **0** |
| Runs con letras mayúsculas-dígitos-**sin minúsculas** (blob de credencial) | **0** — los 34 contienen minúsculas |
| Nombres de variable de clase secreta (`ARBX_*`, `POSTGRES_PASSWORD`, …) presentes en el artefacto | **0** (control positivo: el mismo patrón **sí** los encuentra en `SEC-ENV-EXPOSURE-01.md`) |

**Dos defectos de instrumento propios, declarados** (ambos detectados al reconciliar dos escaneos que no concordaban, y ninguno afecta la conclusión):
1. Mi **primer** control de blobs remotos dio **404 para 12 archivos por un error de quoting mío** — no era un hecho del repo (§1.1).
2. Mi **primer** clasificador etiquetó runs como `MAYUS_IDENT` usando `-match`, que en PowerShell es **case-insensitive**: la etiqueta era falsa. El escaneo reconciliado de arriba usa comparación sensible a mayúsculas y encuentra **cero** runs sin minúsculas.

## Anexo C — Regla de instrumento respetada

Ningún comando de este relevamiento volcó, citó ni hasheó un valor de secreto. Las dos únicas lecturas que tocaron el plano de secretos fueron **conteos y nombres** (`env | cut -d= -f1 | grep -c …`, `while IFS="=" read -r k _`) y **configuración** de Redis (`CONFIG GET`), nunca el contenido. **CERO valores en este artefacto.**
