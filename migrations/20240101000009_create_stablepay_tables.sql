-- StablePay: Enterprise Retail Payment System
-- Instant global payments, auto-reconciliation, reduced costs

-- Payment Providers (Stripe, PayPal, Bank Transfer, Crypto, etc.)
CREATE TABLE IF NOT EXISTS stablepay_providers (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    provider_type VARCHAR(50) NOT NULL, -- stripe, paypal, bank_transfer, crypto, etc.
    api_key_encrypted TEXT,
    webhook_secret_encrypted TEXT,
    configuration JSONB, -- provider-specific config
    fee_percentage DECIMAL(5,4) DEFAULT 0, -- 2.9% = 0.0290
    fee_fixed DECIMAL(10,4) DEFAULT 0, -- flat fee per transaction
    supported_currencies TEXT[] NOT NULL DEFAULT '{}',
    supported_countries TEXT[] NOT NULL DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT true,
    priority INTEGER NOT NULL DEFAULT 100, -- lower = higher priority for routing
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Payment Methods (stored customer payment methods)
CREATE TABLE IF NOT EXISTS stablepay_payment_methods (
    id UUID PRIMARY KEY,
    customer_id UUID NOT NULL,
    provider_id UUID NOT NULL REFERENCES stablepay_providers(id),
    external_id VARCHAR(255), -- ID in external system (e.g., Stripe payment method ID)
    method_type VARCHAR(50) NOT NULL, -- card, bank_account, crypto_wallet, paypal
    brand VARCHAR(50), -- visa, mastercard, etc.
    last_four VARCHAR(4),
    exp_month INTEGER,
    exp_year INTEGER,
    holder_name VARCHAR(255),
    billing_address JSONB,
    is_default BOOLEAN NOT NULL DEFAULT false,
    is_verified BOOLEAN NOT NULL DEFAULT false,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Enhanced Payment Transactions
CREATE TABLE IF NOT EXISTS stablepay_transactions (
    id UUID PRIMARY KEY,
    transaction_number VARCHAR(50) NOT NULL UNIQUE,
    
    -- Relationships
    order_id UUID, -- can be null for standalone payments
    customer_id UUID NOT NULL,
    payment_method_id UUID REFERENCES stablepay_payment_methods(id),
    provider_id UUID NOT NULL REFERENCES stablepay_providers(id),
    
    -- Amount details
    amount DECIMAL(19,4) NOT NULL,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    original_amount DECIMAL(19,4), -- if currency converted
    original_currency VARCHAR(3),
    exchange_rate DECIMAL(12,6),
    
    -- Fee breakdown
    provider_fee DECIMAL(19,4) NOT NULL DEFAULT 0,
    platform_fee DECIMAL(19,4) NOT NULL DEFAULT 0,
    total_fees DECIMAL(19,4) NOT NULL DEFAULT 0,
    net_amount DECIMAL(19,4) NOT NULL, -- amount - total_fees
    
    -- Status and processing
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, processing, succeeded, failed, cancelled, refunded, partially_refunded
    payment_intent_id VARCHAR(255), -- external payment intent ID
    charge_id VARCHAR(255), -- external charge ID
    
    -- Timing
    initiated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    settled_at TIMESTAMPTZ,
    estimated_settlement_date DATE,
    
    -- Error handling
    failure_code VARCHAR(100),
    failure_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    
    -- Reconciliation
    is_reconciled BOOLEAN NOT NULL DEFAULT false,
    reconciled_at TIMESTAMPTZ,
    reconciliation_id UUID,
    
    -- Security
    risk_score DECIMAL(5,2), -- 0-100
    is_flagged_for_review BOOLEAN NOT NULL DEFAULT false,
    fraud_indicators JSONB,
    
    -- Additional data
    description TEXT,
    metadata JSONB,
    gateway_response JSONB,
    
    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    created_by UUID,
    
    -- Idempotency
    idempotency_key VARCHAR(255) UNIQUE
);

-- Refunds
CREATE TABLE IF NOT EXISTS stablepay_refunds (
    id UUID PRIMARY KEY,
    refund_number VARCHAR(50) NOT NULL UNIQUE,
    transaction_id UUID NOT NULL REFERENCES stablepay_transactions(id),
    
    -- Amount
    amount DECIMAL(19,4) NOT NULL,
    currency VARCHAR(3) NOT NULL,
    
    -- Fees
    refunded_fees DECIMAL(19,4) NOT NULL DEFAULT 0, -- fees returned
    net_refund DECIMAL(19,4) NOT NULL,
    
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, processing, succeeded, failed, cancelled
    refund_id_external VARCHAR(255), -- external refund ID
    
    -- Reason
    reason VARCHAR(100), -- duplicate, fraudulent, requested_by_customer, product_issue
    reason_detail TEXT,
    
    -- Timing
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    
    -- Error
    failure_code VARCHAR(100),
    failure_message TEXT,
    
    -- Additional
    metadata JSONB,
    gateway_response JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    created_by UUID
);

-- Payment Batches (for bulk processing and reduced costs)
CREATE TABLE IF NOT EXISTS stablepay_batches (
    id UUID PRIMARY KEY,
    batch_number VARCHAR(50) NOT NULL UNIQUE,
    batch_type VARCHAR(50) NOT NULL, -- payout, settlement, transfer
    
    -- Provider
    provider_id UUID NOT NULL REFERENCES stablepay_providers(id),
    
    -- Summary
    total_amount DECIMAL(19,4) NOT NULL,
    currency VARCHAR(3) NOT NULL,
    transaction_count INTEGER NOT NULL DEFAULT 0,
    total_fees DECIMAL(19,4) NOT NULL DEFAULT 0,
    
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, processing, completed, failed, cancelled
    
    -- Timing
    scheduled_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    
    -- Error
    failure_code VARCHAR(100),
    failure_message TEXT,
    
    -- Additional
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Batch Items (transactions in a batch)
CREATE TABLE IF NOT EXISTS stablepay_batch_items (
    id UUID PRIMARY KEY,
    batch_id UUID NOT NULL REFERENCES stablepay_batches(id) ON DELETE CASCADE,
    transaction_id UUID NOT NULL REFERENCES stablepay_transactions(id),
    
    -- Individual status in batch
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    sequence_number INTEGER NOT NULL,
    
    -- Error
    failure_code VARCHAR(100),
    failure_message TEXT,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    
    UNIQUE(batch_id, transaction_id)
);

-- Auto-Reconciliation Records
CREATE TABLE IF NOT EXISTS stablepay_reconciliations (
    id UUID PRIMARY KEY,
    reconciliation_number VARCHAR(50) NOT NULL UNIQUE,
    
    -- Period
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    
    -- Provider
    provider_id UUID NOT NULL REFERENCES stablepay_providers(id),
    
    -- Summary
    total_transactions INTEGER NOT NULL DEFAULT 0,
    total_amount DECIMAL(19,4) NOT NULL DEFAULT 0,
    total_fees DECIMAL(19,4) NOT NULL DEFAULT 0,
    matched_transactions INTEGER NOT NULL DEFAULT 0,
    unmatched_transactions INTEGER NOT NULL DEFAULT 0,
    
    -- Discrepancies
    discrepancy_amount DECIMAL(19,4) NOT NULL DEFAULT 0,
    discrepancy_count INTEGER NOT NULL DEFAULT 0,
    
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, in_progress, completed, failed, requires_review
    
    -- Files
    provider_statement_url TEXT,
    reconciliation_report_url TEXT,
    
    -- Timing
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    
    -- Additional
    metadata JSONB,
    notes TEXT,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    created_by UUID
);

-- Reconciliation Items (individual matches/mismatches)
CREATE TABLE IF NOT EXISTS stablepay_reconciliation_items (
    id UUID PRIMARY KEY,
    reconciliation_id UUID NOT NULL REFERENCES stablepay_reconciliations(id) ON DELETE CASCADE,
    transaction_id UUID REFERENCES stablepay_transactions(id),
    
    -- External data
    external_transaction_id VARCHAR(255),
    external_amount DECIMAL(19,4),
    external_currency VARCHAR(3),
    external_date TIMESTAMPTZ,
    
    -- Matching
    match_status VARCHAR(50) NOT NULL, -- matched, unmatched, discrepancy
    match_score DECIMAL(5,2), -- 0-100 confidence
    
    -- Discrepancy
    amount_difference DECIMAL(19,4),
    discrepancy_reason TEXT,
    
    -- Resolution
    is_resolved BOOLEAN NOT NULL DEFAULT false,
    resolved_at TIMESTAMPTZ,
    resolved_by UUID,
    resolution_notes TEXT,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Payment Analytics (for cost optimization)
CREATE TABLE IF NOT EXISTS stablepay_analytics (
    id UUID PRIMARY KEY,
    date DATE NOT NULL,
    provider_id UUID REFERENCES stablepay_providers(id),
    currency VARCHAR(3),
    
    -- Volume
    transaction_count INTEGER NOT NULL DEFAULT 0,
    total_volume DECIMAL(19,4) NOT NULL DEFAULT 0,
    
    -- Success rates
    successful_count INTEGER NOT NULL DEFAULT 0,
    failed_count INTEGER NOT NULL DEFAULT 0,
    success_rate DECIMAL(5,2), -- percentage
    
    -- Fees
    total_fees DECIMAL(19,4) NOT NULL DEFAULT 0,
    average_fee_percentage DECIMAL(5,4),
    
    -- Timing
    average_processing_time_seconds INTEGER,
    average_settlement_time_hours INTEGER,
    
    -- Risk
    flagged_count INTEGER NOT NULL DEFAULT 0,
    fraud_detected_count INTEGER NOT NULL DEFAULT 0,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    
    UNIQUE(date, provider_id, currency)
);

-- Payment Routing Rules (for cost optimization)
CREATE TABLE IF NOT EXISTS stablepay_routing_rules (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100, -- lower = higher priority
    
    -- Conditions (JSONB for flexibility)
    conditions JSONB NOT NULL, -- {amount_min, amount_max, currency, country, customer_segment, etc.}
    
    -- Routing
    provider_id UUID NOT NULL REFERENCES stablepay_providers(id),
    fallback_provider_id UUID REFERENCES stablepay_providers(id),
    
    -- Status
    is_active BOOLEAN NOT NULL DEFAULT true,
    
    -- Analytics
    usage_count INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    created_by UUID
);

-- Webhooks (for provider notifications)
CREATE TABLE IF NOT EXISTS stablepay_webhooks (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES stablepay_providers(id),
    
    -- Event
    event_type VARCHAR(100) NOT NULL,
    event_id VARCHAR(255) NOT NULL, -- external event ID
    
    -- Payload
    payload JSONB NOT NULL,
    signature VARCHAR(500),
    
    -- Processing
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, processing, processed, failed
    processed_at TIMESTAMPTZ,
    retry_count INTEGER NOT NULL DEFAULT 0,
    
    -- Error
    error_message TEXT,
    
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(provider_id, event_id)
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_stablepay_transactions_customer ON stablepay_transactions(customer_id);
CREATE INDEX IF NOT EXISTS idx_stablepay_transactions_order ON stablepay_transactions(order_id);
CREATE INDEX IF NOT EXISTS idx_stablepay_transactions_status ON stablepay_transactions(status);
CREATE INDEX IF NOT EXISTS idx_stablepay_transactions_created ON stablepay_transactions(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_stablepay_transactions_settled ON stablepay_transactions(settled_at DESC) WHERE settled_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_stablepay_transactions_reconciled ON stablepay_transactions(is_reconciled, reconciled_at);
CREATE INDEX IF NOT EXISTS idx_stablepay_transactions_provider ON stablepay_transactions(provider_id);

CREATE INDEX IF NOT EXISTS idx_stablepay_payment_methods_customer ON stablepay_payment_methods(customer_id);
CREATE INDEX IF NOT EXISTS idx_stablepay_payment_methods_default ON stablepay_payment_methods(customer_id, is_default) WHERE is_default = true;

CREATE INDEX IF NOT EXISTS idx_stablepay_refunds_transaction ON stablepay_refunds(transaction_id);
CREATE INDEX IF NOT EXISTS idx_stablepay_refunds_status ON stablepay_refunds(status);
CREATE INDEX IF NOT EXISTS idx_stablepay_refunds_created ON stablepay_refunds(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_stablepay_batches_status ON stablepay_batches(status);
CREATE INDEX IF NOT EXISTS idx_stablepay_batches_provider ON stablepay_batches(provider_id);
CREATE INDEX IF NOT EXISTS idx_stablepay_batches_scheduled ON stablepay_batches(scheduled_at);

CREATE INDEX IF NOT EXISTS idx_stablepay_reconciliations_provider ON stablepay_reconciliations(provider_id);
CREATE INDEX IF NOT EXISTS idx_stablepay_reconciliations_period ON stablepay_reconciliations(period_start, period_end);
CREATE INDEX IF NOT EXISTS idx_stablepay_reconciliations_status ON stablepay_reconciliations(status);

CREATE INDEX IF NOT EXISTS idx_stablepay_analytics_date ON stablepay_analytics(date DESC);
CREATE INDEX IF NOT EXISTS idx_stablepay_analytics_provider ON stablepay_analytics(provider_id, date DESC);

CREATE INDEX IF NOT EXISTS idx_stablepay_webhooks_provider ON stablepay_webhooks(provider_id);
CREATE INDEX IF NOT EXISTS idx_stablepay_webhooks_status ON stablepay_webhooks(status) WHERE status IN ('pending', 'failed');
CREATE INDEX IF NOT EXISTS idx_stablepay_webhooks_received ON stablepay_webhooks(received_at DESC);

-- Insert default provider (for demo purposes)
INSERT INTO stablepay_providers (
    id, 
    name, 
    provider_type, 
    fee_percentage, 
    fee_fixed,
    supported_currencies,
    supported_countries,
    priority
) VALUES 
    (gen_random_uuid(), 'StablePay Direct', 'direct', 0.0150, 0.30, ARRAY['USD', 'EUR', 'GBP', 'JPY', 'CAD', 'AUD'], ARRAY['US', 'CA', 'GB', 'EU', 'JP', 'AU'], 1),
    (gen_random_uuid(), 'Stripe', 'stripe', 0.0290, 0.30, ARRAY['USD', 'EUR', 'GBP'], ARRAY['US', 'CA', 'GB', 'EU'], 10),
    (gen_random_uuid(), 'PayPal', 'paypal', 0.0349, 0.49, ARRAY['USD', 'EUR', 'GBP'], ARRAY['US', 'CA', 'GB', 'EU'], 20)
ON CONFLICT DO NOTHING;

