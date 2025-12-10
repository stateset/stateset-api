package main

/**
 * StateSet API - Go Client Example
 *
 * A comprehensive Go client for the StateSet API demonstrating:
 * - Authentication (JWT & API Keys)
 * - Order management
 * - Inventory operations
 * - Shopping cart & checkout
 * - Returns & shipments
 * - Customer management
 * - Analytics
 *
 * Installation:
 * go get github.com/google/uuid
 *
 * Usage:
 * go run go-example.go
 */

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/google/uuid"
)

// ============================================================================
// Type Definitions
// ============================================================================

type AuthResponse struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	TokenType    string `json:"token_type"`
	ExpiresIn    int    `json:"expires_in"`
}

type Order struct {
	ID           string      `json:"id"`
	CustomerID   string      `json:"customer_id"`
	Status       string      `json:"status"`
	TotalAmount  float64     `json:"total_amount"`
	Currency     string      `json:"currency"`
	Items        []OrderItem `json:"items"`
	CreatedAt    string      `json:"created_at"`
	UpdatedAt    string      `json:"updated_at"`
}

type OrderItem struct {
	ProductID string  `json:"product_id"`
	SKU       string  `json:"sku"`
	Quantity  int     `json:"quantity"`
	UnitPrice float64 `json:"unit_price"`
	Name      string  `json:"name"`
}

type InventoryItem struct {
	ID                string  `json:"id"`
	SKU               string  `json:"sku"`
	LocationID        string  `json:"location_id"`
	QuantityOnHand    int     `json:"quantity_on_hand"`
	QuantityAllocated int     `json:"quantity_allocated"`
	QuantityAvailable int     `json:"quantity_available"`
}

type Cart struct {
	ID         string     `json:"id"`
	CustomerID *string    `json:"customer_id,omitempty"`
	SessionID  *string    `json:"session_id,omitempty"`
	Items      []CartItem `json:"items"`
	Subtotal   float64    `json:"subtotal"`
	Tax        float64    `json:"tax"`
	Shipping   float64    `json:"shipping"`
	Total      float64    `json:"total"`
}

type CartItem struct {
	ProductID string  `json:"product_id"`
	SKU       string  `json:"sku"`
	Quantity  int     `json:"quantity"`
	Price     float64 `json:"price"`
	Name      string  `json:"name"`
}

type Customer struct {
	ID        string  `json:"id"`
	Email     string  `json:"email"`
	FirstName string  `json:"first_name"`
	LastName  string  `json:"last_name"`
	Phone     *string `json:"phone,omitempty"`
	CreatedAt string  `json:"created_at"`
}

type Shipment struct {
	ID             string  `json:"id"`
	OrderID        string  `json:"order_id"`
	Carrier        string  `json:"carrier"`
	TrackingNumber *string `json:"tracking_number,omitempty"`
	Status         string  `json:"status"`
	ShippedAt      *string `json:"shipped_at,omitempty"`
	DeliveredAt    *string `json:"delivered_at,omitempty"`
}

type Return struct {
	ID            string       `json:"id"`
	OrderID       string       `json:"order_id"`
	Status        string       `json:"status"`
	Items         []ReturnItem `json:"items"`
	CustomerNotes *string      `json:"customer_notes,omitempty"`
}

type ReturnItem struct {
	OrderItemID string  `json:"order_item_id"`
	Quantity    int     `json:"quantity"`
	Reason      string  `json:"reason"`
	Description *string `json:"description,omitempty"`
}

type PaginatedResponse struct {
	Data       interface{} `json:"data"`
	Pagination struct {
		Page       int `json:"page"`
		PerPage    int `json:"per_page"`
		Total      int `json:"total"`
		TotalPages int `json:"total_pages"`
	} `json:"pagination"`
}

type APIError struct {
	Error struct {
		Code    string                 `json:"code"`
		Message string                 `json:"message"`
		Status  int                    `json:"status"`
		Details map[string]interface{} `json:"details,omitempty"`
	} `json:"error"`
}

// ============================================================================
// StateSet API Client
// ============================================================================

type StateSetClient struct {
	BaseURL      string
	HTTPClient   *http.Client
	AccessToken  string
	RefreshToken string
}

// NewStateSetClient creates a new API client
func NewStateSetClient(baseURL string) *StateSetClient {
	if baseURL == "" {
		baseURL = "http://localhost:8080/api/v1"
	}

	return &StateSetClient{
		BaseURL: baseURL,
		HTTPClient: &http.Client{
			Timeout: 30 * time.Second,
		},
	}
}

// doRequest performs an HTTP request with authentication
func (c *StateSetClient) doRequest(method, path string, body interface{}) (*http.Response, error) {
	var reqBody io.Reader
	if body != nil {
		jsonData, err := json.Marshal(body)
		if err != nil {
			return nil, fmt.Errorf("failed to marshal request body: %w", err)
		}
		reqBody = bytes.NewBuffer(jsonData)
	}

	req, err := http.NewRequest(method, c.BaseURL+path, reqBody)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	if c.AccessToken != "" {
		req.Header.Set("Authorization", "Bearer "+c.AccessToken)
	}

	resp, err := c.HTTPClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}

	return resp, nil
}

// parseResponse parses the response body into the target struct
func parseResponse(resp *http.Response, target interface{}) error {
	defer resp.Body.Close()

	if resp.StatusCode >= 400 {
		var apiErr APIError
		if err := json.NewDecoder(resp.Body).Decode(&apiErr); err != nil {
			return fmt.Errorf("request failed with status %d", resp.StatusCode)
		}
		return fmt.Errorf("API error: %s (code: %s, status: %d)",
			apiErr.Error.Message, apiErr.Error.Code, apiErr.Error.Status)
	}

	if target != nil {
		if err := json.NewDecoder(resp.Body).Decode(target); err != nil {
			return fmt.Errorf("failed to decode response: %w", err)
		}
	}

	return nil
}

// ==========================================================================
// Authentication
// ==========================================================================

func (c *StateSetClient) Login(email, password string) error {
	body := map[string]string{
		"email":    email,
		"password": password,
	}

	resp, err := c.doRequest("POST", "/auth/login", body)
	if err != nil {
		return err
	}

	var authResp AuthResponse
	if err := parseResponse(resp, &authResp); err != nil {
		return err
	}

	c.AccessToken = authResp.AccessToken
	c.RefreshToken = authResp.RefreshToken

	return nil
}

func (c *StateSetClient) Register(email, password, firstName, lastName string) error {
	body := map[string]string{
		"email":      email,
		"password":   password,
		"first_name": firstName,
		"last_name":  lastName,
	}

	resp, err := c.doRequest("POST", "/auth/register", body)
	if err != nil {
		return err
	}

	var authResp AuthResponse
	if err := parseResponse(resp, &authResp); err != nil {
		return err
	}

	c.AccessToken = authResp.AccessToken
	c.RefreshToken = authResp.RefreshToken

	return nil
}

func (c *StateSetClient) CreateAPIKey(name string, permissions []string) (map[string]string, error) {
	expiresAt := time.Now().AddDate(1, 0, 0).Format(time.RFC3339)

	body := map[string]interface{}{
		"name":        name,
		"permissions": permissions,
		"expires_at":  expiresAt,
	}

	resp, err := c.doRequest("POST", "/auth/api-keys", body)
	if err != nil {
		return nil, err
	}

	var result map[string]string
	if err := parseResponse(resp, &result); err != nil {
		return nil, err
	}

	return result, nil
}

func (c *StateSetClient) Logout() error {
	resp, err := c.doRequest("POST", "/auth/logout", nil)
	if err != nil {
		return err
	}

	if err := parseResponse(resp, nil); err != nil {
		return err
	}

	c.AccessToken = ""
	c.RefreshToken = ""

	return nil
}

// ==========================================================================
// Orders
// ==========================================================================

func (c *StateSetClient) CreateOrder(customerID string, items []OrderItem) (*Order, error) {
	body := map[string]interface{}{
		"customer_id": customerID,
		"items":       items,
	}

	resp, err := c.doRequest("POST", "/orders", body)
	if err != nil {
		return nil, err
	}

	var order Order
	if err := parseResponse(resp, &order); err != nil {
		return nil, err
	}

	return &order, nil
}

func (c *StateSetClient) ListOrders(page, limit int) (*PaginatedResponse, error) {
	path := fmt.Sprintf("/orders?page=%d&limit=%d", page, limit)

	resp, err := c.doRequest("GET", path, nil)
	if err != nil {
		return nil, err
	}

	var result PaginatedResponse
	if err := parseResponse(resp, &result); err != nil {
		return nil, err
	}

	return &result, nil
}

func (c *StateSetClient) GetOrder(orderID string) (*Order, error) {
	resp, err := c.doRequest("GET", "/orders/"+orderID, nil)
	if err != nil {
		return nil, err
	}

	var order Order
	if err := parseResponse(resp, &order); err != nil {
		return nil, err
	}

	return &order, nil
}

func (c *StateSetClient) UpdateOrderStatus(orderID, status, notes string) (*Order, error) {
	body := map[string]string{
		"status": status,
		"notes":  notes,
	}

	resp, err := c.doRequest("PUT", "/orders/"+orderID+"/status", body)
	if err != nil {
		return nil, err
	}

	var order Order
	if err := parseResponse(resp, &order); err != nil {
		return nil, err
	}

	return &order, nil
}

func (c *StateSetClient) CancelOrder(orderID, reason string) (*Order, error) {
	body := map[string]string{
		"reason": reason,
	}

	resp, err := c.doRequest("POST", "/orders/"+orderID+"/cancel", body)
	if err != nil {
		return nil, err
	}

	var order Order
	if err := parseResponse(resp, &order); err != nil {
		return nil, err
	}

	return &order, nil
}

// ==========================================================================
// Inventory
// ==========================================================================

func (c *StateSetClient) ListInventory(page, limit int) (*PaginatedResponse, error) {
	path := fmt.Sprintf("/inventory?page=%d&limit=%d", page, limit)

	resp, err := c.doRequest("GET", path, nil)
	if err != nil {
		return nil, err
	}

	var result PaginatedResponse
	if err := parseResponse(resp, &result); err != nil {
		return nil, err
	}

	return &result, nil
}

func (c *StateSetClient) GetLowStockItems() ([]InventoryItem, error) {
	resp, err := c.doRequest("GET", "/inventory/low-stock", nil)
	if err != nil {
		return nil, err
	}

	var result struct {
		Data []InventoryItem `json:"data"`
	}
	if err := parseResponse(resp, &result); err != nil {
		return nil, err
	}

	return result.Data, nil
}

func (c *StateSetClient) ReserveInventory(inventoryID string, quantity int, orderID string) error {
	expiresAt := time.Now().Add(24 * time.Hour).Format(time.RFC3339)

	body := map[string]interface{}{
		"quantity":   quantity,
		"order_id":   orderID,
		"expires_at": expiresAt,
	}

	resp, err := c.doRequest("POST", "/inventory/"+inventoryID+"/reserve", body)
	if err != nil {
		return err
	}

	return parseResponse(resp, nil)
}

// ==========================================================================
// Shopping Cart
// ==========================================================================

func (c *StateSetClient) CreateCart(customerID string) (*Cart, error) {
	sessionID := uuid.New().String()

	body := map[string]string{
		"session_id": sessionID,
	}
	if customerID != "" {
		body["customer_id"] = customerID
	}

	resp, err := c.doRequest("POST", "/carts", body)
	if err != nil {
		return nil, err
	}

	var cart Cart
	if err := parseResponse(resp, &cart); err != nil {
		return nil, err
	}

	return &cart, nil
}

func (c *StateSetClient) GetCart(cartID string) (*Cart, error) {
	resp, err := c.doRequest("GET", "/carts/"+cartID, nil)
	if err != nil {
		return nil, err
	}

	var cart Cart
	if err := parseResponse(resp, &cart); err != nil {
		return nil, err
	}

	return &cart, nil
}

func (c *StateSetClient) AddItemToCart(cartID string, item CartItem) (*Cart, error) {
	resp, err := c.doRequest("POST", "/carts/"+cartID+"/items", item)
	if err != nil {
		return nil, err
	}

	var cart Cart
	if err := parseResponse(resp, &cart); err != nil {
		return nil, err
	}

	return &cart, nil
}

// ==========================================================================
// Customers
// ==========================================================================

func (c *StateSetClient) CreateCustomer(email, firstName, lastName, phone string) (*Customer, error) {
	body := map[string]string{
		"email":      email,
		"first_name": firstName,
		"last_name":  lastName,
	}
	if phone != "" {
		body["phone"] = phone
	}

	resp, err := c.doRequest("POST", "/customers", body)
	if err != nil {
		return nil, err
	}

	var customer Customer
	if err := parseResponse(resp, &customer); err != nil {
		return nil, err
	}

	return &customer, nil
}

func (c *StateSetClient) ListCustomers(page, limit int) (*PaginatedResponse, error) {
	path := fmt.Sprintf("/customers?page=%d&limit=%d", page, limit)

	resp, err := c.doRequest("GET", path, nil)
	if err != nil {
		return nil, err
	}

	var result PaginatedResponse
	if err := parseResponse(resp, &result); err != nil {
		return nil, err
	}

	return &result, nil
}

// ==========================================================================
// Returns
// ==========================================================================

func (c *StateSetClient) CreateReturn(orderID string, items []ReturnItem, notes string) (*Return, error) {
	body := map[string]interface{}{
		"order_id": orderID,
		"items":    items,
	}
	if notes != "" {
		body["customer_notes"] = notes
	}

	resp, err := c.doRequest("POST", "/returns", body)
	if err != nil {
		return nil, err
	}

	var returnObj Return
	if err := parseResponse(resp, &returnObj); err != nil {
		return nil, err
	}

	return &returnObj, nil
}

func (c *StateSetClient) ApproveReturn(returnID string) (*Return, error) {
	resp, err := c.doRequest("POST", "/returns/"+returnID+"/approve", nil)
	if err != nil {
		return nil, err
	}

	var returnObj Return
	if err := parseResponse(resp, &returnObj); err != nil {
		return nil, err
	}

	return &returnObj, nil
}

// ==========================================================================
// Shipments
// ==========================================================================

func (c *StateSetClient) CreateShipment(orderID, carrier, serviceLevel string) (*Shipment, error) {
	body := map[string]string{
		"order_id":      orderID,
		"carrier":       carrier,
		"service_level": serviceLevel,
	}

	resp, err := c.doRequest("POST", "/shipments", body)
	if err != nil {
		return nil, err
	}

	var shipment Shipment
	if err := parseResponse(resp, &shipment); err != nil {
		return nil, err
	}

	return &shipment, nil
}

func (c *StateSetClient) MarkAsShipped(shipmentID, trackingNumber string) (*Shipment, error) {
	body := map[string]string{
		"tracking_number": trackingNumber,
		"shipped_at":      time.Now().Format(time.RFC3339),
	}

	resp, err := c.doRequest("POST", "/shipments/"+shipmentID+"/ship", body)
	if err != nil {
		return nil, err
	}

	var shipment Shipment
	if err := parseResponse(resp, &shipment); err != nil {
		return nil, err
	}

	return &shipment, nil
}

func (c *StateSetClient) TrackShipment(trackingNumber string) (map[string]interface{}, error) {
	resp, err := c.doRequest("GET", "/shipments/track/"+trackingNumber, nil)
	if err != nil {
		return nil, err
	}

	var result map[string]interface{}
	if err := parseResponse(resp, &result); err != nil {
		return nil, err
	}

	return result, nil
}

// ==========================================================================
// Analytics
// ==========================================================================

func (c *StateSetClient) GetDashboardMetrics() (map[string]interface{}, error) {
	resp, err := c.doRequest("GET", "/analytics/dashboard", nil)
	if err != nil {
		return nil, err
	}

	var result map[string]interface{}
	if err := parseResponse(resp, &result); err != nil {
		return nil, err
	}

	return result, nil
}

func (c *StateSetClient) GetHealth() (map[string]interface{}, error) {
	resp, err := c.doRequest("GET", "/health", nil)
	if err != nil {
		return nil, err
	}

	var result map[string]interface{}
	if err := parseResponse(resp, &result); err != nil {
		return nil, err
	}

	return result, nil
}

// ============================================================================
// Example Usage
// ============================================================================

func main() {
	client := NewStateSetClient("http://localhost:8080/api/v1")

	fmt.Println("🚀 StateSet API Go Example\n")

	// 1. Authentication
	fmt.Println("1️⃣  Authenticating...")
	if err := client.Login("admin@stateset.com", "your-password"); err != nil {
		fmt.Printf("❌ Login failed: %v\n", err)
		return
	}
	fmt.Println("✅ Logged in successfully\n")

	// 2. Check API health
	fmt.Println("2️⃣  Checking API health...")
	health, err := client.GetHealth()
	if err != nil {
		fmt.Printf("❌ Health check failed: %v\n", err)
	} else {
		fmt.Printf("✅ API Status: %v\n\n", health["status"])
	}

	// 3. Create a customer
	fmt.Println("3️⃣  Creating customer...")
	customer, err := client.CreateCustomer(
		fmt.Sprintf("test-%d@example.com", time.Now().Unix()),
		"John",
		"Doe",
		"+1-555-0100",
	)
	if err != nil {
		fmt.Printf("❌ Customer creation failed: %v\n", err)
	} else {
		fmt.Printf("✅ Customer created: %s\n\n", customer.ID)
	}

	// 4. Create a shopping cart
	fmt.Println("4️⃣  Creating shopping cart...")
	cart, err := client.CreateCart(customer.ID)
	if err != nil {
		fmt.Printf("❌ Cart creation failed: %v\n", err)
	} else {
		fmt.Printf("✅ Cart created: %s\n\n", cart.ID)

		// 5. Add items to cart
		fmt.Println("5️⃣  Adding items to cart...")
		_, err = client.AddItemToCart(cart.ID, CartItem{
			ProductID: uuid.New().String(),
			SKU:       "WIDGET-001",
			Quantity:  2,
			Price:     99.99,
			Name:      "Premium Widget",
		})
		if err != nil {
			fmt.Printf("❌ Failed to add item: %v\n", err)
		} else {
			fmt.Println("✅ Items added to cart\n")
		}
	}

	// 6. List orders
	fmt.Println("6️⃣  Listing orders...")
	orders, err := client.ListOrders(1, 5)
	if err != nil {
		fmt.Printf("❌ Failed to list orders: %v\n", err)
	} else {
		fmt.Printf("✅ Orders retrieved\n\n")
	}

	// 7. Check low stock items
	fmt.Println("7️⃣  Checking low stock items...")
	lowStock, err := client.GetLowStockItems()
	if err != nil {
		fmt.Printf("❌ Failed to get low stock: %v\n", err)
	} else {
		fmt.Printf("📊 Low stock items: %d\n\n", len(lowStock))
	}

	// 8. Get dashboard metrics
	fmt.Println("8️⃣  Fetching analytics...")
	dashboard, err := client.GetDashboardMetrics()
	if err != nil {
		fmt.Printf("❌ Failed to get dashboard: %v\n", err)
	} else {
		fmt.Println("📈 Dashboard metrics retrieved\n")
		_ = dashboard
	}

	// 9. Create API key
	fmt.Println("9️⃣  Creating API key...")
	apiKey, err := client.CreateAPIKey("Test API Key", []string{"orders:read", "inventory:read"})
	if err != nil {
		fmt.Printf("❌ Failed to create API key: %v\n", err)
	} else {
		key := apiKey["key"]
		if len(key) > 20 {
			key = key[:20] + "..."
		}
		fmt.Printf("✅ API Key created: %s\n\n", key)
	}

	fmt.Println("✨ All examples completed successfully!")
}
