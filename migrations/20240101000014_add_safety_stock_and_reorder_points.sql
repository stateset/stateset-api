-- Add safety stock and reorder point columns to inventory_balances
ALTER TABLE public.inventory_balances
  ADD COLUMN reorder_point NUMERIC(19, 4) DEFAULT 0 CHECK (reorder_point >= 0),
  ADD COLUMN safety_stock NUMERIC(19, 4) DEFAULT 0 CHECK (safety_stock >= 0),
  ADD COLUMN reorder_quantity NUMERIC(19, 4) CHECK (reorder_quantity IS NULL OR reorder_quantity > 0),
  ADD COLUMN max_stock_level NUMERIC(19, 4) CHECK (max_stock_level IS NULL OR max_stock_level > 0),
  ADD COLUMN lead_time_days INTEGER CHECK (lead_time_days IS NULL OR lead_time_days >= 0);

-- Add version column for optimistic locking
ALTER TABLE public.inventory_balances
  ADD COLUMN version INTEGER NOT NULL DEFAULT 1;

-- Add soft delete support
ALTER TABLE public.inventory_balances
  ADD COLUMN deleted_at TIMESTAMPTZ,
  ADD COLUMN deleted_by VARCHAR(255);

-- Add last_counted_at for cycle counting
ALTER TABLE public.inventory_balances
  ADD COLUMN last_counted_at TIMESTAMPTZ,
  ADD COLUMN last_counted_by VARCHAR(255);

-- Create index for non-deleted records
CREATE INDEX idx_inventory_balances_active
  ON public.inventory_balances(inventory_item_id, location_id)
  WHERE deleted_at IS NULL;

-- Create view for reorder recommendations
CREATE OR REPLACE VIEW v_reorder_recommendations AS
SELECT
  im.inventory_item_id,
  im.item_number,
  im.description,
  im.organization_id,
  ib.location_id,
  il.location_name,
  ib.quantity_on_hand,
  ib.quantity_allocated,
  ib.quantity_available,
  ib.reorder_point,
  ib.safety_stock,
  ib.reorder_quantity,
  ib.max_stock_level,
  ib.lead_time_days,
  CASE
    WHEN ib.quantity_available <= ib.safety_stock THEN 'CRITICAL'
    WHEN ib.quantity_available <= ib.reorder_point THEN 'REORDER'
    ELSE 'NORMAL'
  END as stock_status,
  GREATEST(
    COALESCE(ib.reorder_quantity, 0),
    COALESCE(ib.max_stock_level, 0) - ib.quantity_on_hand
  ) as recommended_order_quantity
FROM public.item_master im
INNER JOIN public.inventory_balances ib ON im.inventory_item_id = ib.inventory_item_id
LEFT JOIN public.inventory_locations il ON ib.location_id = il.location_id
WHERE ib.deleted_at IS NULL
  AND ib.quantity_available <= ib.reorder_point;

-- Comments
COMMENT ON COLUMN public.inventory_balances.reorder_point IS 'Quantity level that triggers reorder alert';
COMMENT ON COLUMN public.inventory_balances.safety_stock IS 'Minimum buffer quantity to maintain';
COMMENT ON COLUMN public.inventory_balances.reorder_quantity IS 'Quantity to reorder when hitting reorder point';
COMMENT ON COLUMN public.inventory_balances.max_stock_level IS 'Maximum desired inventory level';
COMMENT ON COLUMN public.inventory_balances.version IS 'Version number for optimistic locking';
COMMENT ON COLUMN public.inventory_balances.deleted_at IS 'Timestamp of soft delete';
COMMENT ON VIEW v_reorder_recommendations IS 'Items that need reordering based on reorder points';
