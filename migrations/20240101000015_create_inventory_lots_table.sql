-- Create inventory_lots table for batch/lot tracking
CREATE TABLE public.inventory_lots (
  lot_id BIGSERIAL PRIMARY KEY,
  inventory_item_id BIGINT NOT NULL,
  location_id INTEGER NOT NULL,
  lot_number VARCHAR(100) NOT NULL,
  quantity NUMERIC(19, 4) NOT NULL CHECK (quantity >= 0),
  original_quantity NUMERIC(19, 4) NOT NULL CHECK (original_quantity > 0),
  unit_cost NUMERIC(19, 4),
  expiration_date DATE,
  manufacture_date DATE,
  received_date DATE NOT NULL,
  supplier_lot_number VARCHAR(100),
  supplier_id BIGINT,
  po_number VARCHAR(50),
  po_line_id BIGINT,
  status VARCHAR(20) NOT NULL DEFAULT 'AVAILABLE' CHECK (status IN (
    'AVAILABLE', 'ALLOCATED', 'QUARANTINE', 'EXPIRED', 'CONSUMED', 'SCRAPPED'
  )),
  quality_status VARCHAR(20) DEFAULT 'PENDING' CHECK (quality_status IN (
    'PENDING', 'PASSED', 'FAILED', 'CONDITIONAL'
  )),
  quarantine_reason TEXT,
  quarantined_at TIMESTAMPTZ,
  quarantined_by VARCHAR(255),
  released_at TIMESTAMPTZ,
  released_by VARCHAR(255),
  notes TEXT,
  metadata JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  FOREIGN KEY (inventory_item_id) REFERENCES public.item_master (inventory_item_id) ON DELETE CASCADE,
  FOREIGN KEY (location_id) REFERENCES public.inventory_locations (location_id) ON DELETE RESTRICT,
  FOREIGN KEY (po_line_id) REFERENCES public.purchase_order_lines (po_line_id) ON DELETE SET NULL
);

-- Unique constraint: lot_number per item per location
CREATE UNIQUE INDEX idx_inventory_lots_unique_lot_number
  ON public.inventory_lots(inventory_item_id, location_id, lot_number)
  WHERE deleted_at IS NULL;

-- Indexes for efficient querying
CREATE INDEX idx_inventory_lots_item
  ON public.inventory_lots(inventory_item_id);

CREATE INDEX idx_inventory_lots_location
  ON public.inventory_lots(location_id);

CREATE INDEX idx_inventory_lots_status
  ON public.inventory_lots(status)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_inventory_lots_expiration
  ON public.inventory_lots(expiration_date)
  WHERE status IN ('AVAILABLE', 'ALLOCATED') AND expiration_date IS NOT NULL;

CREATE INDEX idx_inventory_lots_lot_number
  ON public.inventory_lots(lot_number);

-- Create lot_allocations table to track which lots are allocated to orders
CREATE TABLE public.inventory_lot_allocations (
  allocation_id BIGSERIAL PRIMARY KEY,
  lot_id BIGINT NOT NULL,
  reservation_id UUID,
  reference_type VARCHAR(50) NOT NULL,
  reference_id UUID NOT NULL,
  quantity_allocated NUMERIC(19, 4) NOT NULL CHECK (quantity_allocated > 0),
  allocated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  allocated_by VARCHAR(255),
  fulfilled_at TIMESTAMPTZ,
  cancelled_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  FOREIGN KEY (lot_id) REFERENCES public.inventory_lots (lot_id) ON DELETE CASCADE,
  FOREIGN KEY (reservation_id) REFERENCES public.inventory_reservations (reservation_id) ON DELETE SET NULL
);

-- Indexes for lot allocations
CREATE INDEX idx_inventory_lot_allocations_lot
  ON public.inventory_lot_allocations(lot_id);

CREATE INDEX idx_inventory_lot_allocations_reservation
  ON public.inventory_lot_allocations(reservation_id);

CREATE INDEX idx_inventory_lot_allocations_reference
  ON public.inventory_lot_allocations(reference_type, reference_id);

-- View for expiring lots
CREATE OR REPLACE VIEW v_expiring_lots AS
SELECT
  il.lot_id,
  il.inventory_item_id,
  im.item_number,
  im.description,
  il.location_id,
  loc.location_name,
  il.lot_number,
  il.quantity,
  il.expiration_date,
  il.status,
  CASE
    WHEN il.expiration_date < CURRENT_DATE THEN 'EXPIRED'
    WHEN il.expiration_date <= CURRENT_DATE + INTERVAL '7 days' THEN 'EXPIRING_SOON'
    WHEN il.expiration_date <= CURRENT_DATE + INTERVAL '30 days' THEN 'EXPIRING'
    ELSE 'NORMAL'
  END as expiry_status,
  il.expiration_date - CURRENT_DATE as days_until_expiry
FROM public.inventory_lots il
INNER JOIN public.item_master im ON il.inventory_item_id = im.inventory_item_id
LEFT JOIN public.inventory_locations loc ON il.location_id = loc.location_id
WHERE il.deleted_at IS NULL
  AND il.status IN ('AVAILABLE', 'ALLOCATED')
  AND il.expiration_date IS NOT NULL
  AND il.expiration_date <= CURRENT_DATE + INTERVAL '30 days'
ORDER BY il.expiration_date ASC;

-- Function to automatically expire lots
CREATE OR REPLACE FUNCTION expire_inventory_lots()
RETURNS void AS $$
BEGIN
  UPDATE public.inventory_lots
  SET status = 'EXPIRED', updated_at = now()
  WHERE status IN ('AVAILABLE', 'ALLOCATED')
    AND expiration_date IS NOT NULL
    AND expiration_date < CURRENT_DATE
    AND deleted_at IS NULL;
END;
$$ LANGUAGE plpgsql;

-- Comments
COMMENT ON TABLE public.inventory_lots IS 'Tracks individual batches/lots of inventory with expiration dates and traceability';
COMMENT ON COLUMN public.inventory_lots.lot_number IS 'Internal lot/batch number for tracking';
COMMENT ON COLUMN public.inventory_lots.supplier_lot_number IS 'Supplier''s lot/batch number';
COMMENT ON COLUMN public.inventory_lots.quality_status IS 'Quality control status';
COMMENT ON TABLE public.inventory_lot_allocations IS 'Tracks which lots are allocated to specific orders or work orders';
COMMENT ON VIEW v_expiring_lots IS 'Lots that are expired or expiring soon';
