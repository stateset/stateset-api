-- Add version column to inventory_balances for optimistic locking
-- This prevents lost updates when multiple clients try to modify the same inventory record

-- Add version column if not exists
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'inventory_balances' AND column_name = 'version'
    ) THEN
        ALTER TABLE public.inventory_balances ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
    END IF;
END $$;

-- Add additional indexes for inventory performance optimization

-- Index for time-range queries on inventory balances
CREATE INDEX IF NOT EXISTS idx_inventory_balances_updated_at
    ON public.inventory_balances(updated_at);

-- Index for inventory lots by received date (for FIFO queries)
CREATE INDEX IF NOT EXISTS idx_inventory_lots_received_date
    ON public.inventory_lots(received_date)
    WHERE deleted_at IS NULL;

-- Index for inventory lots by manufacture date
CREATE INDEX IF NOT EXISTS idx_inventory_lots_manufacture_date
    ON public.inventory_lots(manufacture_date)
    WHERE deleted_at IS NULL AND manufacture_date IS NOT NULL;

-- Index for inventory reservations by status
CREATE INDEX IF NOT EXISTS idx_inventory_reservations_status
    ON public.inventory_reservations(status);

-- Index for inventory reservations by expiration (for cleanup queries)
CREATE INDEX IF NOT EXISTS idx_inventory_reservations_expires_at
    ON public.inventory_reservations(expires_at)
    WHERE status NOT IN ('cancelled', 'released', 'expired');

-- Index for inventory transactions by type
CREATE INDEX IF NOT EXISTS idx_inventory_transactions_type
    ON public.inventory_transactions(type);

-- Index for inventory transactions by reference (for order-related lookups)
CREATE INDEX IF NOT EXISTS idx_inventory_transactions_reference
    ON public.inventory_transactions(reference_id, reference_type)
    WHERE reference_id IS NOT NULL;

-- Composite index for low-stock queries
CREATE INDEX IF NOT EXISTS idx_inventory_balances_low_stock
    ON public.inventory_balances(inventory_item_id, location_id, quantity_available)
    WHERE deleted_at IS NULL;

-- Index for lot allocations by lot_id
CREATE INDEX IF NOT EXISTS idx_inventory_lot_allocations_lot
    ON public.inventory_lot_allocations(lot_id);

-- Index for lot allocations by reference (for order lookups)
CREATE INDEX IF NOT EXISTS idx_inventory_lot_allocations_reference
    ON public.inventory_lot_allocations(reference_id, reference_type)
    WHERE reference_id IS NOT NULL;

-- Partial index for active (non-cancelled) lot allocations
CREATE INDEX IF NOT EXISTS idx_inventory_lot_allocations_active
    ON public.inventory_lot_allocations(lot_id, quantity_allocated)
    WHERE cancelled_at IS NULL;

COMMENT ON COLUMN public.inventory_balances.version IS
    'Version number for optimistic locking. Incremented on each update to prevent lost updates in concurrent scenarios.';
