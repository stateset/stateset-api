-- Create returns table
CREATE TABLE IF NOT EXISTS returns (
    id UUID PRIMARY KEY,
    created_date TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    amount DECIMAL(19,4) NOT NULL CHECK (amount >= 0),
    action_needed VARCHAR(32) NOT NULL DEFAULT 'None',
    condition VARCHAR(32) NOT NULL DEFAULT 'New',
    customer_email VARCHAR(255) NOT NULL,
    customer_id UUID NOT NULL,
    description TEXT,
    entered_by UUID,
    flat_rate_shipping DECIMAL(19,4) NOT NULL DEFAULT 0.0 CHECK (flat_rate_shipping >= 0),
    order_date TIMESTAMPTZ NOT NULL,
    order_id UUID NOT NULL,
    reason_category VARCHAR(255),
    reported_condition VARCHAR(32),
    requested_date TIMESTAMPTZ NOT NULL,
    rma VARCHAR(255) NOT NULL,
    serial_number VARCHAR(100),
    shipped_date TIMESTAMPTZ,
    status VARCHAR(32) NOT NULL DEFAULT 'Requested',
    tax_refunded DECIMAL(19,4) NOT NULL DEFAULT 0.0 CHECK (tax_refunded >= 0),
    total_refunded DECIMAL(19,4) NOT NULL DEFAULT 0.0 CHECK (total_refunded >= 0),
    tracking_number VARCHAR(100)
);

-- Create indexes for better performance
CREATE INDEX IF NOT EXISTS idx_returns_customer_id ON returns(customer_id);
CREATE INDEX IF NOT EXISTS idx_returns_order_id ON returns(order_id);
CREATE INDEX IF NOT EXISTS idx_returns_status ON returns(status);
CREATE INDEX IF NOT EXISTS idx_returns_rma ON returns(rma);
CREATE INDEX IF NOT EXISTS idx_returns_created_date ON returns(created_date DESC);
CREATE INDEX IF NOT EXISTS idx_returns_customer_email ON returns(customer_email);