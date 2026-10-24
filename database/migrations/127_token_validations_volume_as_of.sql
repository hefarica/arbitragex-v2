-- Migration 127 — token_validations.volume_as_of (PR #1004, contract §2)
--
-- Why: `volume_24h_usd` is a DEX Screener snapshot value, but the only
-- timestamp in this table is `validated_at`, which is when OUR validator ran.
-- Those are different facts: an HTTP cache hit, a retry, or a queued worker can
-- serve a snapshot hours old while `validated_at` is seconds old. Ordering the
-- operator's census by a volume of unknown age is not a ranking.
--
-- This column records WHEN THE SNAPSHOT WAS CAPTURED (the moment the DEX
-- Screener response was parsed), never the INSERT time. NULL means the age is
-- unknown — and an unknown age never orders (S09 / token-top.ts gate:
-- volume_as_of IS NULL => not orderable).
--
-- Additive and idempotent: no data movement, no default, no backfill. Existing
-- rows keep NULL, which is the honest value for a volume whose snapshot time
-- was never recorded.

BEGIN;

ALTER TABLE token_validations
  ADD COLUMN IF NOT EXISTS volume_as_of TIMESTAMPTZ NULL;

COMMENT ON COLUMN token_validations.volume_as_of IS
  'Capture time of the DEX Screener snapshot behind volume_24h_usd. NULL = unknown age (never substitute validated_at).';

COMMIT;
