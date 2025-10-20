-- StablePay Stablecoin Support (USDC, USDT)
-- Add cryptocurrency payment capabilities

-- Crypto Wallets (customer crypto wallets)
CREATE TABLE IF NOT EXISTS stablepay_crypto_wallets (
    id UUID PRIMARY KEY,
    customer_id UUID NOT NULL,
    wallet_address VARCHAR(255) NOT NULL,
    blockchain VARCHAR(50) NOT NULL, -- ethereum, polygon, solana, etc.
    wallet_type VARCHAR(50) NOT NULL, -- hot, cold, custodial, non_custodial
    label VARCHAR(255),
    is_verified BOOLEAN NOT NULL DEFAULT false,
    is_default BOOLEAN NOT NULL DEFAULT false,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    
    UNIQUE(wallet_address, blockchain)
);

-- Crypto Transactions (on-chain transaction records)
CREATE TABLE IF NOT EXISTS stablepay_crypto_transactions (
    id UUID PRIMARY KEY,
    transaction_id UUID NOT NULL REFERENCES stablepay_transactions(id),
    
    -- Blockchain details
    blockchain VARCHAR(50) NOT NULL, -- ethereum, polygon, solana, arbitrum, optimism
    network VARCHAR(50) NOT NULL, -- mainnet, testnet, goerli, etc.
    token_contract_address VARCHAR(255) NOT NULL, -- USDC/USDT contract address
    token_symbol VARCHAR(10) NOT NULL, -- USDC, USDT
    token_decimals INTEGER NOT NULL DEFAULT 6, -- typically 6 for stablecoins
    
    -- Transaction details
    tx_hash VARCHAR(255) UNIQUE, -- blockchain transaction hash
    block_number BIGINT,
    block_timestamp TIMESTAMPTZ,
    
    -- Addresses
    from_address VARCHAR(255) NOT NULL,
    to_address VARCHAR(255) NOT NULL,
    
    -- Amounts (in token's smallest unit)
    amount_raw VARCHAR(78) NOT NULL, -- raw amount in wei/smallest unit
    amount_decimal DECIMAL(19,6) NOT NULL, -- human-readable amount
    
    -- Gas fees (in native token - ETH, MATIC, SOL, etc.)
    gas_price_gwei DECIMAL(19,9),
    gas_used BIGINT,
    gas_cost_native DECIMAL(19,9), -- cost in native token
    gas_cost_usd DECIMAL(19,4), -- cost in USD
    
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, confirming, confirmed, failed
    confirmations INTEGER NOT NULL DEFAULT 0,
    required_confirmations INTEGER NOT NULL DEFAULT 12,
    
    -- Error handling
    error_code VARCHAR(100),
    error_message TEXT,
    
    -- Metadata
    nonce BIGINT,
    input_data TEXT, -- contract call data
    metadata JSONB,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Stablecoin Exchange Rates (for conversion and pricing)
CREATE TABLE IF NOT EXISTS stablepay_crypto_rates (
    id UUID PRIMARY KEY,
    token_symbol VARCHAR(10) NOT NULL, -- USDC, USDT, ETH, MATIC, SOL
    base_currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    rate DECIMAL(19,9) NOT NULL, -- exchange rate
    
    -- Rate source
    source VARCHAR(50) NOT NULL, -- chainlink, coingecko, binance, etc.
    
    -- Validity
    valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_until TIMESTAMPTZ NOT NULL,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(token_symbol, base_currency, valid_from)
);

-- Blockchain Network Configuration
CREATE TABLE IF NOT EXISTS stablepay_blockchain_networks (
    id UUID PRIMARY KEY,
    blockchain VARCHAR(50) NOT NULL, -- ethereum, polygon, solana, arbitrum
    network VARCHAR(50) NOT NULL, -- mainnet, testnet
    
    -- Network details
    chain_id INTEGER, -- for EVM chains
    rpc_url TEXT NOT NULL,
    explorer_url TEXT, -- etherscan, polygonscan, etc.
    
    -- Gas configuration
    average_gas_price_gwei DECIMAL(19,9),
    fast_gas_price_gwei DECIMAL(19,9),
    native_token_symbol VARCHAR(10) NOT NULL, -- ETH, MATIC, SOL
    
    -- Status
    is_active BOOLEAN NOT NULL DEFAULT true,
    is_testnet BOOLEAN NOT NULL DEFAULT false,
    
    -- Performance
    average_block_time_seconds INTEGER,
    average_confirmation_time_seconds INTEGER,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    
    UNIQUE(blockchain, network)
);

-- Token Contracts (USDC, USDT contracts on different chains)
CREATE TABLE IF NOT EXISTS stablepay_token_contracts (
    id UUID PRIMARY KEY,
    network_id UUID NOT NULL REFERENCES stablepay_blockchain_networks(id),
    
    -- Token details
    token_symbol VARCHAR(10) NOT NULL, -- USDC, USDT
    token_name VARCHAR(100) NOT NULL, -- USD Coin, Tether USD
    contract_address VARCHAR(255) NOT NULL,
    decimals INTEGER NOT NULL DEFAULT 6,
    
    -- Contract info
    token_standard VARCHAR(50), -- ERC20, SPL, etc.
    is_native BOOLEAN NOT NULL DEFAULT false,
    
    -- Features
    supports_permit BOOLEAN NOT NULL DEFAULT false, -- ERC2612 gasless approvals
    supports_meta_transactions BOOLEAN NOT NULL DEFAULT false,
    
    -- Status
    is_active BOOLEAN NOT NULL DEFAULT true,
    
    -- Metadata
    logo_url TEXT,
    website TEXT,
    metadata JSONB,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    
    UNIQUE(network_id, contract_address)
);

-- Crypto Payment Addresses (merchant receiving addresses)
CREATE TABLE IF NOT EXISTS stablepay_payment_addresses (
    id UUID PRIMARY KEY,
    network_id UUID NOT NULL REFERENCES stablepay_blockchain_networks(id),
    
    -- Address details
    address VARCHAR(255) NOT NULL,
    address_type VARCHAR(50) NOT NULL, -- hot_wallet, cold_wallet, smart_contract
    
    -- Purpose
    purpose VARCHAR(100), -- payments, settlements, treasury
    label VARCHAR(255),
    
    -- Security
    requires_multisig BOOLEAN NOT NULL DEFAULT false,
    required_signatures INTEGER,
    
    -- Balance tracking
    last_balance_check TIMESTAMPTZ,
    cached_balance_decimal DECIMAL(19,6),
    
    -- Status
    is_active BOOLEAN NOT NULL DEFAULT true,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    
    UNIQUE(network_id, address)
);

-- Crypto Withdrawal Requests
CREATE TABLE IF NOT EXISTS stablepay_crypto_withdrawals (
    id UUID PRIMARY KEY,
    withdrawal_number VARCHAR(50) NOT NULL UNIQUE,
    
    -- Customer & amount
    customer_id UUID NOT NULL,
    amount DECIMAL(19,6) NOT NULL,
    token_symbol VARCHAR(10) NOT NULL, -- USDC, USDT
    
    -- Destination
    to_wallet_id UUID REFERENCES stablepay_crypto_wallets(id),
    to_address VARCHAR(255) NOT NULL,
    network_id UUID NOT NULL REFERENCES stablepay_blockchain_networks(id),
    
    -- Associated transaction
    crypto_transaction_id UUID REFERENCES stablepay_crypto_transactions(id),
    
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, processing, completed, failed, cancelled
    
    -- Fees
    withdrawal_fee DECIMAL(19,6) NOT NULL DEFAULT 0,
    gas_estimate_usd DECIMAL(19,4),
    
    -- Processing
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    
    -- Error handling
    failure_reason TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    
    -- Metadata
    metadata JSONB,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Blockchain Transaction Monitoring (webhooks from blockchain services)
CREATE TABLE IF NOT EXISTS stablepay_blockchain_events (
    id UUID PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL, -- transaction_pending, transaction_confirmed, block_mined
    
    -- Blockchain details
    blockchain VARCHAR(50) NOT NULL,
    network VARCHAR(50) NOT NULL,
    tx_hash VARCHAR(255),
    block_number BIGINT,
    
    -- Event data
    event_data JSONB NOT NULL,
    
    -- Processing
    processed BOOLEAN NOT NULL DEFAULT false,
    processed_at TIMESTAMPTZ,
    
    -- Source
    source VARCHAR(50) NOT NULL, -- alchemy, infura, quicknode, blocknative
    webhook_id VARCHAR(255),
    
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_crypto_wallets_customer ON stablepay_crypto_wallets(customer_id);
CREATE INDEX IF NOT EXISTS idx_crypto_wallets_address ON stablepay_crypto_wallets(wallet_address, blockchain);
CREATE INDEX IF NOT EXISTS idx_crypto_wallets_default ON stablepay_crypto_wallets(customer_id, is_default) WHERE is_default = true;

CREATE INDEX IF NOT EXISTS idx_crypto_transactions_transaction ON stablepay_crypto_transactions(transaction_id);
CREATE INDEX IF NOT EXISTS idx_crypto_transactions_hash ON stablepay_crypto_transactions(tx_hash);
CREATE INDEX IF NOT EXISTS idx_crypto_transactions_status ON stablepay_crypto_transactions(status);
CREATE INDEX IF NOT EXISTS idx_crypto_transactions_addresses ON stablepay_crypto_transactions(from_address, to_address);
CREATE INDEX IF NOT EXISTS idx_crypto_transactions_block ON stablepay_crypto_transactions(blockchain, network, block_number DESC);

CREATE INDEX IF NOT EXISTS idx_crypto_rates_token ON stablepay_crypto_rates(token_symbol, base_currency);
CREATE INDEX IF NOT EXISTS idx_crypto_rates_validity ON stablepay_crypto_rates(valid_from, valid_until);

CREATE INDEX IF NOT EXISTS idx_token_contracts_network ON stablepay_token_contracts(network_id);
CREATE INDEX IF NOT EXISTS idx_token_contracts_symbol ON stablepay_token_contracts(token_symbol);
CREATE INDEX IF NOT EXISTS idx_token_contracts_active ON stablepay_token_contracts(is_active) WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_payment_addresses_network ON stablepay_payment_addresses(network_id);
CREATE INDEX IF NOT EXISTS idx_payment_addresses_active ON stablepay_payment_addresses(is_active) WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_crypto_withdrawals_customer ON stablepay_crypto_withdrawals(customer_id);
CREATE INDEX IF NOT EXISTS idx_crypto_withdrawals_status ON stablepay_crypto_withdrawals(status);
CREATE INDEX IF NOT EXISTS idx_crypto_withdrawals_created ON stablepay_crypto_withdrawals(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_blockchain_events_tx ON stablepay_blockchain_events(tx_hash);
CREATE INDEX IF NOT EXISTS idx_blockchain_events_processed ON stablepay_blockchain_events(processed) WHERE NOT processed;
CREATE INDEX IF NOT EXISTS idx_blockchain_events_type ON stablepay_blockchain_events(event_type, blockchain, network);

-- Insert default blockchain networks
INSERT INTO stablepay_blockchain_networks (
    id, blockchain, network, chain_id, rpc_url, explorer_url,
    native_token_symbol, is_testnet, average_block_time_seconds
) VALUES 
    -- Ethereum Mainnet
    (gen_random_uuid(), 'ethereum', 'mainnet', 1, 'https://eth-mainnet.alchemyapi.io/v2/', 'https://etherscan.io', 'ETH', false, 12),
    -- Polygon Mainnet
    (gen_random_uuid(), 'polygon', 'mainnet', 137, 'https://polygon-mainnet.g.alchemy.com/v2/', 'https://polygonscan.com', 'MATIC', false, 2),
    -- Arbitrum One
    (gen_random_uuid(), 'arbitrum', 'mainnet', 42161, 'https://arb-mainnet.g.alchemy.com/v2/', 'https://arbiscan.io', 'ETH', false, 1),
    -- Optimism Mainnet
    (gen_random_uuid(), 'optimism', 'mainnet', 10, 'https://opt-mainnet.g.alchemy.com/v2/', 'https://optimistic.etherscan.io', 'ETH', false, 2),
    -- Base Mainnet
    (gen_random_uuid(), 'base', 'mainnet', 8453, 'https://mainnet.base.org', 'https://basescan.org', 'ETH', false, 2)
ON CONFLICT DO NOTHING;

-- Insert USDC and USDT token contracts for each network
DO $$
DECLARE
    ethereum_id UUID;
    polygon_id UUID;
    arbitrum_id UUID;
    optimism_id UUID;
    base_id UUID;
BEGIN
    -- Get network IDs
    SELECT id INTO ethereum_id FROM stablepay_blockchain_networks WHERE blockchain = 'ethereum' AND network = 'mainnet';
    SELECT id INTO polygon_id FROM stablepay_blockchain_networks WHERE blockchain = 'polygon' AND network = 'mainnet';
    SELECT id INTO arbitrum_id FROM stablepay_blockchain_networks WHERE blockchain = 'arbitrum' AND network = 'mainnet';
    SELECT id INTO optimism_id FROM stablepay_blockchain_networks WHERE blockchain = 'optimism' AND network = 'mainnet';
    SELECT id INTO base_id FROM stablepay_blockchain_networks WHERE blockchain = 'base' AND network = 'mainnet';
    
    -- Ethereum USDC
    IF ethereum_id IS NOT NULL THEN
        INSERT INTO stablepay_token_contracts (id, network_id, token_symbol, token_name, contract_address, decimals, token_standard, is_active)
        VALUES (gen_random_uuid(), ethereum_id, 'USDC', 'USD Coin', '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', 6, 'ERC20', true)
        ON CONFLICT DO NOTHING;
        
        INSERT INTO stablepay_token_contracts (id, network_id, token_symbol, token_name, contract_address, decimals, token_standard, is_active)
        VALUES (gen_random_uuid(), ethereum_id, 'USDT', 'Tether USD', '0xdAC17F958D2ee523a2206206994597C13D831ec7', 6, 'ERC20', true)
        ON CONFLICT DO NOTHING;
    END IF;
    
    -- Polygon USDC
    IF polygon_id IS NOT NULL THEN
        INSERT INTO stablepay_token_contracts (id, network_id, token_symbol, token_name, contract_address, decimals, token_standard, is_active)
        VALUES (gen_random_uuid(), polygon_id, 'USDC', 'USD Coin (PoS)', '0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174', 6, 'ERC20', true)
        ON CONFLICT DO NOTHING;
        
        INSERT INTO stablepay_token_contracts (id, network_id, token_symbol, token_name, contract_address, decimals, token_standard, is_active)
        VALUES (gen_random_uuid(), polygon_id, 'USDT', 'Tether USD (PoS)', '0xc2132D05D31c914a87C6611C10748AEb04B58e8F', 6, 'ERC20', true)
        ON CONFLICT DO NOTHING;
    END IF;
    
    -- Arbitrum USDC
    IF arbitrum_id IS NOT NULL THEN
        INSERT INTO stablepay_token_contracts (id, network_id, token_symbol, token_name, contract_address, decimals, token_standard, is_active)
        VALUES (gen_random_uuid(), arbitrum_id, 'USDC', 'USD Coin', '0xaf88d065e77c8cC2239327C5EDb3A432268e5831', 6, 'ERC20', true)
        ON CONFLICT DO NOTHING;
        
        INSERT INTO stablepay_token_contracts (id, network_id, token_symbol, token_name, contract_address, decimals, token_standard, is_active)
        VALUES (gen_random_uuid(), arbitrum_id, 'USDT', 'Tether USD', '0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9', 6, 'ERC20', true)
        ON CONFLICT DO NOTHING;
    END IF;
    
    -- Optimism USDC
    IF optimism_id IS NOT NULL THEN
        INSERT INTO stablepay_token_contracts (id, network_id, token_symbol, token_name, contract_address, decimals, token_standard, is_active)
        VALUES (gen_random_uuid(), optimism_id, 'USDC', 'USD Coin', '0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85', 6, 'ERC20', true)
        ON CONFLICT DO NOTHING;
        
        INSERT INTO stablepay_token_contracts (id, network_id, token_symbol, token_name, contract_address, decimals, token_standard, is_active)
        VALUES (gen_random_uuid(), optimism_id, 'USDT', 'Tether USD', '0x94b008aA00579c1307B0EF2c499aD98a8ce58e58', 6, 'ERC20', true)
        ON CONFLICT DO NOTHING;
    END IF;
    
    -- Base USDC
    IF base_id IS NOT NULL THEN
        INSERT INTO stablepay_token_contracts (id, network_id, token_symbol, token_name, contract_address, decimals, token_standard, is_active)
        VALUES (gen_random_uuid(), base_id, 'USDC', 'USD Coin', '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913', 6, 'ERC20', true)
        ON CONFLICT DO NOTHING;
    END IF;
END $$;

-- Insert initial exchange rates (stablecoins should be ~$1.00)
INSERT INTO stablepay_crypto_rates (id, token_symbol, base_currency, rate, source, valid_from, valid_until)
VALUES 
    (gen_random_uuid(), 'USDC', 'USD', 1.000000000, 'chainlink', NOW(), NOW() + INTERVAL '1 hour'),
    (gen_random_uuid(), 'USDT', 'USD', 1.000000000, 'chainlink', NOW(), NOW() + INTERVAL '1 hour'),
    (gen_random_uuid(), 'ETH', 'USD', 2500.000000000, 'chainlink', NOW(), NOW() + INTERVAL '1 hour'),
    (gen_random_uuid(), 'MATIC', 'USD', 0.850000000, 'chainlink', NOW(), NOW() + INTERVAL '1 hour')
ON CONFLICT DO NOTHING;

-- Add crypto provider to stablepay_providers
INSERT INTO stablepay_providers (
    id, name, provider_type, fee_percentage, fee_fixed,
    supported_currencies, supported_countries, priority
) VALUES (
    gen_random_uuid(),
    'StablePay Crypto',
    'crypto',
    0.0050, -- 0.5% fee for crypto
    0.00,   -- No fixed fee
    ARRAY['USDC', 'USDT'],
    ARRAY['GLOBAL'], -- Available globally
    5 -- Higher priority than traditional but lower than direct
) ON CONFLICT DO NOTHING;

