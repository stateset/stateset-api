-- Create inventory_transactions table for full audit trail
CREATE TABLE public.inventory_transactions (
  transaction_id BIGSERIAL PRIMARY KEY,
  inventory_item_id BIGINT NOT NULL,
  location_id INTEGER NOT NULL,
  transaction_type VARCHAR(50) NOT NULL CHECK (transaction_type IN (
    'ADJUST', 'RESERVE', 'RELEASE', 'TRANSFER_OUT', 'TRANSFER_IN',
    'RECEIVE', 'SHIP', 'CYCLE_COUNT', 'SCRAP', 'QUARANTINE', 'RELEASE_FROM_QUARANTINE'
  )),
  quantity_delta NUMERIC(19, 4) NOT NULL,
  quantity_before NUMERIC(19, 4) NOT NULL,
  quantity_after NUMERIC(19, 4) NOT NULL,
  reason_code VARCHAR(100),
  reason_description TEXT,
  reference_id UUID,
  reference_type VARCHAR(50),
  reservation_id UUID,
  related_transaction_id BIGINT,
  idempotency_key VARCHAR(255) UNIQUE,
  created_by VARCHAR(255),
  metadata JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  FOREIGN KEY (inventory_item_id) REFERENCES public.item_master (inventory_item_id) ON DELETE CASCADE,
  FOREIGN KEY (location_id) REFERENCES public.inventory_locations (location_id) ON DELETE RESTRICT,
  FOREIGN KEY (reservation_id) REFERENCES public.inventory_reservations (reservation_id) ON DELETE SET NULL,
  FOREIGN KEY (related_transaction_id) REFERENCES public.inventory_transactions (transaction_id) ON DELETE SET NULL
);

-- Indexes for efficient querying and reporting
CREATE INDEX idx_inventory_transactions_item
  ON public.inventory_transactions(inventory_item_id);

CREATE INDEX idx_inventory_transactions_location
  ON public.inventory_transactions(location_id);

CREATE INDEX idx_inventory_transactions_item_location
  ON public.inventory_transactions(inventory_item_id, location_id);

CREATE INDEX idx_inventory_transactions_type
  ON public.inventory_transactions(transaction_type);

CREATE INDEX idx_inventory_transactions_created_at
  ON public.inventory_transactions(created_at DESC);

CREATE INDEX idx_inventory_transactions_reference
  ON public.inventory_transactions(reference_type, reference_id);

CREATE INDEX idx_inventory_transactions_idempotency
  ON public.inventory_transactions(idempotency_key)
  WHERE idempotency_key IS NOT NULL;

-- Trigger to automatically create transaction records (optional - can be done in application layer)
CREATE OR REPLACE FUNCTION log_inventory_change()
RETURNS TRIGGER AS $$
DECLARE
  delta NUMERIC;
BEGIN
  IF TG_OP = 'UPDATE' THEN
    IF NEW.quantity_on_hand != OLD.quantity_on_hand THEN
      delta := NEW.quantity_on_hand - OLD.quantity_on_hand;

      INSERT INTO public.inventory_transactions (
        inventory_item_id,
        location_id,
        transaction_type,
        quantity_delta,
        quantity_before,
        quantity_after,
        reason_code,
        created_at
      ) VALUES (
        NEW.inventory_item_id,
        NEW.location_id,
        'ADJUST',
        delta,
        OLD.quantity_on_hand,
        NEW.quantity_on_hand,
        'SYSTEM_TRIGGER',
        now()
      );
    END IF;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Optionally enable automatic transaction logging
-- CREATE TRIGGER trg_inventory_balance_change
-- AFTER UPDATE ON public.inventory_balances
-- FOR EACH ROW
-- EXECUTE FUNCTION log_inventory_change();

-- Comments
COMMENT ON TABLE public.inventory_transactions IS 'Complete audit trail of all inventory movements and changes';
COMMENT ON COLUMN public.inventory_transactions.idempotency_key IS 'Unique key to prevent duplicate transactions';
COMMENT ON COLUMN public.inventory_transactions.metadata IS 'Additional transaction metadata in JSON format';
COMMENT ON COLUMN public.inventory_transactions.related_transaction_id IS 'Links related transactions (e.g., transfer out/in pair)';
