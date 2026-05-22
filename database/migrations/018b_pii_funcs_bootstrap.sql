-- ArbitrageX v2 — Migration 018b: bootstrap PII helper functions
--
-- Forward-only fix for a migration-ORDERING bug surfaced when the full chain is
-- applied in lexical order (e.g. CI e2e via automation/scripts/migrate.sh):
--
--   019_audit_log_partitions.sql migrates audit_log data through
--     arbx_anonymize_ip(ip_address)        -- ip_address is INET
--     arbx_hash_user_agent(user_agent)     -- user_agent is TEXT
--   but those helpers were not defined until 053_audit_pii_hardening.sql (TEXT
--   signature). On a fresh DB, 019 fails to plan: "function
--   arbx_anonymize_ip(inet) does not exist".
--
-- This migration (sorts BEFORE 019) defines the helpers 019 needs, idempotently.
-- It does NOT edit the historical migrations. 053 later runs CREATE OR REPLACE
-- on arbx_hash_user_agent(text) (identical body → no-op) and adds the canonical
-- arbx_anonymize_ip(TEXT) overload; both coexist with the INET overload here.
-- Safe on an already-migrated DB: CREATE OR REPLACE is idempotent.

BEGIN;

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- INET overload required by 019 (column audit_log.ip_address is INET).
-- Truncates to /24 (IPv4) or /48 (IPv6); returns INET so it inserts directly.
CREATE OR REPLACE FUNCTION arbx_anonymize_ip(ip INET)
RETURNS INET
LANGUAGE plpgsql
IMMUTABLE
AS $$
BEGIN
    IF ip IS NULL THEN
        RETURN NULL;
    END IF;
    IF family(ip) = 4 THEN
        RETURN network(set_masklen(ip, 24))::inet;
    ELSE
        RETURN network(set_masklen(ip, 48))::inet;
    END IF;
END;
$$;

-- TEXT helper required by 019; canonical definition mirrored from 053 so the
-- two are byte-identical (053's CREATE OR REPLACE becomes a no-op).
CREATE OR REPLACE FUNCTION arbx_hash_user_agent(ua TEXT)
RETURNS TEXT
LANGUAGE plpgsql
IMMUTABLE
AS $$
BEGIN
    IF ua IS NULL OR ua = '' THEN
        RETURN NULL;
    END IF;
    RETURN 'sha256:' || encode(digest(ua, 'sha256'), 'hex')::text;
END;
$$;

COMMIT;
