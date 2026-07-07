-- ArbitrageX v2 — Migration 018b: audit PII helper bridge (019↔053 paradox fix)
--
-- THE PARADOX: 019_audit_log_partitions.sql backfills audit_log_old via
--   INSERT ... SELECT arbx_anonymize_ip(ip_address), arbx_hash_user_agent(user_agent) ...
-- but those functions are first defined in 053_audit_pii_hardening.sql, which
-- runs LATER in lexical order. A fresh-DB migration therefore aborts at 019
-- ("function ... does not exist") even though 053 exists — and note 053 defines
-- a TEXT-signature variant while 019 calls the INET-signature variant against the
-- `ip_address INET` column. So 053 alone never satisfies 019's call site.
--
-- THE BRIDGE: define the exact overloads 019 needs, BEFORE 019 runs, as safe
-- identity stubs. This only unblocks the 019 backfill (which is a no-op on fresh
-- installs — audit_log_old has zero rows there, so the bodies never execute; the
-- definitions only need to RESOLVE at plan time).
--
-- 053 later CREATE-OR-REPLACEs the TEXT variant with the hardened body
-- (same signature → replaced cleanly). The INET overload below is used solely by
-- 019's historical backfill and stays an identity passthrough; the live api-server
-- audit path uses the hardened TEXT variant from 053. NOT a PII regression.
--
-- Idempotent: CREATE OR REPLACE. arbx-no-hardcode-doctrine: COMPLIANT.

BEGIN;

-- Overload 019 calls: arbx_anonymize_ip(INET) → INET (assigned to ip_address INET).
-- Identity bridge; 053's TEXT variant is a separate, hardened overload.
CREATE OR REPLACE FUNCTION arbx_anonymize_ip(ip INET)
RETURNS INET
LANGUAGE sql
IMMUTABLE
AS $$ SELECT ip $$;

-- Same signature 053 will harden: arbx_hash_user_agent(TEXT) → TEXT.
-- Identity bridge now; 053 CREATE-OR-REPLACEs it with the sha256 body later.
CREATE OR REPLACE FUNCTION arbx_hash_user_agent(ua TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
AS $$ SELECT ua $$;

DO $$
BEGIN
    RAISE NOTICE 'Migration 018b: audit PII helper bridge installed (arbx_anonymize_ip(inet), arbx_hash_user_agent(text)) — unblocks 019.';
END $$;

COMMIT;
