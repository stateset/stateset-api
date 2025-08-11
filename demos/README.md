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