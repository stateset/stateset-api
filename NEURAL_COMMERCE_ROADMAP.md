# Neural Commerce Grid: The Path to Disruptive Intelligent Commerce

**Status**: Phase 3 Complete (Autonomous Agents Active)
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
    *   [x] **Services**: Implemented `ReturnService` and `FraudService` to manage agent work queues.

## 🛠️ Usage

**Environment Variables Required:**
```bash
export OPENAI_API_KEY="sk-..."
export QDRANT_URL="http://localhost:6334"
export INVENTORY_AGENT_INTERVAL_SECONDS=300
export RETURN_AGENT_INTERVAL_SECONDS=60
export FRAUD_AGENT_INTERVAL_SECONDS=30
```

**Endpoints:**

1.  **Semantic Search**: `POST /neural/search`
2.  **Chat with Inventory**: `POST /neural/chat`
3.  **Create Return**: `POST /returns`
4.  **Check Return Status**: `GET /returns/pending`

**Autonomous Behavior:**
*   **FraudAgent** wakes up every 30s, checks new orders, asks LLM to assess risk (0-100 score), and approves/rejects.
*   **ReturnAgent** wakes up every 60s, analyzes returns for quality issues.
*   **InventoryAgent** wakes up every 5 mins, checks stock velocity.