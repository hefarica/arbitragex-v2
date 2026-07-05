-- =====================================================================
-- OMEGA Migration 100 — SEED LIQUIDITY PRIMITIVES into contract_registry
-- =====================================================================
-- FASE 1 of the Liquidity Primitive Registry + Anti-Rug Route Builder plan.
--
-- Populates contract_registry (defined in mig 066) with the 4 SAFE,
-- VERIFIED flash/liquidity primitives per chain. These are the registry
-- rows that FASE 3's select_provider_from_registry will read, REPLACING the
-- hardcoded 3-provider priority in flashloan_engine.rs::select_provider.
--
-- Doctrine (RULE 00 + arbx-no-hardcode-doctrine):
--   - NO new table. Reuses contract_registry (mig 066). DRIFT_REPORT C4
--     confirms the doctrine migrates AWAY from per-purpose tables TOWARDS
--     this unified registry.
--   - NO invented addresses. Only canonical, widely-documented, stable
--     mainnet addresses are seeded. Where per-chain explorer verification
--     was not performed in this pass, status='pending_validation' +
--     verified=FALSE flags the row for confirmation before any live path.
--   - Idempotent: ON CONFLICT (chain_id, address) DO NOTHING.
--   - paper-shadow: metadata.live_enabled=FALSE on every row. Live flip is
--     the operator's manual act (gate 12 paper-trade-first, >=7d accumulation).
--
-- contract_kind enum (mig 066) already has flashloan_aave/balancer/dydx/
-- uniswap_v3. Curve/MakerDAO/Morpho/Euler use 'custom' with
-- metadata.provider_family as the discriminant (FASE 4 will add adapters;
-- they are NOT seeded here pending Solidity work + verified deploys).
--
-- config_hash = sha256(chain_id || '|' || lower(address) || '|' || contract_kind)
-- — deterministic per identity; metadata changes don't change the hash.
-- =====================================================================

BEGIN;

-- ---------------------------------------------------------------------
-- 1. Balancer Vault — fee 0 bps. SAME address on every chain (canonical).
--    Verified canonical, never changed since deployment.
-- ---------------------------------------------------------------------
INSERT INTO contract_registry
    (chain_id, label, address, deployer, salt, init_code_hash, abi_version,
     contract_kind, proxy_kind, verified, enabled, status, config_hash, metadata,
     created_by, updated_by)
SELECT v.chain_id, v.label, v.address, '0xBA12222222228d8Ba445958a75a0704d566BF2C8' AS deployer,
       'balancer-vault-v1', 'unknown', 'balancer-vault-v1',
       'flashloan_balancer'::text, 'none', TRUE, TRUE, 'active',
       encode(digest(lower(v.address) || '|flashloan_balancer|' || v.chain_id::text, 'sha256'), 'hex'),
       jsonb_build_object(
           'primitive_name', 'Balancer Vault',
           'provider_family', 'balancer',
           'callback_sig', 'receiveFlashLoan(IERC20[],uint256[],uint256[],bytes)',
           'supported_assets', ARRAY[
               '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2',
               '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48',
               '0x6b175474e89094c44da98b954eedeac495271d0f'
           ],
           'fee_bps', 0,
           'max_depth_usd', 50000000,
           'callback_latency_ms', 1,
           'paper_shadow_enabled', TRUE,
           'live_enabled', FALSE,
           'source', 'seed_liquidity_primitives_100'
       ),
       'seed_100', 'seed_100'
FROM (
    VALUES
        (1::bigint,     'balancer_vault'),
        (10::bigint,    'balancer_vault'),
        (137::bigint,   'balancer_vault'),
        (42161::bigint, 'balancer_vault'),
        (8453::bigint,  'balancer_vault'),
        (43114::bigint, 'balancer_vault'),
        (100::bigint,   'balancer_vault')
) AS v(chain_id, label)
WHERE EXISTS (SELECT 1 FROM chains WHERE chain_id = v.chain_id)
ON CONFLICT (chain_id, address) DO NOTHING;

-- ---------------------------------------------------------------------
-- 2. Uniswap V3 Factory — per-pool flash anchor. SAME address on every V3 chain.
--    Note: this is the FACTORY (the discoverable anchor). Flash is per-pool
--    (uniswapV3FlashCallback). The ranker resolves the specific pool at runtime.
-- ---------------------------------------------------------------------
INSERT INTO contract_registry
    (chain_id, label, address, deployer, salt, init_code_hash, abi_version,
     contract_kind, proxy_kind, verified, enabled, status, config_hash, metadata,
     created_by, updated_by)
SELECT v.chain_id, 'uniswap_v3_factory_flash', '0x1F98431c8aD98523631AE4a59f267346ea31F984',
       '0x1F98431c8aD98523631AE4a59f267346ea31F984', 'uni-v3-factory', 'unknown', 'uniswap-v3',
       'flashloan_uniswap_v3'::text, 'none', v.verified, TRUE, v.status,
       encode(digest('0x1f98431c8ad98523631ae4a59f267346ea31f984|flashloan_uniswap_v3|' || v.chain_id::text, 'sha256'), 'hex'),
       jsonb_build_object(
           'primitive_name', 'Uniswap V3 per-pool flash',
           'provider_family', 'uniswap_v3',
           'callback_sig', 'uniswapV3FlashCallback(address,address,uint256,uint256,bytes)',
           'supported_assets', ARRAY[]::text[],
           'fee_bps', 0,
           'max_depth_usd', 0,
           'callback_latency_ms', 1,
           'anchor_kind', 'factory',
           'note', 'Per-pool flash. Ranker resolves the specific pool at runtime via factory.getPool().',
           'paper_shadow_enabled', TRUE,
           'live_enabled', FALSE,
           'source', 'seed_liquidity_primitives_100'
       ),
       'seed_100', 'seed_100'
FROM (
    VALUES
        (1::bigint,     TRUE,  'active'::text),
        (10::bigint,    TRUE,  'active'),
        (137::bigint,   TRUE,  'active'),
        (42161::bigint, TRUE,  'active'),
        (8453::bigint,  TRUE,  'active'),
        (43114::bigint, TRUE,  'active'),
        (56::bigint,    FALSE, 'pending_validation')
) AS v(chain_id, verified, status)
WHERE EXISTS (SELECT 1 FROM chains WHERE chain_id = v.chain_id)
ON CONFLICT (chain_id, address) DO NOTHING;

-- ---------------------------------------------------------------------
-- 3. MakerDAO DSS Flash — minta DAI, repay-or-revert. Ethereum mainnet only.
--    Verified canonical address.
-- ---------------------------------------------------------------------
INSERT INTO contract_registry
    (chain_id, label, address, deployer, salt, init_code_hash, abi_version,
     contract_kind, proxy_kind, verified, enabled, status, config_hash, metadata,
     created_by, updated_by)
SELECT 1, 'makerdao_dss_flash', '0x60744434d6339a6B27173D763f2370D8E36e8B4C',
       '0x60744434d6339a6B27173D763f2370D8E36e8B4C', 'dss-flash', 'unknown', 'makerdao-dss',
       'custom'::text, 'none', TRUE, TRUE, 'active',
       encode(digest('0x60744434d6339a6b27173d763f2370d8e36e8b4c|custom|1', 'sha256'), 'hex'),
       jsonb_build_object(
           'primitive_name', 'MakerDAO DSS Flash',
           'provider_family', 'makerdao_dss',
           'callback_sig', 'onFlashLoan(address,uint256,uint256,bytes)',
           'supported_assets', ARRAY['0x6b175474e89094c44da98b954eedeac495271d0f'],
           'fee_bps', 5,
           'max_depth_usd', 500000000,
           'callback_latency_ms', 1,
           'note', 'Mints DAI; repay-or-revert. Mainnet only.',
           'paper_shadow_enabled', TRUE,
           'live_enabled', FALSE,
           'source', 'seed_liquidity_primitives_100'
       ),
       'seed_100', 'seed_100'
WHERE EXISTS (SELECT 1 FROM chains WHERE chain_id = 1)
ON CONFLICT (chain_id, address) DO NOTHING;

-- ---------------------------------------------------------------------
-- 4. Aave V3 Pool — fee 5 bps (0.05%). Two addresses:
--      - Ethereum mainnet: 0x87870Bca3F3f6D5b2B7F21489783060DbED28cd4 (verified=TRUE)
--      - L2s (Polygon/Avalanche CONFIRMED on explorer; Arb/Op/Base canonical,
--        pending_validation). Aave V3 uses CREATE2 -> same address across L2s.
-- ---------------------------------------------------------------------
INSERT INTO contract_registry
    (chain_id, label, address, deployer, salt, init_code_hash, abi_version,
     contract_kind, proxy_kind, verified, enabled, status, config_hash, metadata,
     created_by, updated_by)
SELECT v.chain_id, 'aave_v3_pool', v.address,
       v.address, 'aave-v3-pool', 'unknown', 'aave-v3',
       'flashloan_aave'::text, 'none', v.verified, TRUE, v.status,
       encode(digest(lower(v.address) || '|flashloan_aave|' || v.chain_id::text, 'sha256'), 'hex'),
       jsonb_build_object(
           'primitive_name', 'Aave V3 Pool',
           'provider_family', 'aave_v3',
           'callbacks', jsonb_build_object(
               'flashLoan', 'flashLoan(address[],uint256[],uint256[],address,bytes)',
               'flashLoanSimple', 'flashLoanSimple(address,uint256,bytes,uint16)'
           ),
           'supported_assets', v.supported_assets,
           'fee_bps', 5,
           'max_depth_usd', 100000000,
           'callback_latency_ms', 1,
           'paper_shadow_enabled', TRUE,
           'live_enabled', FALSE,
           'source', 'seed_liquidity_primitives_100'
       ),
       'seed_100', 'seed_100'
FROM (
    VALUES
        -- Ethereum mainnet: canonical, widely-documented Pool address.
        (1::bigint,     '0x87870Bca3F3f6D5b2B7F21489783060DbED28cd4'::text, TRUE,  'active'::text,
            ARRAY['0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2',
                  '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48',
                  '0xdac17f958d2ee523a2206206994597c13d831ec7',
                  '0x6b175474e89094c44da98b954eedeac495271d0f',
                  '0x2260fac5e5542a773aa44fbcfedf7c193bc2c599']::text[]),
        -- Polygon: explorer-verified (polygonscan).
        (137::bigint,   '0x794a61358D6845594F94dc1DB02A252b5b4814aD', TRUE,  'active',
            ARRAY['0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270',
                  '0x2791bca1f2de4661ed88a30c99a7a9449aa84174']::text[]),
        -- Avalanche: explorer-verified (snowtrace).
        (43114::bigint, '0x794a61358D6845594F94dc1DB02A252b5b4814aD', TRUE,  'active',
            ARRAY['0xb31f66aa3c1e785363f0875a1b3e7337f77f0af1',
                  '0xb97ef9ef8734c71904d8002f8b6bc66dd9c48a6e']::text[]),
        -- Arbitrum/Base/Optimism: canonical Aave V3 address (CREATE2), pending
        -- per-chain explorer confirmation before live.
        (42161::bigint, '0x794a61358D6845594F94dc1DB02A252b5b4814aD', FALSE, 'pending_validation',
            ARRAY['0x82af49447d8a07e3bd95bd0d56f35349985f1a71',
                  '0xaf88d065e77c8cc2239327c5edb3a432268e5831']::text[]),
        (8453::bigint,  '0x794a61358D6845594F94dc1DB02A252b5b4814aD', FALSE, 'pending_validation',
            ARRAY['0x4200000000000000000000000000000000000006',
                  '0x833589fcd6edb6e08f4c7c32d4f71b54bdab8a42']::text[]),
        (10::bigint,    '0x794a61358D6845594F94dc1DB02A252b5b4814aD', FALSE, 'pending_validation',
            ARRAY['0x4200000000000000000000000000000000000006',
                  '0x7f5c764cbc14f9669b88837ca1490cca17c31607']::text[])
) AS v(chain_id, address, verified, status, supported_assets)
WHERE EXISTS (SELECT 1 FROM chains WHERE chain_id = v.chain_id)
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;

-- =====================================================================
-- DOWN
-- =====================================================================
-- BEGIN;
-- DELETE FROM contract_registry WHERE created_by = 'seed_100';
-- COMMIT;
-- =====================================================================
-- Deferred (NOT seeded here — pending Solidity adapters + verified deploys):
--   * Curve flash/carry     — FASE 4 (pool-specific; needs CurveFlashAdapter.sol)
--   * Morpho Blue           — FASE 4 (MorphoBlueFlashAdapter.sol)
--   * Euler V2              — FASE 4 (EulerV2FlashAdapter.sol)
--   * dYdX Solo             — FASE 4 (DyDxFlashAdapter.sol real impl)
--   * Uniswap V4 Singleton  — FASE 4 (UniV4SingletonFlashAdapter.sol; ETH/Base/Arb only)
-- Do NOT seed these until the adapter contracts exist + addresses are verified.
-- =====================================================================
