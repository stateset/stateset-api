-- Create orders table
CREATE TABLE IF NOT EXISTS orders (
    id UUID PRIMARY KEY,
    customer_id UUID NOT NULL,
    order_number VARCHAR(255) NOT NULL UNIQUE,
    total_amount DECIMAL(19,4) NOT NULL DEFAULT 0.0,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    payment_status VARCHAR(50) NOT NULL DEFAULT 'pending',
    fulfillment_status VARCHAR(50) NOT NULL DEFAULT 'unfulfilled',
    payment_method VARCHAR(100),
    shipping_method VARCHAR(100),
    notes TEXT,
    shipping_address TEXT,
    billing_address TEXT,
    tracking_number VARCHAR(255),
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    is_archived BOOLEAN NOT NULL DEFAULT false,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Create indexes for better performance
CREATE INDEX IF NOT EXISTS idx_orders_customer_id ON orders(customer_id);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
CREATE INDEX IF NOT EXISTS idx_orders_created_at ON orders(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_order_number ON orders(order_number);