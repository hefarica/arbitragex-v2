-- ArbitrageX v2 — Migration 120: envelope encryption for service_credentials.
-- WO-03 (2026-09-06) — design: audits/omniscience-integration-2026-09-06/WO-03-DESIGN.md
-- Numbered 120 (not 119) per orchestrator collision ruling: 119 is reserved
-- for the WO-04 trading-config lp_fee migration (GOAL-WORKORDERS.md:11).
--
-- Closes the §2.5 gap: secret_value TEXT has lived in cleartext since
-- migration 057 (the "058 will add pgcrypto" promise in 057:14-18 never
-- landed — 058 is token_validations, a different table).
--
-- Scheme (envelope, master key EXTERNAL to the DB — arbx-no-hardcode-doctrine):
--   salt(row)  = gen_random_bytes(16)                    [this migration / TS]
--   RK(row)    = hex( HMAC-SHA256(master, 'arbx-svc-cred-v1:' || salt) )
--   ciphertext = pgp_sym_encrypt(secret, RK_hex, 'cipher-algo=aes256, compress-algo=0')
--
--   Only RK crosses into Postgres. The master key NEVER does unless the
--   operator explicitly activates the optional SQL backfill below (Path A).
--
-- Structure (this file) is keyless and always applied. The SQL backfill is
-- gated on the psql variable arbx_credentials_master_key, which
-- database/run_migrations.sh does NOT inject today (extending it is a
-- proposed one-line diff in the design doc §7 — out of this WO's claim).
-- Without the variable this file is a pure no-op beyond adding columns, and
-- the api-server boot sweep (credentials/crypto.ts backfillCredentialEncryption)
-- performs the verified conversion instead (Path B, guaranteed path).
--
-- Idempotency (run_migrations.sh re-applies EVERY file on EVERY deploy):
--   - ADD COLUMN: catalog-guarded DO blocks (rerun-lock-safety doctrine;
--     service_credentials is NOT in the lint hot-table list, but the guard
--     is applied anyway — GEN-CI-FAIL retrofit 2026-08-30).
--   - Backfill UPDATEs: WHERE guards make re-runs match 0 rows.
--   - psql :var does NOT interpolate inside $$ blocks (001b precedent) —
--     every statement referencing :'var' is PLAIN SQL.
--   - The var is defaulted to '' here when the runner does not inject it, so
--     an undefined :'var' can never abort an unextended runner's deploy.
--
-- Key material constraint (Path A): the master key must be base64/hex/
-- alphanumeric (openssl rand -base64 32) — no backslashes or quotes, because
-- :'var'::bytea parses the escape format. Database must be UTF8 (docker
-- postgres:15 default) for the '…' ellipsis in secret_hint parity.
--
-- No "single-source" CHECK (plaintext XOR envelope) is added ON PURPOSE
-- (design D5): the verified scrub sequence (encrypt → verify → scrub) needs a
-- transient both-set state; the invariant is enforced by the api-server code
-- and asserted at every boot.
--
-- Reversibility:
--   Columns are additive; dropping them restores the 057 shape. Encrypted
--   rows can be restored to plaintext ONLY with the master key (see design
--   §7 rollback SQL) — losing the key loses the secrets (documented).

-- ─── psql variable fallbacks (never fail an unextended runner) ─────────────
\if :{?arbx_credentials_master_key}
\else
\set arbx_credentials_master_key ''
\endif
\if :{?arbx_credentials_master_key_version}
\else
\set arbx_credentials_master_key_version 1
\endif

BEGIN;

-- ─── 1. Envelope columns (catalog-guarded, idempotent) ─────────────────────
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_attribute
     WHERE attrelid = 'service_credentials'::regclass
       AND attname = 'secret_ciphertext' AND NOT attisdropped
  ) THEN
    EXECUTE 'ALTER TABLE service_credentials ADD COLUMN secret_ciphertext BYTEA';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_attribute
     WHERE attrelid = 'service_credentials'::regclass
       AND attname = 'secret_salt' AND NOT attisdropped
  ) THEN
    EXECUTE 'ALTER TABLE service_credentials ADD COLUMN secret_salt BYTEA';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_attribute
     WHERE attrelid = 'service_credentials'::regclass
       AND attname = 'secret_key_version' AND NOT attisdropped
  ) THEN
    EXECUTE 'ALTER TABLE service_credentials ADD COLUMN secret_key_version SMALLINT';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_attribute
     WHERE attrelid = 'service_credentials'::regclass
       AND attname = 'secret_hint' AND NOT attisdropped
  ) THEN
    EXECUTE 'ALTER TABLE service_credentials ADD COLUMN secret_hint TEXT';
  END IF;
END $$;

COMMENT ON COLUMN service_credentials.secret_ciphertext IS
  'WO-03: pgp_sym_encrypt envelope (AES-256). Presence = authoritative secret source; secret_value is then NULL.';
COMMENT ON COLUMN service_credentials.secret_salt IS
  'WO-03: 16-byte per-row salt for HMAC-SHA256 row-key derivation. Always set together with secret_ciphertext.';
COMMENT ON COLUMN service_credentials.secret_key_version IS
  'WO-03: master-key version that produced this envelope (rotation window support; NULL on legacy rows).';
COMMENT ON COLUMN service_credentials.secret_hint IS
  'WO-03: precomputed public masked suffix ("…abcd"/"****", <=8 chars) — parity with crypto.ts maskHint(); lets the list endpoint avoid decrypting.';

COMMIT;

-- ─── 2. OPTIONAL SQL backfill (Path A) — active ONLY when the runner injects
--       -v arbx_credentials_master_key=... (design §7). Otherwise no-op and
--       the api-server boot sweep does the verified conversion (Path B).
--       Plain SQL only: :'var' does not interpolate inside $$ blocks.

-- (2a) salts for rows that still carry plaintext and have no envelope
UPDATE service_credentials
   SET secret_salt = gen_random_bytes(16)
 WHERE secret_value IS NOT NULL AND secret_value <> ''
   AND secret_ciphertext IS NULL
   AND secret_salt IS NULL
   AND :'arbx_credentials_master_key' <> '';

-- (2b) envelope encrypt + hint + version + scrub plaintext.
--      Row-key derivation MUST stay byte-identical with credentials/crypto.ts
--      deriveRowKeyHex(): HMAC-SHA256(master, 'arbx-svc-cred-v1:' || salt) hex.
UPDATE service_credentials sc
   SET secret_ciphertext = pgp_sym_encrypt(
         sc.secret_value,
         encode(hmac(
           convert_to('arbx-svc-cred-v1:', 'UTF8') || sc.secret_salt,
           :'arbx_credentials_master_key'::bytea,
           'sha256'), 'hex'),
         'cipher-algo=aes256, compress-algo=0'),
       secret_key_version = :'arbx_credentials_master_key_version'::int,
       secret_hint = CASE
         WHEN length(btrim(sc.secret_value)) <= 4 THEN '****'
         ELSE '…' || right(btrim(sc.secret_value), 4)
       END,
       secret_value = NULL
 WHERE sc.secret_value IS NOT NULL AND sc.secret_value <> ''
   AND sc.secret_ciphertext IS NULL
   AND sc.secret_salt IS NOT NULL
   AND :'arbx_credentials_master_key' <> '';

-- ─── 3. rowCount VERIFICATION — any mismatch FAILS the deploy (ON_ERROR_STOP)
-- WO-03 fix (2026-09-07): psql \if does NOT evaluate SQL expressions ("0 = 0"
-- → "Boolean expected", condition falls to FALSE and the abort branch runs
-- unconditionally — caught by CI integration with live Postgres). Compare in
-- SQL and branch on the resulting 0/1 integer, which \if accepts.
SELECT (count(*) = 0)::int AS remaining_ok, count(*) AS n_remaining
  FROM service_credentials
 WHERE secret_value IS NOT NULL AND secret_value <> ''
   AND secret_ciphertext IS NULL
   AND :'arbx_credentials_master_key' <> '';
\gset
\if :remaining_ok
\echo '120: envelope backfill verified — :n_remaining plaintext rows pending'
\else
\echo '120: FATAL — :n_remaining rows failed to encrypt; aborting deploy'
SELECT 1/0 AS backfill_verification_failed;
\endif

-- ─── 4. Roundtrip VERIFICATION (decrypt what we just encrypted).
--       A wrong key raises inside pgcrypto → ON_ERROR_STOP aborts the deploy.
--       Restricted to the var's version so a later re-run after a rotation
--       (rows already at v2, var still v1) stays a no-op instead of failing.
SELECT (count(*) = 0)::int AS roundtrip_ok, count(*) AS n_undecryptable
  FROM service_credentials
 WHERE secret_ciphertext IS NOT NULL
   AND secret_key_version = :'arbx_credentials_master_key_version'::int
   AND pgp_sym_decrypt(secret_ciphertext,
       encode(hmac(
         convert_to('arbx-svc-cred-v1:', 'UTF8') || secret_salt,
         :'arbx_credentials_master_key'::bytea,
         'sha256'), 'hex')) IS NULL
   AND :'arbx_credentials_master_key' <> '';
\gset
\if :roundtrip_ok
\echo '120: decrypt roundtrip verified for version :'arbx_credentials_master_key_version'
\else
\echo '120: FATAL — :n_undecryptable encrypted rows failed to decrypt; aborting deploy'
SELECT 1/0 AS backfill_roundtrip_failed;
\endif
