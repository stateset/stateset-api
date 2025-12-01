-- Create optimized view for low stock items
CREATE MATERIALIZED VIEW mv_low_stock_items AS
SELECT
  im.inventory_item_id,
  im.item_number,
  im.description,
  im.primary_uom_code,
  im.organization_id,
  SUM(ib.quantity_on_hand) as total_on_hand,
  SUM(ib.quantity_allocated) as total_allocated,
  SUM(ib.quantity_available) as total_available,
  ARRAY_AGG(
    jsonb_build_object(
      'location_id', ib.location_id,
      'location_name', il.location_name,
      'quantity_on_hand', ib.quantity_on_hand,
      'quantity_allocated', ib.quantity_allocated,
      'quantity_available', ib.quantity_available,
      'reorder_point', ib.reorder_point,
      'safety_stock', ib.safety_stock
    )
  ) as locations,
  MIN(ib.quantity_available) as min_location_available,
  MAX(ib.reorder_point) as max_reorder_point,
  CASE
    WHEN MIN(ib.quantity_available) <= MAX(ib.safety_stock) THEN 'CRITICAL'
    WHEN SUM(ib.quantity_available) <= MAX(ib.reorder_point) THEN 'LOW'
    ELSE 'NORMAL'
  END as stock_level,
  MAX(ib.updated_at) as last_updated
FROM public.item_master im
INNER JOIN public.inventory_balances ib ON im.inventory_item_id = ib.inventory_item_id
LEFT JOIN public.inventory_locations il ON ib.location_id = il.location_id
WHERE ib.deleted_at IS NULL
GROUP BY im.inventory_item_id, im.item_number, im.description, im.primary_uom_code, im.organization_id;

-- Create index on the materialized view
CREATE INDEX idx_mv_low_stock_items_available
  ON mv_low_stock_items(total_available);

CREATE INDEX idx_mv_low_stock_items_stock_level
  ON mv_low_stock_items(stock_level);

CREATE INDEX idx_mv_low_stock_items_item_number
  ON mv_low_stock_items(item_number);

-- Create view for inventory valuation
CREATE OR REPLACE VIEW v_inventory_valuation AS
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
  COALESCE(
    (SELECT AVG(unit_cost)
     FROM public.inventory_lots
     WHERE inventory_item_id = im.inventory_item_id
       AND location_id = ib.location_id
       AND status IN ('AVAILABLE', 'ALLOCATED')
       AND unit_cost IS NOT NULL),
    0
  ) as average_unit_cost,
  ib.quantity_on_hand * COALESCE(
    (SELECT AVG(unit_cost)
     FROM public.inventory_lots
     WHERE inventory_item_id = im.inventory_item_id
       AND location_id = ib.location_id
       AND status IN ('AVAILABLE', 'ALLOCATED')
       AND unit_cost IS NOT NULL),
    0
  ) as total_value,
  ib.updated_at
FROM public.item_master im
INNER JOIN public.inventory_balances ib ON im.inventory_item_id = ib.inventory_item_id
LEFT JOIN public.inventory_locations il ON ib.location_id = il.location_id
WHERE ib.deleted_at IS NULL
  AND ib.quantity_on_hand > 0;

-- Create view for inventory movement summary
CREATE OR REPLACE VIEW v_inventory_movement_summary AS
SELECT
  inventory_item_id,
  location_id,
  DATE(created_at) as movement_date,
  transaction_type,
  COUNT(*) as transaction_count,
  SUM(CASE WHEN quantity_delta > 0 THEN quantity_delta ELSE 0 END) as total_inbound,
  SUM(CASE WHEN quantity_delta < 0 THEN ABS(quantity_delta) ELSE 0 END) as total_outbound,
  SUM(quantity_delta) as net_change
FROM public.inventory_transactions
WHERE created_at >= CURRENT_DATE - INTERVAL '90 days'
GROUP BY inventory_item_id, location_id, DATE(created_at), transaction_type;

-- Create view for active reservations summary
CREATE OR REPLACE VIEW v_active_reservations_summary AS
SELECT
  ir.inventory_item_id,
  im.item_number,
  im.description,
  ir.location_id,
  il.location_name,
  COUNT(*) as active_reservation_count,
  SUM(ir.quantity) as total_reserved_quantity,
  MIN(ir.reserved_at) as oldest_reservation,
  MAX(ir.reserved_at) as newest_reservation,
  COUNT(CASE WHEN ir.expires_at IS NOT NULL AND ir.expires_at < now() + INTERVAL '24 hours' THEN 1 END) as expiring_soon_count
FROM public.inventory_reservations ir
INNER JOIN public.item_master im ON ir.inventory_item_id = im.inventory_item_id
LEFT JOIN public.inventory_locations il ON ir.location_id = il.location_id
WHERE ir.status = 'ACTIVE'
GROUP BY ir.inventory_item_id, im.item_number, im.description, ir.location_id, il.location_name;

-- Create function to refresh materialized view
CREATE OR REPLACE FUNCTION refresh_inventory_views()
RETURNS void AS $$
BEGIN
  REFRESH MATERIALIZED VIEW CONCURRENTLY mv_low_stock_items;
END;
$$ LANGUAGE plpgsql;

-- Create index for faster refresh
CREATE UNIQUE INDEX idx_mv_low_stock_items_unique
  ON mv_low_stock_items(inventory_item_id);

-- Comments
COMMENT ON MATERIALIZED VIEW mv_low_stock_items IS 'Materialized view for fast low-stock queries. Refresh periodically with refresh_inventory_views()';
COMMENT ON VIEW v_inventory_valuation IS 'Current inventory value based on average lot costs';
COMMENT ON VIEW v_inventory_movement_summary IS 'Daily summary of inventory movements by type';
COMMENT ON VIEW v_active_reservations_summary IS 'Summary of active inventory reservations';
