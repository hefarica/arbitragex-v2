-- =====================================================================
-- OMEGA-8 / M3 Migration 070 — audit_event PII hardening (Capa 2)
-- =====================================================================
-- Forward-only · idempotent · safe.
--
-- Cierra el hallazgo P1-3 del audit Capa 2:
--   • Las funciones `arbx_anonymize_ip(text) → text` y
--     `arbx_hash_user_agent(text) → text` (mig 053) están definidas pero
--     NO se aplican a `audit_event` (mig 066). El audit retroactivo 055
--     sólo cubrió `audit_log`.
--
--   • Esta migración:
--     1. Anonimiza retroactivamente todos los rows existentes en
--        `audit_event` cuyas columnas `ip_address` o `user_agent` aún
--        contengan PII en claro.
--     2. Convierte `audit_event.ip_address` de INET a CIDR (consistente
--        con `audit_log.ip_address` post-055).
--     3. Documenta la columna `user_agent` como hash sha256 — el INSERT en
--        el código (registry-engine.ts) se actualiza por separado para
--        envolver con `arbx_hash_user_agent($)` desde la aplicación.
--
-- Idempotencia: DO blocks + IF NOT EXISTS + skip-if-already-converted.
--
-- Rollback manual: NO automático (INET es subconjunto de CIDR; revertir
-- requiere CAST inverso y desanonimizar IPs — imposible por diseño).
-- Para escenarios de desarrollo, eliminar la tabla audit_event y recrear
-- desde 066 + 070.
-- =====================================================================

BEGIN;

-- 1. Anonimizar retroactivamente filas que aún tengan IP completa.
--    Cualquier row con `family(ip_address) = 4` y máscara /32 (o /128 IPv6)
--    se considera "en claro"; lo reemplazamos por la versión /24 (/48).
DO $$
DECLARE
    v_anonymized INTEGER := 0;
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name = 'audit_event'
           AND column_name = 'ip_address'
           AND data_type = 'inet'
    ) THEN
        UPDATE audit_event
           SET ip_address = arbx_anonymize_ip(host(ip_address))::cidr
         WHERE ip_address IS NOT NULL
           AND (
                (family(ip_address) = 4 AND masklen(ip_address) = 32) OR
                (family(ip_address) = 6 AND masklen(ip_address) = 128)
           );
        GET DIAGNOSTICS v_anonymized = ROW_COUNT;
        RAISE NOTICE 'Migration 070: anonymized % existing audit_event.ip_address rows in-place', v_anonymized;

        EXECUTE 'ALTER TABLE audit_event ALTER COLUMN ip_address TYPE cidr USING ip_address::cidr';
        RAISE NOTICE 'Migration 070: audit_event.ip_address converted INET → CIDR';
    ELSE
        RAISE NOTICE 'Migration 070: audit_event.ip_address is not INET (already migrated or absent), skipping';
    END IF;
END$$;

-- 2. Retroactivamente hashear user_agent crudos (heurística: si NO empieza
--    con 'sha256:', es UA en claro y se reemplaza por su hash).
UPDATE audit_event
   SET user_agent = arbx_hash_user_agent(user_agent)
 WHERE user_agent IS NOT NULL
   AND user_agent NOT LIKE 'sha256:%';

COMMENT ON COLUMN audit_event.ip_address IS
    'OMEGA-8/M3 P1-3: CIDR /24 (IPv4) o /48 (IPv6) — anonimizado vía '
    'arbx_anonymize_ip() antes del INSERT (registry-engine.ts).';
COMMENT ON COLUMN audit_event.user_agent IS
    'OMEGA-8/M3 P1-3: hash sha256 con prefijo "sha256:" — generado vía '
    'arbx_hash_user_agent() antes del INSERT (registry-engine.ts).';

COMMIT;
