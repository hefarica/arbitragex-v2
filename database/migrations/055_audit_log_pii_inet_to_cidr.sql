-- ============================================================================
-- Migration 055: cable PII helpers into audit_log INSERT (audit N2, 2026-05-10)
--
-- Migration 053 added arbx_anonymize_ip() returning TEXT with CIDR suffix
-- (e.g., "192.168.1.0/24") and arbx_hash_user_agent() returning TEXT
-- ("sha256:<hex>"). The api-server INSERT in backend/api-server/src/index.ts
-- never wired them in — raw IP (INET) and raw UA (TEXT) were still being
-- stored, producing a false GDPR sense.
--
-- This migration:
--   1. Converts audit_log.ip_address from INET to CIDR (postgres'
--      anonymized-network type), so the helper output ('a.b.c.0/24') is
--      directly assignable.
--   2. Retroactively anonymizes existing rows in audit_log:
--        - ip_address → /24 (IPv4) or /48 (IPv6) network portion
--        - user_agent → arbx_hash_user_agent(user_agent)
--      Destructive on historical UA data; irreversible by audit policy.
--   3. Companion change in api-server (same commit): wraps INSERT params
--      with arbx_anonymize_ip($7)::cidr and arbx_hash_user_agent($N).
--
-- IDEMPOTENCY: column-type checks via information_schema.columns; UPDATE
-- guards against re-hashing already-hashed user_agent values.
-- ============================================================================

BEGIN;

-- ---------------------------------------------------------------------------
-- Step 1: ip_address INET → CIDR (anonymize existing rows in-place first).
-- ---------------------------------------------------------------------------
DO $$
DECLARE
    v_anonymized BIGINT := 0;
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name = 'audit_log'
           AND column_name = 'ip_address'
           AND udt_name = 'inet'
    ) THEN
        -- Anonymize in-place to /24 (IPv4) or /48 (IPv6) before changing type.
        -- network(set_masklen(...)) returns the CIDR-coerced network address
        -- which still casts back to INET (host bits zeroed) for this UPDATE.
        EXECUTE $sql$
            UPDATE audit_log
               SET ip_address = network(set_masklen(ip_address,
                     CASE WHEN family(ip_address) = 4 THEN 24 ELSE 48 END))
             WHERE ip_address IS NOT NULL
        $sql$;
        GET DIAGNOSTICS v_anonymized = ROW_COUNT;
        RAISE NOTICE 'Migration 055: anonymized % existing audit_log.ip_address rows in-place', v_anonymized;

        -- ALTER TYPE on the partitioned parent propagates to all partitions in PG 12+.
        EXECUTE 'ALTER TABLE audit_log ALTER COLUMN ip_address TYPE cidr USING ip_address::cidr';
        RAISE NOTICE 'Migration 055: audit_log.ip_address converted INET → CIDR';
    ELSE
        RAISE NOTICE 'Migration 055: audit_log.ip_address is not INET (already migrated or absent), skipping';
    END IF;
END $$;

-- ---------------------------------------------------------------------------
-- Step 2: user_agent → arbx_hash_user_agent(user_agent) for existing rows.
-- Guarded by NOT LIKE 'sha256:%' so re-runs are no-ops.
-- ---------------------------------------------------------------------------
DO $$
DECLARE
    v_hashed BIGINT := 0;
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name = 'audit_log'
           AND column_name = 'user_agent'
    ) THEN
        EXECUTE $sql$
            UPDATE audit_log
               SET user_agent = arbx_hash_user_agent(user_agent)
             WHERE user_agent IS NOT NULL
               AND user_agent NOT LIKE 'sha256:%'
        $sql$;
        GET DIAGNOSTICS v_hashed = ROW_COUNT;
        RAISE NOTICE 'Migration 055: hashed % existing audit_log.user_agent rows', v_hashed;
    ELSE
        RAISE NOTICE 'Migration 055: audit_log.user_agent column absent, skipping';
    END IF;
END $$;

-- ---------------------------------------------------------------------------
-- Step 3: Final sanity log
-- ---------------------------------------------------------------------------
DO $$
DECLARE
    v_total BIGINT;
BEGIN
    SELECT COUNT(*) INTO v_total FROM audit_log;
    RAISE NOTICE 'Migration 055: audit_log PII hardened across % total rows; ip_address now CIDR, user_agent now sha256-prefixed', v_total;
END $$;

COMMIT;
