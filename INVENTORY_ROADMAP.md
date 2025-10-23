Inventory Roadmap

  - Schema Alignment
      - Update inventory_transactions/inventory_reservations tables (and SeaORM models) to use the same numeric inventory_item_id and location_id as item_master/inventory_locations, with
        proper foreign keys and supporting indexes.
      - Let Postgres own inventory_balances.quantity_available (generated column) by removing manual writes and adjusting ActiveModels.
  - Reservation Layer
      - Persist every reservation in inventory_reservations, carrying reference data, and reference those rows when releasing/cancelling.
      - Add consistent row-level locking (FOR UPDATE/LockType::Exclusive) around balance reads/updates so concurrent reservations cannot oversubscribe stock.
  - Service Refactors
      - Rework inventory_service, inventory_adjustment_service, and inventory_sync to:
          - centralize transaction logging (write real transaction rows with correct IDs/quantities),
          - rely on the DB for derived fields,
          - reuse the reservation records created above.
  - API & Tests
      - Expose reservation IDs in the inventory handlers (reserve/release endpoints) and plumb them through the service layer.
      - Extend integration/unit tests to cover reservation persistence, locking, and transaction logging so regressions are caught automatically.

  Once these pieces are in place, the inventory subsystem will have a coherent data model, durable reservation tracking, and reliable auditing across all services.