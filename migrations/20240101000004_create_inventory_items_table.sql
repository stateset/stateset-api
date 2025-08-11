-- Create inventory_items table
CREATE TABLE IF NOT EXISTS inventory_items (
    id TEXT PRIMARY KEY,
    sku TEXT NOT NULL,
    description TEXT NOT NULL,
    size TEXT NOT NULL,
    incoming INTEGER NOT NULL DEFAULT 0,
    color TEXT NOT NULL,
    warehouse INTEGER NOT NULL,
    arriving DATE NOT NULL,
    purchase_order_id TEXT NOT NULL,
    available INTEGER NOT NULL DEFAULT 0,
    delivery_date DATE NOT NULL,
    arrival_date DATE NOT NULL,
    upc TEXT NOT NULL,
    restock_date DATE,
    lot_number TEXT,
    expiration_date TIMESTAMPTZ,
    unit_cost DECIMAL(19,4),
    cogs_amount DECIMAL(19,4),
    cogs_currency TEXT,
    cogs_exchange_rate_id TEXT,
    cogs_last_updated TIMESTAMPTZ,
    cogs_method TEXT,
    total_value DECIMAL(19,4),
    average_cost DECIMAL(19,4),
    fifo_layers JSONB,
    lifo_layers JSONB,
    quality_status TEXT,
    sustainability_score DECIMAL(10,4),
    last_stocktake_date DATE,
    stocktake_quantity INTEGER,
    stocktake_variance DECIMAL(19,4),
    allocated_quantity INTEGER DEFAULT 0,
    reserved_quantity INTEGER DEFAULT 0,
    damaged_quantity INTEGER DEFAULT 0,
    manufacturing_date DATE,
    supplier_id TEXT,
    cost_center TEXT,
    abc_classification TEXT,
    turnover_rate DECIMAL(10,4),
    reorder_point INTEGER,
    economic_order_quantity INTEGER,
    safety_stock_level INTEGER,
    weight DECIMAL(10,4),
    weight_unit TEXT,
    volume DECIMAL(10,4),
    volume_unit TEXT,
    location_in_warehouse TEXT,
    last_movement_date TIMESTAMPTZ
);

-- Create indexes for better performance
CREATE INDEX IF NOT EXISTS idx_inventory_items_sku ON inventory_items(sku);
CREATE INDEX IF NOT EXISTS idx_inventory_items_warehouse ON inventory_items(warehouse);
CREATE INDEX IF NOT EXISTS idx_inventory_items_available ON inventory_items(available);
CREATE INDEX IF NOT EXISTS idx_inventory_items_reorder ON inventory_items(reorder_point) WHERE reorder_point IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_inventory_items_last_movement ON inventory_items(last_movement_date DESC);