-- DeFi Pools, Reserves and Routes
-- Strict on-chain state mirroring

CREATE TABLE IF NOT EXISTS pools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id INTEGER REFERENCES chains(chain_id),
    factory_id UUID REFERENCES factories(id),
    address VARCHAR(42) NOT NULL,
    token0_id UUID REFERENCES tokens(id),
    token1_id UUID REFERENCES tokens(id),
    fee_tier INTEGER, -- Para UniV3
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (chain_id, address)
);

CREATE TABLE IF NOT EXISTS pool_reserves (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id UUID REFERENCES pools(id),
    block_number BIGINT NOT NULL,
    reserve0 NUMERIC NOT NULL,
    reserve1 NUMERIC NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Para UniV3
CREATE TABLE IF NOT EXISTS pool_ticks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id UUID REFERENCES pools(id),
    block_number BIGINT NOT NULL,
    tick INTEGER NOT NULL,
    liquidity_net NUMERIC NOT NULL,
    liquidity_gross NUMERIC NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id INTEGER REFERENCES chains(chain_id),
    hash VARCHAR(64) UNIQUE NOT NULL, -- SHA256 of ordered pool addresses
    is_active BOOLEAN DEFAULT TRUE,
    estimated_gas BIGINT,
    last_profitable_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS route_legs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    route_id UUID REFERENCES routes(id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL,
    pool_id UUID REFERENCES pools(id),
    token_in_id UUID REFERENCES tokens(id),
    token_out_id UUID REFERENCES tokens(id),
    UNIQUE (route_id, step_index)
);
