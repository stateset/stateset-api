# StateSet Market Overview and Development Plan

## 1. Executive Summary

StateSet is a brand‑new, open‑source iCommerce operating system: a self‑hostable, API‑first backend for modern commerce and operations. The platform unifies order management, inventory, returns, warranties, shipments, procurement, and manufacturing execution, and pairs that operational depth with two differentiators:

- **Agentic Commerce Protocol (ACP) integration** for AI‑native shopping and checkout inside ChatGPT and other agentic surfaces.
- **StablePay** stablecoin/crypto payment rails to reduce cross‑border payment cost and settlement time.

The near‑term objective is to win enterprise and high‑growth brands by replacing expensive, fragmented OMS/SCM stacks and by enabling AI‑driven checkout and operations automation.

## 2. Current Product Status (2025)

### 2.1 Platform Maturity

The `stateset-api` repository is at `v0.2.1` and ships a production‑ready Rust backend:

- **Core domains:** OMS, inventory allocation and reservations, returns/RMA, warranties, shipments/ASN, purchase orders, work orders/BOM, analytics.
- **Interfaces:** REST JSON API, gRPC services, event outbox pattern, and webhook infrastructure (roadmap items expanding this).
- **Production traits:** RBAC/JWT/API keys, idempotency, rate limiting, structured logging, Prometheus metrics, and OpenTelemetry hooks.
- **Deployment:** Dockerized, multi‑stage build, SQLite for local dev and PostgreSQL for production.

### 2.2 ACP and Agentic Ops

The `agentic_server/` workspace provides a standalone ACP‑compliant server for ChatGPT Instant Checkout:

- Full ACP checkout session lifecycle and delegated payments endpoint.
- Stateless, low‑latency, designed to hand off completed orders to StateSet as the system of record.

Agentic Ops (returns, subscriptions, inventory, procurement, fraud, and recovery agents) is positioned as the automation layer that sits on top of the StateSet operational core.

## 3. Market Opportunity

### 3.1 TAM / SAM / SOM

- **TAM:** Global ecommerce GMV is approximately **$6.3T (2024)**.
- **SAM:** Enterprise retail and marketplaces ($50M+ GMV brands) represent roughly **$1.5T** of GMV that needs deeper OMS/SCM/automation than storefront‑only platforms.
- **Initial SOM (Years 1–3):** Shopify Plus and Salesforce Commerce ecosystems combined represent **~$200B GMV** of reachable headless/enterprise brands.

### 3.2 Customer Pain

For a representative $100M GMV enterprise:

- **Payments:** 2.9% card fees plus 1–2% FX on cross‑border sales, often $3–4M/year.
- **Manual operations:** returns, inventory exceptions, procurement, and fraud workflows typically require 15–30 FTEs.

StateSet targets **70–85% cost reduction** through StablePay rails and autonomous Agentic Ops.

## 4. Competitive Positioning

### 4.1 Where StateSet Wins

- **Operational breadth:** native OMS + SCM + manufacturing execution in one open platform.
- **Ownership and flexibility:** self‑hosted and open‑source; brands can extend data models and workflows.
- **AI‑native commerce:** ACP checkout and autonomous ops are first‑class components, not add‑ons.
- **Payment economics:** StablePay enables low‑fee, instant settlement cross‑border volume.

### 4.2 Known Gaps

- No turnkey storefront or merchant UI; StateSet is headless and integrates into existing frontends and ERPs.
- Ecosystem and marketplace are early relative to major SaaS platforms; integrations are a key roadmap focus.

## 5. Go‑To‑Market Plan

### Phase 1: Cross‑Border Stablecoin Payments (Weeks 1–4 per customer)

- Target Shopify Plus/Salesforce brands with high international mix.
- Deliver immediate savings by routing stablecoin volume through StablePay.
- Implementation path: app/connector install + payment routing + reconciliation.

### Phase 2: Autonomous Returns and Subscriptions (Weeks 5–8)

- Deploy returns and subscription agents on top of StateSet workflows.
- Reduce CS and ops FTEs while improving resolution times.

### Phase 3: Full Agent Suite + Platform Expansion (Weeks 9–12 and ongoing)

- Add inventory, procurement, fraud, and recovery agents.
- Expand integrations (carriers, ERPs, marketplaces) and make StateSet the primary execution backbone.

Distribution is bottoms‑up via app stores, top‑down via enterprise sales, and amplified through SI/agency partnerships.

## 6. Development Roadmap (Year‑by‑Year)

Roadmap features align with `docs/ROADMAP.md` and expand the platform into a full iCommerce OS.

### 2025 (Launch / Year 0)

- Ship v0.2.x maturity items: GraphQL, multi‑tenancy foundations, webhooks, bulk ops, SDK generation.
- Tighten ACP handoff paths, publish ACP reference integrations.
- Release initial Shopify/Salesforce connectors.

### 2026 (Year 1)

- v0.3 enterprise security/compliance (SSO, audit logging, encryption, GDPR tooling).
- Scale foundations: read replicas, horizontal scaling, caching, connection pooling.
- StablePay multi‑currency and PSP partnerships.

### 2027 (Year 2)

- v0.4 AI/automation: 12+ agents, natural‑language API, predictive analytics, workflow builder.
- Integration ecosystem expansion: Amazon FBA, ERPs, carriers, accounting, and marketplace sync.

### 2028 (Year 3)

- v0.5 global expansion: multi‑region control plane, localization, tax engines, mobile/edge SDKs.
- Push to v1.0 stability and long‑term support.
- Add marketplace, loyalty, subscription management, and gift card modules.

## 7. Scale Projections (Customers, GMV, Transactions, ACP Volume)

These are base‑case estimates and should be refined quarterly as pipeline and product telemetry solidify.

Assumptions:

- Average order value (AOV) ~ $100.
- ACP share grows as agentic checkout adoption increases in AI surfaces.
- GMV approximates payment volume for StablePay‑enabled flows.

| Year | Customers | GMV Through StateSet | Total Transactions | ACP Share | ACP Transactions |
|---|---:|---:|---:|---:|---:|
| 2025 (Y0) | 5–10 design partners + ~100 OSS installs | ~$0.5B | ~5M | ~0.5% | ~25k |
| 2026 (Y1) | ~50 | ~$5B | ~50M | ~2% | ~1M |
| 2027 (Y2) | ~200 | ~$20B | ~200M | ~6% | ~12M |
| 2028 (Y3) | ~800 | ~$80B | ~800M | ~15% | ~120M |

Longer term expectation is that StateSet becomes a global execution layer for agentic commerce, where a meaningful share of AI‑initiated shopping flows route through ACP and settle via StablePay, with StateSet maintaining authoritative order, inventory, and fulfillment state.

## 8. Key Risks and Mitigations

- **Regulatory change in crypto payments:** use licensed custodial partners and retain traditional payment fallback.
- **Buyer and merchant adoption of AI checkout:** start with high‑intent categories and enterprise pilots; publish reference UX patterns.
- **Agent mistakes or operational drift:** staged rollout with policy constraints, audit trails, and gradual autonomy increases.
- **Integration depth vs incumbents:** prioritize connectors and migration tooling to reduce switching cost.

## 9. Next Steps

- Convert this overview into a quarterly KPI dashboard tied to actual platform telemetry (orders created, payments processed, ACP sessions completed).
- Publish a public roadmap and OSS issue board to align community and commercial priorities.
- Validate assumptions with the first 5–10 design partners and update the Year 1 model accordingly.

