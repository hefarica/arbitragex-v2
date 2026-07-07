-- =====================================================================
-- OMEGA-8 / M3 Migration 069 — runtime_ack hardening (Capa 2)
-- =====================================================================
-- Forward-only · idempotent · safe.
--
-- Cierra los hallazgos del audit Capa 2:
--   • P1-1: invariant I-2 (idempotencia POST-INSERT) no estaba enforced
--           por el schema. Esta migración añade UNIQUE (event_id, layer)
--           y permite el ON CONFLICT (event_id, layer) DO NOTHING del
--           handler `POST /api/system/runtime-ack`.
--   • P2-1: CHECK chain_id ahora rechaza valores <= 0 (Zod ya lo hacía a
--           nivel API; la DB lo replica como defensa).
--   • P2-3: índice parcial por failures (status ∈ {failed,timeout,rejected})
--           ordenado DESC por received_at, para troubleshooting.
--
-- Estrategia anti-duplicados PRE-UNIQUE:
--   1. Detectar filas duplicadas por (event_id, layer).
--   2. Conservar la más reciente por `received_at` (estado final más fresco
--      = más informativo para troubleshooting; los `pending`/`received`
--      previos al `applied`/`failed` final son redundantes).
--   3. Mover duplicados desplazados a `runtime_ack_dedupe_archive_069`
--      (auditoría forense; mantiene la trazabilidad histórica).
--   4. Borrar duplicados de la tabla principal.
--   5. Crear UNIQUE constraint.
--
-- Idempotencia:
--   • Toda DDL envuelta en `DO $$ ... EXCEPTION WHEN duplicate_object ...`
--     o `IF NOT EXISTS`.
--   • Re-ejecutar este script en una DB ya migrada = no-op.
--
-- Rollback manual (NO automático):
--   ALTER TABLE runtime_ack DROP CONSTRAINT runtime_ack_event_layer_unique;
--   ALTER TABLE runtime_ack DROP CONSTRAINT runtime_ack_chain_id_positive;
--   DROP INDEX IF EXISTS idx_runtime_ack_failures;
--   -- Los rows en runtime_ack_dedupe_archive_069 se conservan a propósito.
-- =====================================================================

BEGIN;

-- 1. Archivo forense de duplicados (idempotente)
CREATE TABLE IF NOT EXISTS runtime_ack_dedupe_archive_069 (
    archived_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    archived_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    original_id        UUID NOT NULL,
    event_id           UUID NOT NULL,
    resource           TEXT NOT NULL,
    chain_id           BIGINT,
    idempotency_key    TEXT NOT NULL,
    config_hash_before CHAR(64),
    config_hash_after  CHAR(64) NOT NULL,
    worker_id          TEXT NOT NULL,
    layer              TEXT NOT NULL,
    status             TEXT NOT NULL,
    latency_ms         INTEGER,
    error              TEXT,
    received_at        TIMESTAMPTZ NOT NULL
);
COMMENT ON TABLE runtime_ack_dedupe_archive_069 IS
    'OMEGA-8/M3 P1-1: snapshot de filas runtime_ack desplazadas antes de '
    'crear UNIQUE(event_id, layer). Forense, no operativa.';

-- 2. Detectar y archivar duplicados (NO borrar fila más reciente)
WITH ranked AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY event_id, layer
            ORDER BY received_at DESC, id DESC
        ) AS rn
    FROM runtime_ack
),
duplicates AS (
    SELECT id FROM ranked WHERE rn > 1
),
archived AS (
    INSERT INTO runtime_ack_dedupe_archive_069 (
        original_id, event_id, resource, chain_id, idempotency_key,
        config_hash_before, config_hash_after, worker_id, layer, status,
        latency_ms, error, received_at
    )
    SELECT
        ra.id, ra.event_id, ra.resource, ra.chain_id, ra.idempotency_key,
        ra.config_hash_before, ra.config_hash_after, ra.worker_id, ra.layer,
        ra.status, ra.latency_ms, ra.error, ra.received_at
    FROM runtime_ack ra
    INNER JOIN duplicates d ON ra.id = d.id
    ON CONFLICT DO NOTHING
    RETURNING original_id
)
DELETE FROM runtime_ack
 WHERE id IN (SELECT original_id FROM archived);

-- 3. UNIQUE constraint (idempotente)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'runtime_ack_event_layer_unique'
    ) THEN
        ALTER TABLE runtime_ack
          ADD CONSTRAINT runtime_ack_event_layer_unique
          UNIQUE (event_id, layer);
    END IF;
END$$;

-- 4. CHECK chain_id > 0 (idempotente). NULL sigue permitido — algunos
--    recursos (global, etc.) no tienen chain.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'runtime_ack_chain_id_positive'
    ) THEN
        ALTER TABLE runtime_ack
          ADD CONSTRAINT runtime_ack_chain_id_positive
          CHECK (chain_id IS NULL OR chain_id > 0);
    END IF;
END$$;

-- 5. Índice parcial para failures (idempotente)
CREATE INDEX IF NOT EXISTS idx_runtime_ack_failures
    ON runtime_ack (received_at DESC)
    WHERE status IN ('failed', 'timeout', 'rejected');

COMMENT ON CONSTRAINT runtime_ack_event_layer_unique ON runtime_ack IS
    'OMEGA-8/M3 P1-1: enforces invariant I-2 (idempotencia POST-INSERT). '
    'Used together with INSERT ... ON CONFLICT (event_id, layer) DO NOTHING '
    'in routes/system-manifest.ts. A retry from any producer creates ONE row, '
    'never duplicates.';

COMMENT ON CONSTRAINT runtime_ack_chain_id_positive ON runtime_ack IS
    'OMEGA-8/M3 P2-1: aligns SQL CHECK with the Zod schema in '
    'routes/system-manifest.ts which already requires chain_id > 0 when '
    'present. Defense-in-depth against bypass writes.';

COMMIT;
