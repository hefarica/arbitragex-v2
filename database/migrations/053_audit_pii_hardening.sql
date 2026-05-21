-- ============================================================================
-- Migration 053: PII hardening on audit_log
--
-- Per audit 2026-05-10 finding M7: audit_log stores raw IP + User-Agent
-- for 90 days. GDPR / data minimization: hash UA, truncate IP to /24
-- (IPv4) or /48 (IPv6).
--
-- Strategy: don't backfill historical rows (operator's choice). New rows
-- inserted via api-server should use these helpers.
-- ============================================================================

BEGIN;

-- Requires pgcrypto extension for digest()
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Helper: truncate IP to /24 (IPv4) or /48 (IPv6).
-- Returns the network portion as text. Original IP not stored anywhere
-- after this transformation.
CREATE OR REPLACE FUNCTION arbx_anonymize_ip(ip TEXT)
RETURNS TEXT
LANGUAGE plpgsql
IMMUTABLE
AS $$
DECLARE
    parsed INET;
BEGIN
    IF ip IS NULL OR ip = '' THEN
        RETURN NULL;
    END IF;
    BEGIN
        parsed := ip::INET;
    EXCEPTION WHEN OTHERS THEN
        RETURN '<malformed>';
    END;

    IF family(parsed) = 4 THEN
        -- /24: keep first 3 octets
        RETURN host(network(set_masklen(parsed, 24))) || '/24';
    ELSE
        -- /48: keep first 3 hextets
        RETURN host(network(set_masklen(parsed, 48))) || '/48';
    END IF;
END;
$$;

-- Helper: hash User-Agent with sha256 prefix (12 hex chars = 48 bits, plenty
-- to distinguish browsers without identifying individual instances).
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

-- Sanity verification
DO $$
BEGIN
    -- IPv4 /24
    IF arbx_anonymize_ip('192.168.1.42') != '192.168.1.0/24' THEN
        RAISE EXCEPTION 'IPv4 anonymization broken';
    END IF;
    -- IPv6 /48
    IF arbx_anonymize_ip('2001:db8::1') NOT LIKE '%/48' THEN
        RAISE EXCEPTION 'IPv6 anonymization broken';
    END IF;
    -- NULL
    IF arbx_anonymize_ip(NULL) IS NOT NULL THEN
        RAISE EXCEPTION 'NULL handling broken';
    END IF;
    -- Malformed
    IF arbx_anonymize_ip('not-an-ip') != '<malformed>' THEN
        RAISE EXCEPTION 'Malformed handling broken';
    END IF;
    -- UA
    IF arbx_hash_user_agent('Mozilla/5.0') NOT LIKE 'sha256:%' THEN
        RAISE EXCEPTION 'UA hashing broken';
    END IF;

    RAISE NOTICE 'Migration 053: PII helpers verified (anonymize_ip + hash_user_agent)';
END $$;

-- The api-server's audit_log INSERT should use these helpers via UPDATE wrapper:
-- INSERT INTO audit_log (..., ip_address, user_agent) VALUES (
--     ..., arbx_anonymize_ip($1), arbx_hash_user_agent($2)
-- );
-- Operator action: update INSERT statements in api-server/src/index.ts
-- (audit hooks) to call these helpers. See M7 follow-up note.

COMMIT;
