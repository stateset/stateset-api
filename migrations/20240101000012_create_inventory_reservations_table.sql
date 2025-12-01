-- Create inventory_reservations table for tracking active reservations
CREATE TABLE public.inventory_reservations (
  reservation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  inventory_item_id BIGINT NOT NULL,
  location_id INTEGER NOT NULL,
  quantity NUMERIC(19, 4) NOT NULL CHECK (quantity > 0),
  reference_id UUID,
  reference_type VARCHAR(50),
  status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'FULFILLED', 'CANCELLED', 'EXPIRED')),
  reserved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ,
  fulfilled_at TIMESTAMPTZ,
  cancelled_at TIMESTAMPTZ,
  created_by VARCHAR(255),
  notes TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  FOREIGN KEY (inventory_item_id) REFERENCES public.item_master (inventory_item_id) ON DELETE CASCADE,
  FOREIGN KEY (location_id) REFERENCES public.inventory_locations (location_id) ON DELETE RESTRICT
);

-- Indexes for efficient querying
CREATE INDEX idx_inventory_reservations_item_location
  ON public.inventory_reservations(inventory_item_id, location_id)
  WHERE status = 'ACTIVE';

CREATE INDEX idx_inventory_reservations_reference
  ON public.inventory_reservations(reference_type, reference_id);

CREATE INDEX idx_inventory_reservations_status
  ON public.inventory_reservations(status);

CREATE INDEX idx_inventory_reservations_expires_at
  ON public.inventory_reservations(expires_at)
  WHERE status = 'ACTIVE' AND expires_at IS NOT NULL;

-- Function to automatically expire reservations
CREATE OR REPLACE FUNCTION expire_reservations()
RETURNS void AS $$
BEGIN
  UPDATE public.inventory_reservations
  SET status = 'EXPIRED', updated_at = now()
  WHERE status = 'ACTIVE'
    AND expires_at IS NOT NULL
    AND expires_at < now();
END;
$$ LANGUAGE plpgsql;

-- Comments for documentation
COMMENT ON TABLE public.inventory_reservations IS 'Tracks inventory reservations for orders, allocations, and other business processes';
COMMENT ON COLUMN public.inventory_reservations.reference_id IS 'External reference (e.g., order_id, work_order_id)';
COMMENT ON COLUMN public.inventory_reservations.reference_type IS 'Type of reference (e.g., ORDER, WORK_ORDER, TRANSFER)';
COMMENT ON COLUMN public.inventory_reservations.expires_at IS 'Optional expiration time for auto-releasing reservations';
