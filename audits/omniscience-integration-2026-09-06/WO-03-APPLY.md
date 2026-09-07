# WO-03 — APPLY: cifrado envelope de credenciales API en Postgres

Fecha: 2026-09-06 · Gang Omniscience · applier role: `ecc:security-reviewer`
(rubric: `ecc:postgres-patterns` + `ecc:database-migrations`) · diseño:
`audits/omniscience-integration-2026-09-06/WO-03-DESIGN.md` · board:
`GOAL-WORKORDERS.md:11`.

**Estado: APPLIED_VERIFIED (local, NO-GIT)** — tsc EXIT=0 · vitest api-server
**729/729** (59 archivos) · lint-migration-rerun-lock-safety EXIT=0. CERO
git-write, CERO VPS-mutation, CERO request a dominio público, CERO
cargo/target, CERO secretos reales (único vector de key = sintético de test).

Lexicón OMEGA: las credenciales cifradas son claves de operador para
Variedades de Liquidez/oráculos; el hot-path de detección y el terminus de
ejecución quedan intactos (§32/§33/§34.3).

---

## 0. Provenance del apply (dos agentes, un WO)

Este apply aterrizó en **dos etapas**: un primer agente WO-03 (kind apply,
Oleada 4) escribió `crypto.ts`, `crypto.test.ts`, la migración, los edits de
`store.ts`/`projection.ts`/`validators.ts` y la sección de
`MIGRATION_HISTORY.md`, pero **murió antes de verificar y antes de escribir
este reporte** (no existía `WO-03-APPLY.md`). Este reemplazo (regla
Respawn-2, `~/.claude/CLAUDE.md` §Gang) audité línea a línea ese estado
contra el diseño, lo completé, corregí **1 defecto real** que la primera
verificación (nunca corrida por el caído) expuso, y ejecuté los 4 gates.

Deltas de ESTE agente sobre el estado heredado:

1. **Colisión-119 resuelta según ruling del orquestador** (`GOAL-WORKORDERS.md:11`):
   migración renombrada `119_…` → **`120_service_credentials_envelope_encryption.sql`**
   (mv de filesystem; archivo untracked — sin git-write) + 14 referencias
   internas actualizadas (ver §2).
2. **Defecto corregido en `crypto.test.ts:199-216`** — ver §3.
3. **+3 tests de proyección con filas cifradas** (entregable §9 del diseño
   que el caído no alcanzó a escribir) en `projection.test.ts:133-207` — ver §4.
4. Documentación: errata 119→120 en `WO-03-DESIGN.md:9-19`, nota de numeración
   en `MIGRATION_HISTORY.md:200-204`.

---

## 1. Inventario aplicado (con anclas file:line del estado final)

| Archivo | Rol | Anclas clave |
|---|---|---|
| `database/migrations/120_service_credentials_envelope_encryption.sql` (NUEVO) | Estructura keyless + Path A opcional | fallbacks psql `:50-57` · ADD COLUMN catalog-guarded `:62-92` · COMMENTs `:94-103` · backfill SQL `:110-138` · verificación rowCount `:140-152` · verificación roundtrip `:154-174` |
| `backend/api-server/src/credentials/crypto.ts` (NUEVO, 449 líneas) | Resolución de key, derivación, encrypt/decrypt pgcrypto, barrido de boot | errores tipados `:42-61` · `resolveMasterKeys` `:86-132` (fail-fast half-config) · `deriveRowKeyHex` `:144-146` · `maskHint` `:154-157` · `encryptSecret` `:170-186` · `decryptSecret` `:201-229` · `backfillCredentialEncryption` `:262-437` · `scrubPlaintext` `:439-448` |
| `backend/api-server/src/credentials/store.ts` (MODIF) | Persistencia envelope-aware | `DbRow` +4 columnas `:41-45` · `rowToPublic` `has_value`/`value_suffix` `:54-66` · `listCredentials` sin descifrar `:80-89` · `secretFromRow` `:95-115` · `readCredentialSecret` `:124-146` · `readCredentialForBulk` (contrato `StoredCredentialRow` intacto) `:173-206` · `upsertCredential` CASE envelope/legacy/metadata-only `:220-280` |
| `backend/api-server/src/credentials/projection.ts` (MODIF) | Proyección Redis decrypt-aware | barrido primero `:120` · descifrado pre-mirror `:142-158` · rethrow `CredentialsKeyRequiredError` `:181-187` |
| `backend/api-server/src/credentials/validators.ts` (MODIF, doc-only D7) | Contrato plaintext-only documentado | header `:14-19` — cero lógica nueva |
| `backend/api-server/src/credentials/crypto.test.ts` (NUEVO) | 27 tests del módulo crypto | vector paridad `:71-84` · maskHint `:86-95` · resolveMasterKeys `:97-153` · encrypt/decrypt `:155-232` · backfill `:237-348` |
| `backend/api-server/src/credentials/projection.test.ts` (MODIF) | +3 tests envelope | ver §4 |
| `database/migrations/MIGRATION_HISTORY.md` (MODIF) | Sección 120 | `:200-234` |
| `audits/omniscience-integration-2026-09-06/WO-03-DESIGN.md` (MODIF) | Errata 119→120 | `:9-19` |

Integración **sin cambios** (fuera de touchpoints, verificada por lectura):
`routes/credentials.ts:64` (`mirrorAfterWrite` consume
`readCredentialForBulk` decrypt-aware), `:157` (noop byte-compare
plaintext==plaintext sigue correcto), `:335-353` (upsert→mirror);
`index.ts:1590` (`void rehydrateSvcCredMirror(...)` fire-and-forget — el
rethrow aborta boot vía unhandled-rejection default de Node 20).

Fidelidad al diseño: D1-D8 implementados textualmente (D5 sin CHECK — el
transitorio encrypt→verify→scrub lo exige; D8 el listado JAMÁS descifra —
suffix viene de `secret_hint`).

## 2. Colisión-119 → `120_` (ruling del orquestador, ejecutado)

- Ruling: `GOAL-WORKORDERS.md:11` — "WO-04-Rust CONSERVA la 119 (su mitad TS
  ya cita el CHECK), WO-03 aplica como `120_`".
- Evidencia del otro claimant: `WO-04-APPLY-TS.md:35` ("bounds = CHECK de la
  migración 119") + `backend/api-server/src/routes/trading-config.ts:174` y
  `simulation/computeSimulatedNet.ts:242` (WO-04 ya aplicado en el árbol
  local cita su futura 119).
- Ejecutado: rename del archivo + header `120_…sql:1-5` + `\echo '120:'`
  (`:148,150,170,172`) + referencias en `crypto.ts:12,35,141,151,253`,
  `crypto.test.ts:9,71`, `store.ts:2,16,41`, `validators.ts:16`,
  `MIGRATION_HISTORY.md:200-204,229`, errata `WO-03-DESIGN.md:9-19`.
- Grep post-edit: cero "119" residual en `src/credentials/` y en el `.sql`
  (única mención = la nota del ruling mismo).
- El rename es seguro ANTES del primer deploy (archivo nunca commiteado ni
  aplicado — runner re-aplica todo el dir, `run_migrations.sh:5-7`).

## 3. Defecto encontrado y corregido (verificación cumplió su función)

`crypto.test.ts` (heredado del caído) test "rejects version current-1 without
PREV…" usaba `expect(() => decryptSecret(…)).toThrowError(…)` sobre una
función **async**: el rechazo jamás se observaba → unhandled rejection →
**1 test file failed en la primera corrida de este agente** (35 tests: 34
pass/1 fail + 1 error). El módulo `crypto.ts` era correcto; el test mentía
sobre su propia aserción. Fix: `await expect(…).rejects.toThrowError(…)` en
ambas ramas (`crypto.test.ts:199-216`, comentario in situ documentando el
modo de fallo). Re-run: 35/35.

## 4. Tests de proyección con filas cifradas (nuevos, `projection.test.ts:133-207`)

1. `:164` — fila envelope **se descifra antes** de mirror (`secret_value`
   proyectado = plaintext descifrado; canal reload publicado 1×).
2. `:182` — fila envelope **sin master key** ⇒ `CredentialsKeyRequiredError`
   rethrown, warn `credentials.projection_rehydrate_blocked`, Redis
   **vacío** (nunca servir proyección parcial — fail-fast RULE 02).
3. `:196` — envelope **incompleto** (sin salt) ⇒ fila saltada sin fabricar
   secreto (R8), summary `mirrored: 0`.

## 5. Gates de verificación (corridos por este agente)

| Gate | Comando | Resultado |
|---|---|---|
| Typecheck | `npx tsc --noEmit -p tsconfig.json` (backend/api-server) | **EXIT=0** (incluye los test files editados) |
| Suite objetivo | `npx vitest run src/credentials` | **35/35** (27 crypto + 8 projection) |
| No-regresión | `npx vitest run` (api-server completo) | **729/729, 59 archivos, exit 0** (753.9s) |
| Lint migración | `bash automation/tools/lint-migration-rerun-lock-safety.sh database/migrations/120_…sql` | **OK, EXIT=0** (nota benigna: trailing `\endif` psql meta-command sin `;`) |

CERO cargo (capa TS — charter). CERO compilación de Rust. CERO target/.

## 6. Cómo lee/decifra el api-server (contrato runtime, implementado)

- **Resolución** (`crypto.ts:86-132`, memoizada): `ARBX_CREDENTIALS_MASTER_KEY_FILE`
  (Vault-agent sink; ausente/vacío ⇒ throw) → `ARBX_CREDENTIALS_MASTER_KEY`
  (vacío ⇒ modo legacy) → <32 chars ⇒ throw (half-config = incidente).
  `ARBX_CREDENTIALS_MASTER_KEY_PREV` + `ARBX_CREDENTIALS_KEY_VERSION` (int ≥1,
  default 1) definen la ventana de rotación.
- **Barrido de boot** (`crypto.ts:262-437`, invocado `projection.ts:120`):
  sin key + 0 ciphertext ⇒ legacy warn; sin key + N>0 ciphertext ⇒
  `CredentialsKeyRequiredError` (**crash on boot**, RULE 02); con key ⇒
  fase-1 cifrar (guard rowCount=1) → fase-2 roundtrip verify → fase-3 scrub
  (guard), recovery de crash both-set `:330-363`, rotación v-1→v `:366-413`,
  assert invariante final `:416-423`, summary único `credentials.backfill_complete`
  (R9).
- **Lectura**: envelope ⇒ descifra (key ausente ⇒ throw); legacy ⇒ plaintext
  como hoy (`store.ts:95-115`). **Escritura**: con key ⇒ envelope + hint,
  `secret_value=NULL`; sin key ⇒ legacy idéntico a pre-WO-03
  (`store.ts:220-280`).
- **Deploy sin key = cero cambio de comportamiento** (adopción en 2 pasos:
  merge → provisionar key → restart).

## 7. Rotación y rollback (resumen operativo; detalle en diseño §6/§7)

**Rotación**: pg_dump de `service_credentials` → `.env` VPS con
`…_PREV=<K1>`, `…MASTER_KEY=<K2>`, `…KEY_VERSION=2` → `docker compose
--env-file .env … up -d api-server` (env runtime, sin rebuild — RULE 03/04
aplican solo a `NEXT_PUBLIC_*`) → boot re-cifra v1→v2 → verificar `SELECT
DISTINCT secret_key_version` ⇒ solo 2 → cerrar ventana (quitar `_PREV`,
restart). Filas v1 halladas sin PREV ⇒ `CredentialsDecryptError:
unsupported_key_version` (fail-honest).

**Rollback**:
- Pre-activación (columnas inertes, sin key): redeploy del SHA anterior.
- Post-backfill **con** key — restauración a plaintext (operador, psql):
  ```sql
  -- requiere -v arbx_credentials_master_key=<K> (Path A del runner, diseño §7)
  UPDATE service_credentials sc
     SET secret_value = pgp_sym_decrypt(sc.secret_ciphertext,
         encode(hmac(convert_to('arbx-svc-cred-v1:','UTF8') || sc.secret_salt,
             :'arbx_credentials_master_key'::bytea,'sha256'),'hex'))
   WHERE sc.secret_ciphertext IS NOT NULL;
  ```
- Post-backfill **sin** key: restaurar el pg_dump (la pérdida de la key = 
  indescifrable por diseño — propiedad del cifrado, no bug).
- Columnas: forward-only, aditivas, inertes si no hay consumidor.

**Post-deploy sugerido** (§5.4 del diseño): `XLEN arbx:opps:detected` delta 0
(este WO solo escribe `arbx:svc_cred:*` por rutas admin preexistentes).

## 8. Diffs out-of-claim NO aplicados (documentados, requieren PR)

- `database/run_migrations.sh` — inyección opcional
  `-v arbx_credentials_master_key=…` / `…_version=…` (activa Path A; diseño §7).
- `docker/compose.{dev,prod}.yml` — paso de `ARBX_CREDENTIALS_MASTER_KEY[_FILE|_PREV]`,
  `ARBX_CREDENTIALS_KEY_VERSION` al api-server (placeholders `${VAR}`, diseño §7).
- `.env` VPS — material real de key (NUNCA en repo; `openssl rand -base64 32`).

## 9. Declaraciones honestas (R8)

- **VPS: 0 requests, 0 mutaciones.** El estado live de `service_credentials`
  NO fue consultado en este apply (no necesario para edición local; el
  conteo de filas a convertir lo mide el operador con
  `SELECT COUNT(*) FROM service_credentials;` pre-deploy — opcional).
- Vault sigue SEALED shamir 3/2 **sin consumidor** — el diseño funciona
  100% sin Vault (fuente env); la fuente `_FILE` ya está implementada y
  testeada (`crypto.test.ts:136-152`) para el día que el agente exista.
- Orquestación OMEGA-team: este subagent no dispone de Task tool — la
  validación se ejecutó con los gates automatizados (tsc + 729 tests + lint
  de migraciones) en lugar de despachar un validator subordinado.
- 429/self-contamination: 0 requests a dominio público (presupuesto intacto).
