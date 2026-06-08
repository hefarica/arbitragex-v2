-- ArbitrageX v2 — Migration 094: seed Chainlink USD price feeds (chain 1).
--
-- WHY: the price_worker had NO working price source — Alchemy key 429-exhausted,
-- and the free CoinGecko /simple/token_price endpoint now returns HTTP 400 (it
-- requires an API key). So `arbx:token_prices:1` stayed empty, every priced leg
-- hit `no_price_oracle`, and arbx:opps:detected froze at 379 (0 new opps). Live
-- 3-agent diagnosis 2026-06-05 (workflow wzj59x85r) identified this as the #1
-- blocker (~57-64% of all rejections).
--
-- WHAT: the CANONICAL public Chainlink mainnet USD aggregators (source:
-- docs.chain.link/data-feeds/price-feeds/addresses). These are public
-- infrastructure contracts, NOT operator secrets — migration 017's "operator
-- supplies oracle addresses" stance is satisfied by the operator's explicit
-- choice (2026-06-05) of "Chainlink on-chain". The searcher-rs price_worker
-- reads kind='chainlink' enabled rows and eth_calls latestRoundData() to
-- populate the price cache the spine cascade already consumes. Read-only;
-- no signing, no capital.
--
-- Decimals = 8 (Chainlink USD feeds report 8 decimals). Addresses are EIP-55
-- checksummed; the worker lowercases for eth_call / Redis meta lookups.

INSERT INTO price_oracles
  (chain_id, kind, token_address, quote_symbol, oracle_address, decimals, enabled, description, created_by)
VALUES
  (1, 'chainlink', '0xC02aaa39b223FE8D0A0e5C4F27eAD9083C756Cc2', 'USD',
      '0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419', 8, TRUE,
      'WETH/USD — Chainlink ETH/USD aggregator (mainnet)', 'migration_094_chainlink_seed'),
  (1, 'chainlink', '0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599', 'USD',
      '0xF4030086522a5bEEa4988F8cA5B36dbC97BeE88c', 8, TRUE,
      'WBTC/USD — Chainlink BTC/USD aggregator (mainnet)', 'migration_094_chainlink_seed'),
  (1, 'chainlink', '0xA0b86991c6218b36c1D19D4a2e9Eb0cE3606eB48', 'USD',
      '0x8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6', 8, TRUE,
      'USDC/USD — Chainlink (mainnet)', 'migration_094_chainlink_seed'),
  (1, 'chainlink', '0xdAC17F958D2ee523a2206206994597C13D831ec7', 'USD',
      '0x3E7d1eAB13ad0104d2750B8863b489D65364e32D', 8, TRUE,
      'USDT/USD — Chainlink (mainnet)', 'migration_094_chainlink_seed'),
  (1, 'chainlink', '0x6B175474E89094C44Da98b954EedeAC495271d0F', 'USD',
      '0xAed0c38402a5d19df6E4c03F4E2DceD6e29c1ee9', 8, TRUE,
      'DAI/USD — Chainlink (mainnet)', 'migration_094_chainlink_seed')
ON CONFLICT (chain_id, kind, token_address, quote_symbol) DO UPDATE
  SET oracle_address = EXCLUDED.oracle_address,
      decimals       = EXCLUDED.decimals,
      enabled        = TRUE,
      description    = EXCLUDED.description,
      updated_at     = NOW();
