# Neural Commerce Grid: The Path to Disruptive Intelligent Commerce

**Status**: Phase 3+ Complete (5 Active Agents)
**Goal**: Transform Stateset from a "Headless Commerce API" into the world's first "AI-Native Commerce Grid".

## 🚀 Execution Roadmap

### ✅ Phase 1: The Brain & Memory (Completed)
*   [x] Integrate `qdrant-client` & `async-openai`.
*   [x] Implement "Semantic Search" endpoint (`POST /neural/search`).

### ✅ Phase 2: The Voice (Completed)
*   [x] Build `CognitiveService` (LLM Gateway).
*   [x] Implement `ChatService` (RAG Orchestrator).
*   [x] Create "Chat with Inventory" endpoint (`POST /neural/chat`).

### ✅ Phase 3: The Hands (Completed)
*   **Goal**: Autonomous Action.
*   **Status**:
    *   [x] **Inventory Agent**: Monitors stock & sales trends -> Drafts Purchase Orders.
    *   [x] **Return Agent**: Monitors return requests -> Analyzes reasons -> Flags Quality Alerts.
    *   [x] **Fraud Agent**: Monitors completed orders -> Analyzes risk patterns -> Flags Fraud.
    *   [x] **Recovery Agent**: Monitors abandoned carts -> Sends Recovery Messages.
    *   [x] **Pricing Agent**: Monitors demand & competitors -> Optimizes Prices.

## 🛠️ Usage

**Environment Variables Required:**
```bash
export OPENAI_API_KEY="sk-..."
export QDRANT_URL="http://localhost:6334"
export INVENTORY_AGENT_INTERVAL_SECONDS=300
export RETURN_AGENT_INTERVAL_SECONDS=60
export FRAUD_AGENT_INTERVAL_SECONDS=30
export RECOVERY_AGENT_INTERVAL_SECONDS=300
export PRICING_AGENT_INTERVAL_SECONDS=3600
```

**Autonomous Behavior:**
*   **PricingAgent** runs hourly, checking demand/stock and adjusting prices to maximize revenue.
*   **RecoveryAgent** finds abandoned carts and engages users.
*   **FraudAgent** protects transactions in near real-time.
*   **ReturnAgent** ensures product quality feedback loops.
*   **InventoryAgent** keeps the shelves stocked.