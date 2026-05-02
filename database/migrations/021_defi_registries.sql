-- DeFi Registries for 100% On-Chain Configuration
-- Zero hardcoding allowed.

CREATE TABLE chains (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id INTEGER UNIQUE NOT NULL,
    name VARCHAR(100) NOT NULL,
    native_currency VARCHAR(10) NOT NULL,
    explorer_url VARCHAR(255),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE rpcs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id INTEGER REFERENCES chains(chain_id),
    url VARCHAR(500) NOT NULL,
    type VARCHAR(20) CHECK (type IN ('HTTP', 'WSS')),
    priority INTEGER DEFAULT 10,
    is_active BOOLEAN DEFAULT TRUE,
    health_status VARCHAR(20) DEFAULT 'UNKNOWN',
    latency_ms INTEGER,
    last_checked_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE dexes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) UNIQUE NOT NULL,
    protocol_type VARCHAR(50) CHECK (protocol_type IN ('UNISWAP_V2', 'UNISWAP_V3', 'CURVE', 'BALANCER')),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE routers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dex_id UUID REFERENCES dexes(id),
    chain_id INTEGER REFERENCES chains(chain_id),
    address VARCHAR(42) NOT NULL,
    version VARCHAR(20),
    is_trusted BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (chain_id, address)
);

CREATE TABLE factories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dex_id UUID REFERENCES dexes(id),
    chain_id INTEGER REFERENCES chains(chain_id),
    address VARCHAR(42) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (chain_id, address)
);

CREATE TABLE tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id INTEGER REFERENCES chains(chain_id),
    address VARCHAR(42) NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    decimals INTEGER NOT NULL,
    is_stablecoin BOOLEAN DEFAULT FALSE,
    is_active BOOLEAN DEFAULT TRUE,
    risk_level VARCHAR(20) DEFAULT 'UNKNOWN',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (chain_id, address)
);
