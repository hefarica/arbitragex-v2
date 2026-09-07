# WO-03 — Cifrado envelope de credenciales API en Postgres (DESIGN)

Fecha: 2026-09-06 · Gang Omniscience · reviewer role: `ecc:security-reviewer`
(rubric: `ecc:postgres-patterns` + `ecc:database-migrations`) · WO board:
`audits/omniscience-integration-2026-09-06/GOAL-WORKORDERS.md:11`
(`WO-03 … EN CURSO (diseño)`; el despacho original murió por 429 en Oleada 3 —
re-despacho con charter embebido, `GOAL-WORKORDERS.md:35`).

> **ERRATA (apply, 2026-09-06):** la migración aterrizó como
> `120_service_credentials_envelope_encryption.sql`, NO como `119_…` como
> este documento diseña. Decisión del orquestador ante colisión
> (`GOAL-WORKORDERS.md:11`): la 119 queda reservada para la migración
> lp_fee de WO-04 (su mitad TS ya cita "el CHECK de la migración 119" —
> `WO-04-APPLY-TS.md:35`; `backend/api-server/src/routes/trading-config.ts:174`).
> Toda mención de "119" en el cuerpo de este diseño debe leerse "120".
> Detalle del aterrizaje: `WO-03-APPLY.md`.

Estado del gap verificado (informe §2.5 confirmado por lectura directa):

- `secret_value TEXT` en claro — `database/migrations/057_service_credentials.sql:47`
  (el comentario `057:14-18` prometía "migration 058 will add at-rest
  encryption via pgcrypto"; la 058 real es `058_token_validations.sql`, otra
  tabla — el cifrado NUNCA aterrizó).
- El TODO de cifrado vive también en `backend/api-server/src/credentials/store.ts:12-14`
  ("stored as plain TEXT for the MVP").
- Vault: SEALED (shamir 3/2), sin consumidor — el diseño NO depende de Vault.
- pgcrypto YA está habilitado en la base: `database/run_migrations.sh:52`
  (`CREATE EXTENSION IF NOT EXISTS pgcrypto`) y `database/migrations/001_roles.sql:8`.

Lexicón OMEGA: las credenciales cifradas aquí son claves de operador para
Variedades de Liquidez y oráculos (no TOPOLOGÍA de ejecución); ningún cambio
toca el hot-path de detección ni el terminus de ejecución (§32/§33 intactos).

---

## 0. Decisiones de diseño (resumen ejecutivo)

| # | Decisión | Justificación (evidencia) |
|---|----------|---------------------------|
| D1 | Cifrado **en-DB con pgcrypto** (`pgp_sym_encrypt`, AES-256, MDC), clave de fila derivada en TS | Charter manda pgcrypto; PGP packet provee IV/S2K random por llamada + tamper detection; TS no reimplementa PGP |
| D2 | Master key **solo en env/file del api-server** (`ARBX_CREDENTIALS_MASTER_KEY[_FILE]`), NUNCA en repo ni en SQL; **solo la clave de fila derivada cruza al DB** | Envelope real: compromiso de sesión PG expone ≤1 clave de fila, no el master |
| D3 | Derivación **por fila** (`HMAC-SHA256(master, "arbx-svc-cred-v1:" ‖ salt)`), no por tenant | El esquema real NO tiene columna tenant: tabla single-operator `(provider, scope)` única — `057_service_credentials.sql:56` |
| D4 | Migración `119` **estructural + backfill SQL OPCIONAL vía psql var**; backfill **garantizado** por barrido TS en boot (verificación de rowCount + roundtrip descifrado) | El runner re-aplica cada migración en cada deploy sin ledger (`database/run_migrations.sh:5-7`); psql `:'var'` NO interpola dentro de `$$` (precedente `001b_role_passwords.sql:20-22`); `database/run_migrations.sh` está FUERA del claim de este WO → el SQL de backfill se activa solo si el operador extiende el runner (diff propuesto §7) |
| D5 | Sin CHECK "single-source" en DB: el invariante `plaintext XOR envelope` lo exige el código y lo **asserta el boot** | La secuencia verificada cifrar→verificar→scrub necesita un estado transitorio ambos-activos (crash-safe, §5.3); un CHECK lo impediría |
| D6 | Degradación honesta SIN key: modo legacy (comportamiento idéntico al de hoy) con warn; **fail-fast (crash on boot, RULE 02)** si existen filas cifradas y no hay master key | Charter (3); `CLAUDE.md` RULE 02 "Si falta → Crash on Boot (es seguridad, no bug)" |
| D7 | `validators.ts` = **sin cambio funcional** (auditoría documentada §6.3): solo ve plaintext ya descifrado | `validators.ts:26-30` (firma recibe `secret: string`); el descifrado ocurre antes, en `store.ts` |
| D8 | `secret_hint TEXT` (sufijo enmascarado precalculado, ≤8 chars) para que el listado NUNCA descifre | El contrato público ya expone last-4 (`store.ts:24-28` + `shared-ts/src/contracts/credentials.ts:48-49`); descifrar para mostrar 4 chars expande superficie sin necesidad |

---

## 1. Esquema de cifrado envelope

### 1.1 Jerarquía de claves

```
Master key K  (≥32 chars, externa al DB)
  ├─ fuente A (opcional, Vault): ARBX_CREDENTIALS_MASTER_KEY_FILE  → archivo sink del vault-agent
  ├─ fuente B (env, VPS .env):   ARBX_CREDENTIALS_MASTER_KEY
  ├─ rotación: ARBX_CREDENTIALS_MASTER_KEY_PREV  (ventana de rotación)
  └─ ARBX_CREDENTIALS_KEY_VERSION (int ≥1, default 1) — versión de la key ACTUAL

Clave de fila RK(fila) = hex( HMAC-SHA256( K_version, "arbx-svc-cred-v1:" ‖ salt_fila ) )
  salt_fila = 16 bytes aleatorios por fila (crypto.randomBytes / gen_random_bytes(16))

Ciphertext(fila) = pgp_sym_encrypt(secret_plaintext, RK_hex,
                                    'cipher-algo=aes256, compress-algo=0')
```

- **K nunca cruza a Postgres.** Solo `RK_hex` viaja como bind parameter a
  `pgp_sym_encrypt`/`pgp_sym_decrypt`. Una sesión PG comprometida (o un
  `pg_stat_activity` curioso) ve a lo sumo la clave de UNA fila.
- `compress-algo=0`: secretos cortos, compresión sin beneficio y con side
  channels; `cipher-algo=aes256` (el default de pgp_sym_encrypt es aes128 —
  se fija explícitamente).
- El PGP packet de pgcrypto trae S2K salt + IV random por llamada y MDC
  (modification detection): manipulación del ciphertext ⇒ error al descifrar
  (fail-loud, R8).
- Derivación **por fila** (D3): el esquema real no tiene tenant —
  `057_service_credentials.sql:42-59` define `(provider, scope)` único global;
  la clave por fila garantiza además que filas idénticas producen ciphertexts
  independientes y que la rotación puede avanzar fila a fila.

### 1.2 Paridad TS ↔ SQL de la derivación (contrato bloqueado)

Ambas implementaciones DEBEN producir bytes idénticos:

- TS (`crypto.ts`): `createHmac("sha256", master).update("arbx-svc-cred-v1:").update(salt).digest("hex")`
- SQL (migración, Path A): `encode(hmac(convert_to('arbx-svc-cred-v1:','UTF8') || secret_salt, :'key'::bytea, 'sha256'), 'hex')`

Vector de paridad fijado en test (`crypto.test.ts`, vector sintético de test,
no material real): master `unit-test-master-key-32-chars-ok!!` (34 chars),
salt `hex 000102030405060708090a0b0c0d0e0f` ⇒
`b9a03be3109b0b76991d9d6750ae3410f0926f73208da1b53eac7c59f0ccb6ef`.
La paridad cruzada TS↔pgcrypto se verifica en runtime en cada boot
(fase verify del backfill, §5.2): un mismatch aborta el arranque.

Restricción documentada del material de la key: base64/hex/alfanumérico
(`openssl rand -base64 32`); SIN backslashes ni comillas (el cast
`:'key'::bytea` del Path A usa el formato escape de bytea).

### 1.3 Columnas nuevas (migración 119)

| Columna | Tipo | Semántica |
|---|---|---|
| `secret_ciphertext` | `BYTEA NULL` | envelope pgcrypto; presencia = fuente de verdad del secreto |
| `secret_salt` | `BYTEA NULL` | 16 bytes; set junto con ciphertext, siempre |
| `secret_key_version` | `SMALLINT NULL` | versión de master key que cifró la fila (rotación) |
| `secret_hint` | `TEXT NULL` | sufijo enmascarado público ("…abcd" / "****"), ≤8 chars — paridad EXACTA con `maskSuffix` (`store.ts:24-28`): `trim`, `len<=4 → "****"`, si no `"…" + last4` |

`secret_value TEXT` se CONSERVA (no se dropea — doctrina forward-only,
`database/migrations/MIGRATION_HISTORY.md` "Forward-only doctrine"): tras el
backfill queda `NULL` en filas convertidas; filas nuevas sin key (modo legacy)
siguen escribiéndolo. Nunca coexisten ambos tras un boot exitoso (D5, assert
§5.2d).

---

## 2. Migración `119_service_credentials_envelope_encryption.sql`

Ubicación: `database/migrations/119_service_credentials_envelope_encryption.sql`
(sin colisión: `ls database/migrations` llega a `118_opportunities_detected_at_breakdown_idx.sql`).

### 2.1 Requisitos del runner respetados

- **Idempotente** (re-aplicado en cada deploy, `run_migrations.sh:5-7,74-88`):
  ADD COLUMN catalog-guardados (`DO $$ … pg_attribute …`), CONSTRAINT
  catalog-guardado (`pg_constraint`), UPDATEs con guardas `WHERE
  secret_ciphertext IS NULL` (segunda pasada = 0 filas).
- **Rerun-lock-safety** (`automation/tools/lint-migration-rerun-lock-safety.sh:46`,
  HOT_TABLES = `opportunities|simulations|paper_trade_runs`):
  `service_credentials` NO es hot table (escritura solo por admin route), pero
  los ALTER van catalog-guardados igualmente por doctrina (GEN-CI-FAIL
  2026-08-30, `MIGRATION_HISTORY.md` colas).
- **ON_ERROR_STOP=1** (`run_migrations.sh:43`): cualquier error SQL aborta el
  deploy — la verificación de rowCount usa esto para fallar el deploy (§2.3).
- **FAIL-FAST sin key**: el archivo define la psql var vacía si el runner no la
  inyecta (`\if :{?…} \else \set … '' \endif`) — un `:'var'` sin definir
  abortaría CADA deploy; con fallback vacío el backfill SQL es no-op y el
  barrido TS (§5) es el que garantiza la conversión. psql≥10 requerido;
  el stack corre postgres:15 (`docker/compose.prod.yml:25`).

### 2.2 Estructura (aplicada siempre, sin key)

```sql
BEGIN;
DO $$ … ADD COLUMN secret_ciphertext BYTEA … $$;      -- catalog-guard
DO $$ … ADD COLUMN secret_salt       BYTEA … $$;
DO $$ … ADD COLUMN secret_key_version SMALLINT … $$;  -- NULLable (D3: set solo con envelope)
DO $$ … ADD COLUMN secret_hint       TEXT    … $$;
COMMIT;
-- (sin índice de barrido: service_credentials es una tabla fría de ~pocas
--  filas por diseño — un índice partial añadiría lock-surface sin beneficio)
```

### 2.3 Backfill SQL (Path A — OPCIONAL, se activa solo con psql var)

Solo ejecuta si el operador extiende `run_migrations.sh` (diff propuesto §7;
archivo fuera del claim de este WO). Sin esa extensión: no-op honesto.

```sql
\if :{?arbx_credentials_master_key}
\else
  \set arbx_credentials_master_key ''
\endif
-- (idem arbx_credentials_master_key_version, default 1)

-- (1) salts para filas plaintext sin envelope
UPDATE service_credentials
   SET secret_salt = gen_random_bytes(16)
 WHERE secret_value IS NOT NULL AND secret_value <> ''
   AND secret_ciphertext IS NULL AND secret_salt IS NULL
   AND :'arbx_credentials_master_key' <> '';

-- (2) envelope + hint + versión + scrub de plaintext
UPDATE service_credentials sc
   SET secret_ciphertext = pgp_sym_encrypt(
         sc.secret_value,
         encode(hmac(convert_to('arbx-svc-cred-v1:','UTF8') || sc.secret_salt,
                     :'arbx_credentials_master_key'::bytea, 'sha256'),'hex'),
         'cipher-algo=aes256, compress-algo=0'),
       secret_key_version = :'arbx_credentials_master_key_version'::int,
       secret_hint = CASE WHEN length(btrim(sc.secret_value)) <= 4 THEN '****'
                          ELSE '…' || right(btrim(sc.secret_value), 4) END,
       secret_value = NULL
 WHERE sc.secret_value IS NOT NULL AND sc.secret_value <> ''
   AND sc.secret_ciphertext IS NULL AND sc.secret_salt IS NOT NULL
   AND :'arbx_credentials_master_key' <> '';

-- (3) VERIFICACIÓN de rowCount — mismatch ⇒ deploy FAIL (ON_ERROR_STOP)
SELECT count(*) AS n_remaining FROM service_credentials
 WHERE secret_value IS NOT NULL AND secret_value <> ''
   AND secret_ciphertext IS NULL
   AND :'arbx_credentials_master_key' <> '';
\gset
\if :n_remaining = 0
\echo '119: backfill envelope verificado — 0 filas plaintext pendientes'
\else
\echo '119: FATAL — :n_remaining filas sin cifrar tras el backfill'
SELECT 1/0 AS backfill_verification_failed;
\endif

-- (4) VERIFICACIÓN roundtrip de descifrado (solo filas de la versión de la var;
--     tras una rotación las filas v-1 NO se tocan aquí — idempotencia futura)
SELECT count(*) AS n_undecryptable FROM service_credentials
 WHERE secret_ciphertext IS NOT NULL
   AND secret_key_version = :'arbx_credentials_master_key_version'::int
   AND pgp_sym_decrypt(secret_ciphertext,
       encode(hmac(convert_to('arbx-svc-cred-v1:','UTF8') || secret_salt,
                   :'arbx_credentials_master_key'::bytea,'sha256'),'hex')) IS NULL
   AND :'arbx_credentials_master_key' <> '';
\gset
\if :n_undecryptable = 0
\else
\echo '119: FATAL — filas indescifrables tras el backfill'
SELECT 1/0 AS backfill_roundtrip_failed;
\endif
```

Nota R8 sobre (4): `pgp_sym_decrypt` con clave errónea RAISE (no devuelve
NULL) — el `IS NULL` es formally-unreachable; el raise en sí aborta el deploy.
La cláusula existe para dejar la intención verificable y para el caso
"ciphertext corrupto" que pgcrypto reporta igualmente como error.

### 2.4 Backfill TS (Path B — GARANTIZADO, implementado en este WO)

`backfillCredentialEncryption(pool, logger)` en `credentials/crypto.ts`,
invocado al inicio de `rehydrateSvcCredMirror` (boot, `index.ts:1590`).
Detalle en §5.2. Verificaciones: rowCount por fase (=1 o aborta), roundtrip
descifrado == plaintext ANTES del scrub, y assert final de invariantes.

---

## 3. Consumidor Vault — OPCIONAL (hoy SEALED, shamir 3/2)

- **El diseño funciona SIN Vault**: fuente B (env) es suficiente y es el modo
  por defecto. Vault SEALED no bloquea nada.
- Integración SIN código nuevo: la fuente A (`ARBX_CREDENTIALS_MASTER_KEY_FILE`)
  ya está implementada; el vault-agent escribe la key al archivo y el
  api-server la lee en boot. Wiring propuesto (documentativo, placeholders):

```hcl
# /etc/vault.d/agent-credentials.hcl  (EJEMPLO — no existe aún; placeholders)
template {
  contents = "{{ with secret \"${VAULT_KV_PATH}\" }}{{ .Data.data.credentials_master_key }}{{ end }}"
  destination = "${ARBX_CREDENTIALS_MASTER_KEY_FILE}"   # p.ej. /vault/secrets/arbx-credentials-master-key
  perms       = "0400"
}
```

- Compose (propuesta, FUERA de claim): `ARBX_CREDENTIALS_MASTER_KEY_FILE:
  ${ARBX_CREDENTIALS_MASTER_KEY_FILE:-}` en el servicio api-server.
- Misconfig honesta: archivo configurado pero ausente ⇒ `CredentialsKeyRequiredError`
  en boot (fail-fast, no fallback silencioso a env).
- Futuro NO implementado (documentado): modo transit-engine (envelope DEK
  wrapped por Vault) — requiere Vault unsealed + consumidor real; fuera del
  alcance de este WO.

---

## 4. Touchpoints — diffs propuestos

### 4.1 `backend/api-server/src/credentials/crypto.ts` (NUEVO, dentro del claim-dir)

Key resolution + derivación + encrypt/decrypt via pgcrypto + backfill +
errores tipados (`CredentialsKeyRequiredError`, `CredentialsDecryptError`,
`CredentialsBackfillError`). Ver §5 para el contrato completo.

### 4.2 `backend/api-server/src/credentials/store.ts`

| Zona | Cambio |
|---|---|
| `DbRow` (store.ts:30-42) | + `secret_ciphertext: Buffer \| null`, `secret_salt`, `secret_key_version: number \| null`, `secret_hint: string \| null` |
| `rowToPublic` (store.ts:44-59) | `has_value = (secret_value no-vacío) OR secret_ciphertext IS NOT NULL`; `value_suffix = secret_hint ?? maskSuffix(secret_value)` (legacy) |
| `listCredentials` (store.ts:64-72) | SELECT agrega las 4 columnas; NUNCA descifra (D8) |
| `readCredentialSecret` (store.ts:79-93) | Si `secret_ciphertext` → resolve key (fail-fast si falta) → `pgp_sym_decrypt`; si no, legacy |
| `readCredentialForBulk` (store.ts:120-146) | Ídem; la interfaz `StoredCredentialRow` NO cambia (routes/tests intactos) |
| `upsertCredential` (store.ts:153-182) | Con key + secret nuevo: cifra (RK derivada en TS, `pgp_sym_encrypt` en DB) y escribe envelope + hint, `secret_value=NULL`. Sin key: escritura legacy idéntica a hoy (el modo queda advertido UNA vez en boot con `credentials.encryption_disabled`; la función no recibe logger y la tabla es fría — R9). Metadata-only refresh (`secret_value=null`): conserva la fuente existente (CASE en ON CONFLICT) |

### 4.3 `backend/api-server/src/credentials/projection.ts`

- `rehydrateSvcCredMirror` (projection.ts:91-143): (a) ejecuta
  `backfillCredentialEncryption` primero (crash-safe, idempotente); (b) el
  SELECT trae columnas envelope y descifra las filas cifradas antes de
  proyectar (la proyección Redis lleva el secreto RAW por contrato,
  projection.ts:10-13); (c) si hay filas cifradas y NO hay key ⇒ log error +
  **rethrow** (boot crash vía unhandled rejection en Node 20 —
  `backend/api-server/Dockerfile:12` `node:20-bookworm-slim`, default
  `--unhandled-rejections=throw`). Cambio de contrato documentado: "never
  throws" pasa a ser "never throws EXCEPTO key-required-con-ciphertext"
  (security fail-fast, RULE 02).

### 4.4 `backend/api-server/src/credentials/validators.ts` — SIN cambio funcional (D7)

Evidencia: los validators reciben `(scope, secret, metadata)` ya en plaintext
(validators.ts:26-30); el descifrado ocurre aguas arriba en `store.ts` antes
de toda llamada (routes pasan el secret recién enviado o recién descifrado —
`routes/credentials.ts:157,178,291,324`). Touchpoint aplicado: comentario de
contrato en el header del módulo (plaintext-only) — sin lógica nueva
(simplicidad §2: un guard anti-ciphertext sería teatro: TS nunca ve el
ciphertext como string).

### 4.5 Archivos NO tocados (fuera de claim) — diffs propuestos solo como documentación

- `database/run_migrations.sh` (§7): inyección opcional de las psql vars.
- `docker/compose.dev.yml` / `docker/compose.prod.yml` (§7): env vars al
  api-server. OJO RULE 03/04: `--env-file .env` explícito (no afecta
  `NEXT_PUBLIC_*`; el api-server lee env en runtime, no build-time).
- `.env` (VPS, gitignored): material real de key — NUNCA en repo.

---

## 5. Contrato runtime del api-server (fail-fast)

### 5.1 Resolución de key (boot/lazy, memoizada por proceso)

1. `ARBX_CREDENTIALS_MASTER_KEY_FILE` definido ⇒ leer archivo (utf8, trim).
   Archivo ausente/vacío ⇒ **throw** (misconfig ruidosa).
2. Si no, `ARBX_CREDENTIALS_MASTER_KEY` (trim). Vacío ⇒ **modo legacy** (ausente).
3. Presente pero <32 chars ⇒ **throw** (half-config = peligroso).
4. `ARBX_CREDENTIALS_MASTER_KEY_PREV` (opcional, ventana de rotación) y
   `ARBX_CREDENTIALS_KEY_VERSION` (int ≥1, default 1).

### 5.2 Barrido de boot (`backfillCredentialEncryption`)

- a. Sin key ⇒ si `count(secret_ciphertext IS NOT NULL) > 0` ⇒
  `CredentialsKeyRequiredError` (crash on boot); si 0 ⇒ return (legacy, warn).
- b. Filas "plaintext sin envelope": **fase 1** cifrar (mantiene plaintext):
  `UPDATE … SET ciphertext/salt/version/hint WHERE id=$1 AND secret_ciphertext
  IS NULL` — rowCount≠1 ⇒ aborta; **fase 2** verify: `pgp_sym_decrypt(ct,
  RK)` == plaintext en memoria (mismatch ⇒ aborta, plaintext INTACTO);
  **fase 3** scrub: `UPDATE … SET secret_value=NULL WHERE id=$1 AND
  secret_ciphertext IS NOT NULL` — rowCount≠1 ⇒ aborta. Crash entre fases ⇒
  estado ambos-set ⇒ la siguiente boot lo re-para (verify+scrub) — crash-safe.
- c. Filas versión < current con PREV disponible: descifrar con PREV →
  re-cifrar con current (nuevo salt) → verify → rowCount guard
  `WHERE secret_key_version = <old>` (optimista). Single-phase (overwrite):
  el runbook exige pg_dump previo (§8).
- d. Assert final: `count(plaintext AND ciphertext) == 0` y log resumen
  `credentials.backfill_complete {converted, rotated, scrubbed, failed}`
  (R9: summary único a info, no per-fila).
- e. Migración 119 recién desplegada sin key ⇒ (a) legacy ⇒ CERO cambio de
  comportamiento (deploy seguro, adopción en 2 pasos).

### 5.3 Lectura/escritura

- Lectura de secreto (`readCredentialSecret`, `readCredentialForBulk`):
  envelope presente ⇒ descifrar (sin key ⇒ throw). Legacy plaintext ⇒ como hoy.
- Escritura: §4.2. El noop-detection por byte-compare del bulk
  (`routes/credentials.ts:167`) sigue correcto: compara plaintext==plaintext.
- `mirrorAfterWrite` (`routes/credentials.ts:59-76`) sin cambios: consume
  `readCredentialForBulk` (ya decrypt-aware).

### 5.4 Invariante §33.1.3

Este WO NO toca `arbx:opps:detected` (solo `arbx:svc_cred:*` por rutas admin
preexistentes). Verificación post-deploy sugerida: `XLEN arbx:opps:detected`
delta 0.

---

## 6. Plan de ROTACIÓN

1. `pg_dump` de `service_credentials` (gate del runbook).
2. .env VPS: `ARBX_CREDENTIALS_MASTER_KEY_PREV=<K1>` (la actual),
   `ARBX_CREDENTIALS_MASTER_KEY=<K2 nueva>`, `ARBX_CREDENTIALS_KEY_VERSION=2`.
3. `docker compose --env-file .env -f docker/compose.<env>.yml up -d api-server`
   (env runtime; sin rebuild — no hay NEXT_PUBLIC_* involucrados).
4. Boot: barrido (c) re-cifra todas las filas v1→v2 y proyecta a Redis.
5. Verificar: `SELECT DISTINCT secret_key_version FROM service_credentials`
   ⇒ solo `2`; log `credentials.backfill_complete.rotated`.
6. Cerrar ventana: eliminar `…_PREV` del .env y reiniciar (las filas v2 ya no
   la necesitan). Filas v1 halladas después sin PREV ⇒
   `CredentialsDecryptError: unsupported_key_version` (fail-honest).

---

## 7. Plan de ROLLBACK + diffs out-of-claim

- **Pre-activación** (migración aplicada, sin key): rollback = redeploy del
  SHA anterior; las columnas nuevas quedan (forward-only) y son inertes.
- **Post-backfill** (ciphertext existe): restauración a plaintext SOLO con la
  master key (SQL de restauración documentado en el runbook WO-14 o §5.2 del
  APPLY); sin key ⇒ restaurar el pg_dump del paso 1 de rotación / pre-cutover.
- **Pérdida de la master key** = filas indescifrables (documentado, honesto:
  es la propiedad del cifrado).

Diffs propuestos (NO aplicados — fuera de claim):

```bash
# database/run_migrations.sh — dentro de run_file(), tras los -v existentes (línea ~47):
    -v arbx_credentials_master_key="${ARBX_CREDENTIALS_MASTER_KEY:-}" \
    -v arbx_credentials_master_key_version="${ARBX_CREDENTIALS_KEY_VERSION:-1}" \
# (la key viaja en argv de docker exec — visible en /proc del host; documentado
#  como trade-off del Path A; el Path B nunca la saca del proceso api-server)
```

```yaml
# docker/compose.{dev,prod}.yml — servicio api-server, environment:
      ARBX_CREDENTIALS_MASTER_KEY: ${ARBX_CREDENTIALS_MASTER_KEY:-}
      ARBX_CREDENTIALS_MASTER_KEY_PREV: ${ARBX_CREDENTIALS_MASTER_KEY_PREV:-}
      ARBX_CREDENTIALS_KEY_VERSION: ${ARBX_CREDENTIALS_KEY_VERSION:-1}
      ARBX_CREDENTIALS_MASTER_KEY_FILE: ${ARBX_CREDENTIALS_MASTER_KEY_FILE:-}
```

---

## 8. Amenazas cubiertas / residuales

| Amenaza | Mitigación |
|---|---|
| Dump de PG robado | Ciphertext AES-256 + clave por fila; master key no está en el DB |
| Compromiso de sesión/consultas PG | Solo RK de una fila cruza como bind param |
| Master key perdida | Rotación con PREV; si no hay copia ⇒ indescifrable (documentado) |
| Tamper de ciphertext | MDC de PGP ⇒ error al descifrar (fail-loud) |
| Backfill a medias (crash) | Fases idempotentes + reparación en siguiente boot (§5.2b) |
| Deploy sin .env de key | Modo legacy idéntico a hoy (honesto, warn por escritura) |
| Ciphertext presente sin key | Crash on boot (RULE 02) — nunca servir vacío silencioso |

---

## 9. Entregables de este WO

- `audits/omniscience-integration-2026-09-06/WO-03-DESIGN.md` (este documento).
- APPLIED (kind: apply): `credentials/crypto.ts` (nuevo), `credentials/store.ts`,
  `credentials/projection.ts`, `credentials/validators.ts` (doc-only),
  `database/migrations/119_service_credentials_envelope_encryption.sql`,
  `database/migrations/MIGRATION_HISTORY.md` (sección 119),
  `credentials/crypto.test.ts` (+ proyección con filas cifradas).
- Verificación: `tsc --noEmit` + `vitest run` (credentials suite) — ver
  `WO-03-APPLY.md`. CERO cargo/target (capa TS). CERO git write. CERO VPS
  mutation. CERO request a dominio público.
