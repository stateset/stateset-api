-- Create payments table
CREATE TABLE IF NOT EXISTS payments (
    id UUID PRIMARY KEY,
    order_id UUID NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    amount DECIMAL(19,4) NOT NULL,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    payment_method VARCHAR(50) NOT NULL,
    payment_method_id VARCHAR(255),
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    description TEXT,
    transaction_id VARCHAR(255),
    gateway_response JSONB,
    refunded_amount DECIMAL(19,4) NOT NULL DEFAULT 0,
    refund_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    processed_at TIMESTAMPTZ
);

-- Create indexes for payments table
CREATE INDEX IF NOT EXISTS idx_payments_order_id ON payments(order_id);
CREATE INDEX IF NOT EXISTS idx_payments_status ON payments(status);
CREATE INDEX IF NOT EXISTS idx_payments_created_at ON payments(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_payments_transaction_id ON payments(transaction_id) WHERE transaction_id IS NOT NULL;

-- Create ASN (Advanced Shipping Notice) tables
CREATE TABLE IF NOT EXISTS asns (
    id UUID PRIMARY KEY,
    asn_number VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    supplier_id UUID NOT NULL,
    supplier_name VARCHAR(255) NOT NULL,
    purchase_order_id UUID,
    expected_delivery_date DATE,
    shipping_date DATE,
    carrier_type VARCHAR(50),
    tracking_number VARCHAR(255),
    shipping_address TEXT,
    notes TEXT,
    created_by UUID,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Create ASN items table
CREATE TABLE IF NOT EXISTS asn_items (
    id UUID PRIMARY KEY,
    asn_id UUID NOT NULL REFERENCES asns(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    product_name VARCHAR(255) NOT NULL,
    product_sku VARCHAR(255),
    quantity_expected INTEGER NOT NULL,
    quantity_received INTEGER NOT NULL DEFAULT 0,
    quantity_rejected INTEGER NOT NULL DEFAULT 0,
    unit_price DECIMAL(19,4),
    line_total DECIMAL(19,4),
    condition VARCHAR(50),
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Create indexes for ASN tables
CREATE INDEX IF NOT EXISTS idx_asns_status ON asns(status);
CREATE INDEX IF NOT EXISTS idx_asns_supplier_id ON asns(supplier_id);
CREATE INDEX IF NOT EXISTS idx_asns_created_at ON asns(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_asn_items_asn_id ON asn_items(asn_id);
CREATE INDEX IF NOT EXISTS idx_asn_items_product_id ON asn_items(product_id);

-- Create purchase orders tables
CREATE TABLE IF NOT EXISTS purchase_orders (
    id UUID PRIMARY KEY,
    po_number VARCHAR(255) NOT NULL UNIQUE,
    supplier_id UUID NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    order_date TIMESTAMPTZ NOT NULL,
    expected_delivery_date DATE,
    total_amount DECIMAL(19,4) NOT NULL DEFAULT 0,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    shipping_address TEXT,
    billing_address TEXT,
    payment_terms VARCHAR(255),
    notes TEXT,
    created_by UUID NOT NULL,
    approved_by UUID,
    approved_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Create purchase order items table
CREATE TABLE IF NOT EXISTS purchase_order_items (
    id UUID PRIMARY KEY,
    purchase_order_id UUID NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    sku VARCHAR(255),
    product_name VARCHAR(255) NOT NULL,
    description TEXT,
    quantity_ordered INTEGER NOT NULL,
    quantity_received INTEGER NOT NULL DEFAULT 0,
    quantity_backordered INTEGER NOT NULL DEFAULT 0,
    unit_cost DECIMAL(19,4),
    line_total DECIMAL(19,4),
    tax_rate DECIMAL(5,4) DEFAULT 0,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Create indexes for purchase orders
CREATE INDEX IF NOT EXISTS idx_purchase_orders_supplier_id ON purchase_orders(supplier_id);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_status ON purchase_orders(status);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_created_at ON purchase_orders(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_purchase_order_items_po_id ON purchase_order_items(purchase_order_id);
CREATE INDEX IF NOT EXISTS idx_purchase_order_items_product_id ON purchase_order_items(product_id);

-- Create suppliers table if it doesn't exist
CREATE TABLE IF NOT EXISTS suppliers (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    contact_name VARCHAR(255),
    email VARCHAR(255),
    phone VARCHAR(50),
    address_line_1 VARCHAR(255),
    address_line_2 VARCHAR(255),
    city VARCHAR(255),
    state VARCHAR(255),
    postal_code VARCHAR(20),
    country VARCHAR(2),
    tax_id VARCHAR(50),
    payment_terms VARCHAR(100) DEFAULT 'Net 30',
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Create indexes for suppliers
CREATE INDEX IF NOT EXISTS idx_suppliers_name ON suppliers(name);
CREATE INDEX IF NOT EXISTS idx_suppliers_email ON suppliers(email);
CREATE INDEX IF NOT EXISTS idx_suppliers_status ON suppliers(status);

-- Insert some sample data for testing
INSERT INTO suppliers (id, name, contact_name, email, phone, payment_terms, status)
VALUES 
    (gen_random_uuid(), 'TechSupply Inc', 'John Smith', 'john@techsupply.com', '+1-555-0101', 'Net 30', 'active'),
    (gen_random_uuid(), 'GlobalParts Ltd', 'Sarah Johnson', 'sarah@globalparts.com', '+1-555-0102', 'Net 45', 'active')
ON CONFLICT DO NOTHING;

-- Create warehouses table
CREATE TABLE IF NOT EXISTS warehouses (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    code VARCHAR(50) NOT NULL UNIQUE,
    address_line_1 VARCHAR(255),
    address_line_2 VARCHAR(255),
    city VARCHAR(255),
    state VARCHAR(255),
    postal_code VARCHAR(20),
    country VARCHAR(2),
    contact_name VARCHAR(255),
    contact_phone VARCHAR(50),
    contact_email VARCHAR(255),
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    is_default BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Insert default warehouse
INSERT INTO warehouses (id, name, code, status, is_default)
VALUES (gen_random_uuid(), 'Main Warehouse', 'MAIN', 'active', true)
ON CONFLICT DO NOTHING;
