-- Create products table
CREATE TABLE IF NOT EXISTS products (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL UNIQUE,
    description TEXT NOT NULL,
    status VARCHAR(20) NOT NULL,
    product_type VARCHAR(20) NOT NULL,
    attributes JSONB NOT NULL DEFAULT '[]',
    seo JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_products_slug ON products(slug);
CREATE INDEX idx_products_status ON products(status);
CREATE INDEX idx_products_created_at ON products(created_at DESC);

-- Create product_variants table
CREATE TABLE IF NOT EXISTS product_variants (
    id UUID PRIMARY KEY,
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    sku VARCHAR(255) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    price DECIMAL(19,4) NOT NULL,
    compare_at_price DECIMAL(19,4),
    cost DECIMAL(19,4),
    weight DOUBLE PRECISION,
    dimensions JSONB,
    options JSONB NOT NULL DEFAULT '{}',
    inventory_tracking BOOLEAN NOT NULL DEFAULT true,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_product_variants_product_id ON product_variants(product_id);
CREATE INDEX idx_product_variants_sku ON product_variants(sku);

-- Create categories table
CREATE TABLE IF NOT EXISTS categories (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    parent_id UUID REFERENCES categories(id),
    position INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- Create product_categories junction table
CREATE TABLE IF NOT EXISTS product_categories (
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    category_id UUID NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    PRIMARY KEY (product_id, category_id)
);

-- Create customers table (enhanced)
CREATE TABLE IF NOT EXISTS customers (
    id UUID PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    first_name VARCHAR(255) NOT NULL,
    last_name VARCHAR(255) NOT NULL,
    phone VARCHAR(50),
    accepts_marketing BOOLEAN NOT NULL DEFAULT false,
    customer_group_id UUID,
    default_shipping_address_id UUID,
    default_billing_address_id UUID,
    tags JSONB NOT NULL DEFAULT '[]',
    metadata JSONB,
    email_verified BOOLEAN NOT NULL DEFAULT false,
    email_verified_at TIMESTAMPTZ,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_customers_email ON customers(email);
CREATE INDEX idx_customers_status ON customers(status);

-- Create customer_addresses table
CREATE TABLE IF NOT EXISTS customer_addresses (
    id UUID PRIMARY KEY,
    customer_id UUID NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    name VARCHAR(255),
    company VARCHAR(255),
    address_line_1 VARCHAR(255) NOT NULL,
    address_line_2 VARCHAR(255),
    city VARCHAR(255) NOT NULL,
    province VARCHAR(255),
    country_code VARCHAR(2) NOT NULL,
    postal_code VARCHAR(20),
    phone VARCHAR(50),
    is_default_shipping BOOLEAN NOT NULL DEFAULT false,
    is_default_billing BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_customer_addresses_customer_id ON customer_addresses(customer_id);

-- Create carts table
CREATE TABLE IF NOT EXISTS carts (
    id UUID PRIMARY KEY,
    session_id VARCHAR(255),
    customer_id UUID REFERENCES customers(id),
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    subtotal DECIMAL(19,4) NOT NULL DEFAULT 0,
    tax_total DECIMAL(19,4) NOT NULL DEFAULT 0,
    shipping_total DECIMAL(19,4) NOT NULL DEFAULT 0,
    discount_total DECIMAL(19,4) NOT NULL DEFAULT 0,
    total DECIMAL(19,4) NOT NULL DEFAULT 0,
    metadata JSONB,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_carts_session_id ON carts(session_id);
CREATE INDEX idx_carts_customer_id ON carts(customer_id);
CREATE INDEX idx_carts_status ON carts(status);
CREATE INDEX idx_carts_expires_at ON carts(expires_at);

-- Create cart_items table
CREATE TABLE IF NOT EXISTS cart_items (
    id UUID PRIMARY KEY,
    cart_id UUID NOT NULL REFERENCES carts(id) ON DELETE CASCADE,
    variant_id UUID NOT NULL REFERENCES product_variants(id),
    quantity INTEGER NOT NULL,
    unit_price DECIMAL(19,4) NOT NULL,
    line_total DECIMAL(19,4) NOT NULL,
    discount_amount DECIMAL(19,4) NOT NULL DEFAULT 0,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_cart_items_cart_id ON cart_items(cart_id);
CREATE UNIQUE INDEX idx_cart_items_cart_variant ON cart_items(cart_id, variant_id);

-- Create wishlists table
CREATE TABLE IF NOT EXISTS wishlists (
    id UUID PRIMARY KEY,
    customer_id UUID NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    is_public BOOLEAN NOT NULL DEFAULT false,
    share_token VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_wishlists_customer_id ON wishlists(customer_id);
CREATE INDEX idx_wishlists_share_token ON wishlists(share_token) WHERE share_token IS NOT NULL;

-- Create wishlist_items table
CREATE TABLE IF NOT EXISTS wishlist_items (
    id UUID PRIMARY KEY,
    wishlist_id UUID NOT NULL REFERENCES wishlists(id) ON DELETE CASCADE,
    variant_id UUID NOT NULL REFERENCES product_variants(id),
    added_at TIMESTAMPTZ NOT NULL,
    UNIQUE(wishlist_id, variant_id)
);

-- Add commerce-specific indexes for performance
CREATE INDEX idx_products_search ON products USING gin(to_tsvector('english', name || ' ' || description));
CREATE INDEX idx_product_variants_price ON product_variants(price);
CREATE INDEX idx_cart_items_variant_id ON cart_items(variant_id); 