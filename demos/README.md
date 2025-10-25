# Demos

This directory contains shell scripts that exercise various API flows.

## Agents Concierge Demo

Runs a simple flow using the Agents API:
- Fetch product recommendations
- Ask the agent to add an item to a cart on behalf of a customer

Run:

```bash
cd demos
chmod +x agents_concierge_demo.sh
API_URL=http://localhost:8080 ./agents_concierge_demo.sh
```

Notes:
- Requires the API server running (`cargo run` in project root).
- The demo uses random UUIDs for `customer_id` and `cart_id` since cart creation is not currently exposed via HTTP in `api_v1_routes`. The add-to-cart call will exercise the endpoint but may return a not found error if backing data is missing.
- Customize by exporting `AUTH_HEADER` if your server expects authentication, e.g. `AUTH_HEADER="Authorization: Bearer <token>"`. 

## Order & Shipment Demos

The following scripts exercise the commerce APIs for creating orders and shipments. All scripts expect a running API and valid authentication (set either `AUTH_TOKEN` for a Bearer token or `AUTH_HEADER` for a fully formed header). Optional environment variables let you override default payload values.

### `order_creation_demo.sh`

Creates a single order using sample payload data.

```bash
cd demos
chmod +x order_creation_demo.sh
API_URL=http://localhost:8080 AUTH_TOKEN=... ./order_creation_demo.sh
```

Key overrides:
- `ORDER_CUSTOMER_ID` – UUID to associate with the order (defaults to a new UUID)
- `ORDER_ITEM_ID` – Product variant identifier or SKU expected by your catalog
- `ORDER_ITEM_QUANTITY`, `ORDER_ITEM_PRICE` – Item quantity and unit price floats

### `shipment_creation_demo.sh`

POSTs a shipment for an existing order. Provide `SHIPMENT_ORDER_ID` (or `ORDER_ID`) so the call references a real order record.

```bash
cd demos
chmod +x shipment_creation_demo.sh
API_URL=http://localhost:8080 AUTH_TOKEN=... SHIPMENT_ORDER_ID=<order_uuid> ./shipment_creation_demo.sh
```

Other knobs:
- `SHIPMENT_METHOD` – Shipping method string (standard, express, overnight, two-day, international, custom)
- `SHIPMENT_TRACKING_NUMBER`, `SHIPMENT_ADDRESS`, `SHIPMENT_RECIPIENT` – Tracking and recipient details

### `order_to_shipment_flow.sh`

Runs the full flow: create an order then immediately create a shipment for the returned order id.

```bash
cd demos
chmod +x order_to_shipment_flow.sh
API_URL=http://localhost:8080 AUTH_TOKEN=... ./order_to_shipment_flow.sh
```

Environment overrides mirror the two standalone scripts (for example `FLOW_ITEM_ID`, `FLOW_SHIPPING_METHOD`, etc.). If the order creation step fails (missing catalog data or permissions), the script halts before attempting the shipment call.
