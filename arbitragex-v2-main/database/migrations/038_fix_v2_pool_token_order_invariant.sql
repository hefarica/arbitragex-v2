-- ArbitrageX v2 — Migration 038: fix swapped token0/token1 in pool 037
--                  + enforce Uniswap V2 invariant via CHECK constraint.
--
-- Incident (2026-05-07): Migration 037 declared the WBTC/USDC V2 pool
-- (0x004375...3A416) with token0_id=USDC, token1_id=WBTC. Uniswap V2
-- invariant requires token0 = the LOWER address. WBTC=0x2260... is
-- lexicographically BELOW USDC=0xa0b8..., so the on-chain token0 is
-- WBTC. The misdeclaration caused pool_sync_worker to write the wrong
-- token0_addr to ReservesEntry, which then caused triangular_worker to
-- read swap orientation backwards on USDC→WBTC hops.
--
-- Result: 1183 fake-positive triangular opportunities emitted with
-- expected_profit_usd ~ $4M each (already marked AnomalousMath out-
-- of-band by operator). Same orientation flip would have happened on
-- ANY pool we declared with the higher address as token0.
--
-- Fix is two-part:
--   (1) UPDATE the WBTC/USDC row to swap token0_id ↔ token1_id.
--   (2) Add a CHECK constraint enforcing
--       LOWER((SELECT address FROM tokens WHERE id=token0_id))
--         < LOWER((SELECT address FROM tokens WHERE id=token1_id))
--       so the next operator-authored pool insert can't repeat the bug.
--       (Implemented as a deferred trigger because PG CHECK can't
--       reference other tables; trigger fires BEFORE INSERT/UPDATE.)
--
-- After applying: restart searcher-rs to re-bootstrap pool_sync_worker
-- which rewrites ReservesEntry with the correct token0_addr from PG.

BEGIN;

-- ── (1) Fix the swapped row ──────────────────────────────────────────────
UPDATE pools
   SET token0_id = (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WBTC'),
       token1_id = (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC')
 WHERE chain_id = 1
   AND address  = '0x004375dff511095cc5a197a54140a24efef3a416';

-- ── (2) Trigger to enforce token0_address < token1_address invariant ────
-- Applies to UniswapV2-protocol pools only (V3 has its own ordering rules
-- that we already get right via the V3 quote path).
CREATE OR REPLACE FUNCTION enforce_v2_token_order()
RETURNS TRIGGER AS $$
DECLARE
  proto       TEXT;
  addr0_lower TEXT;
  addr1_lower TEXT;
BEGIN
  -- Only enforce for UniswapV2-style protocols.
  SELECT d.protocol_type
    INTO proto
    FROM factories f
    JOIN dexes d ON d.id = f.dex_id
   WHERE f.id = NEW.factory_id;

  IF proto IS NULL OR proto <> 'UNISWAP_V2' THEN
    RETURN NEW;
  END IF;

  SELECT LOWER(address) INTO addr0_lower FROM tokens WHERE id = NEW.token0_id;
  SELECT LOWER(address) INTO addr1_lower FROM tokens WHERE id = NEW.token1_id;

  IF addr0_lower IS NULL OR addr1_lower IS NULL THEN
    RAISE EXCEPTION 'pool % token0_id/token1_id reference missing tokens row', NEW.address;
  END IF;

  IF addr0_lower >= addr1_lower THEN
    RAISE EXCEPTION 'pool % violates Uniswap V2 invariant: token0 (%) must be lexicographically < token1 (%)',
      NEW.address, addr0_lower, addr1_lower;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS pools_v2_token_order_trigger ON pools;
CREATE TRIGGER pools_v2_token_order_trigger
  BEFORE INSERT OR UPDATE ON pools
  FOR EACH ROW
  EXECUTE FUNCTION enforce_v2_token_order();

-- ── Sanity: verify the fix landed and the invariant holds for all V2 rows.
DO $$
DECLARE
  bad_count INT;
BEGIN
  SELECT COUNT(*) INTO bad_count
    FROM pools p
    JOIN factories f ON f.id = p.factory_id
    JOIN dexes d ON d.id = f.dex_id
    JOIN tokens t0 ON t0.id = p.token0_id
    JOIN tokens t1 ON t1.id = p.token1_id
   WHERE d.protocol_type = 'UNISWAP_V2'
     AND LOWER(t0.address) >= LOWER(t1.address);
  IF bad_count > 0 THEN
    RAISE WARNING 'migration 038: % V2 pool rows STILL violate token0<token1 invariant', bad_count;
  END IF;
END $$;

COMMIT;
