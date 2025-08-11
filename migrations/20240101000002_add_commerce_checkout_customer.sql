-- Customer addresses table
CREATE TABLE IF NOT EXISTS customer_addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id UUID NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    name VARCHAR(255),
    company VARCHAR(255),
    address_line_1 VARCHAR(255) NOT NULL,
    address_line_2 VARCHAR(255),
    city VARCHAR(100) NOT NULL,
    province VARCHAR(100) NOT NULL,
    country_code CHAR(2) NOT NULL,
    postal_code VARCHAR(20) NOT NULL,
    phone VARCHAR(50),
    is_default_shipping BOOLEAN NOT NULL DEFAULT FALSE,
    is_default_billing BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Indexes for customer addresses
CREATE INDEX idx_customer_addresses_customer_id ON customer_addresses(customer_id);
CREATE INDEX idx_customer_addresses_default_shipping ON customer_addresses(customer_id, is_default_shipping) WHERE is_default_shipping = TRUE;
CREATE INDEX idx_customer_addresses_default_billing ON customer_addresses(customer_id, is_default_billing) WHERE is_default_billing = TRUE;

-- Update customers table to include address references
ALTER TABLE customers
ADD COLUMN IF NOT EXISTS default_shipping_address_id UUID REFERENCES customer_addresses(id),
ADD COLUMN IF NOT EXISTS default_billing_address_id UUID REFERENCES customer_addresses(id);

-- Update orders table to include commerce fields
ALTER TABLE orders
ADD COLUMN IF NOT EXISTS customer_email VARCHAR(255),
ADD COLUMN IF NOT EXISTS shipping_address JSONB,
ADD COLUMN IF NOT EXISTS billing_address JSONB,
ADD COLUMN IF NOT EXISTS payment_method VARCHAR(50),
ADD COLUMN IF NOT EXISTS payment_status VARCHAR(50),
ADD COLUMN IF NOT EXISTS transaction_id VARCHAR(255); 