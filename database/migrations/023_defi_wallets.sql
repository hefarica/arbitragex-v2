-- DeFi Wallets Registry
-- Strict requirement: Private keys MUST be encrypted at rest.

CREATE TABLE IF NOT EXISTS wallets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address VARCHAR(42) UNIQUE NOT NULL,
    label VARCHAR(100),
    private_key_encrypted TEXT NOT NULL, -- AES-256-GCM encrypted using an off-db master key
    encryption_iv VARCHAR(32) NOT NULL,
    encryption_tag VARCHAR(32) NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS wallet_allowances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id UUID REFERENCES wallets(id),
    token_id UUID REFERENCES tokens(id),
    spender_address VARCHAR(42) NOT NULL,
    amount NUMERIC NOT NULL,
    last_checked_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(wallet_id, token_id, spender_address)
);
