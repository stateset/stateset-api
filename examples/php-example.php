<?php
/**
 * StateSet API - PHP Client Example
 *
 * A comprehensive PHP client for the StateSet API demonstrating:
 * - Authentication (JWT & API Keys)
 * - Order management
 * - Inventory operations
 * - Shopping cart & checkout
 * - Returns & shipments
 * - Customer management
 * - Analytics
 *
 * Installation:
 * composer require guzzlehttp/guzzle
 *
 * Usage:
 * php php-example.php
 */

require_once __DIR__ . '/vendor/autoload.php';

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

/**
 * StateSet API Client
 */
class StateSetClient
{
    private $client;
    private $baseUrl;
    private $accessToken;
    private $refreshToken;

    /**
     * Constructor
     *
     * @param string $baseUrl API base URL
     */
    public function __construct($baseUrl = 'http://localhost:8080/api/v1')
    {
        $this->baseUrl = $baseUrl;
        $this->client = new Client([
            'base_uri' => $baseUrl,
            'timeout' => 30.0,
            'headers' => [
                'Content-Type' => 'application/json',
                'Accept' => 'application/json',
            ],
        ]);
    }

    /**
     * Make HTTP request
     *
     * @param string $method HTTP method
     * @param string $path API path
     * @param array|null $data Request body
     * @param array $headers Additional headers
     * @return array Response data
     * @throws Exception
     */
    private function request($method, $path, $data = null, $headers = [])
    {
        try {
            $options = [
                'headers' => array_merge(
                    $this->accessToken ? ['Authorization' => "Bearer {$this->accessToken}"] : [],
                    $headers
                ),
            ];

            if ($data !== null) {
                $options['json'] = $data;
            }

            $response = $this->client->request($method, $path, $options);
            $body = (string) $response->getBody();

            return json_decode($body, true) ?: [];
        } catch (RequestException $e) {
            $response = $e->getResponse();
            if ($response) {
                $error = json_decode((string) $response->getBody(), true);
                if (isset($error['error'])) {
                    throw new Exception(
                        "API Error: {$error['error']['message']} " .
                        "(code: {$error['error']['code']}, status: {$error['error']['status']})"
                    );
                }
            }
            throw new Exception("Request failed: " . $e->getMessage());
        }
    }

    // ========================================================================
    // Authentication
    // ========================================================================

    /**
     * Login with email and password
     *
     * @param string $email User email
     * @param string $password User password
     * @return array Auth response
     */
    public function login($email, $password)
    {
        $response = $this->request('POST', '/auth/login', [
            'email' => $email,
            'password' => $password,
        ]);

        $this->accessToken = $response['access_token'];
        $this->refreshToken = $response['refresh_token'];

        return $response;
    }

    /**
     * Register a new user
     *
     * @param string $email User email
     * @param string $password User password
     * @param string $firstName First name
     * @param string $lastName Last name
     * @return array Auth response
     */
    public function register($email, $password, $firstName, $lastName)
    {
        $response = $this->request('POST', '/auth/register', [
            'email' => $email,
            'password' => $password,
            'first_name' => $firstName,
            'last_name' => $lastName,
        ]);

        $this->accessToken = $response['access_token'];
        $this->refreshToken = $response['refresh_token'];

        return $response;
    }

    /**
     * Create API key
     *
     * @param string $name Key name
     * @param array $permissions Permissions array
     * @return array API key data
     */
    public function createApiKey($name, $permissions)
    {
        return $this->request('POST', '/auth/api-keys', [
            'name' => $name,
            'permissions' => $permissions,
            'expires_at' => date('c', strtotime('+1 year')),
        ]);
    }

    /**
     * Logout
     *
     * @return array Response
     */
    public function logout()
    {
        $response = $this->request('POST', '/auth/logout');
        $this->accessToken = null;
        $this->refreshToken = null;
        return $response;
    }

    // ========================================================================
    // Orders
    // ========================================================================

    /**
     * Create order
     *
     * @param array $data Order data
     * @return array Order
     */
    public function createOrder($data)
    {
        return $this->request('POST', '/orders', $data);
    }

    /**
     * List orders
     *
     * @param array $params Query parameters
     * @return array Paginated response
     */
    public function listOrders($params = [])
    {
        $query = http_build_query($params);
        return $this->request('GET', "/orders?{$query}");
    }

    /**
     * Get order by ID
     *
     * @param string $orderId Order ID
     * @return array Order
     */
    public function getOrder($orderId)
    {
        return $this->request('GET', "/orders/{$orderId}");
    }

    /**
     * Update order status
     *
     * @param string $orderId Order ID
     * @param string $status New status
     * @param string|null $notes Optional notes
     * @return array Order
     */
    public function updateOrderStatus($orderId, $status, $notes = null)
    {
        $data = ['status' => $status];
        if ($notes !== null) {
            $data['notes'] = $notes;
        }
        return $this->request('PUT', "/orders/{$orderId}/status", $data);
    }

    /**
     * Cancel order
     *
     * @param string $orderId Order ID
     * @param string|null $reason Cancellation reason
     * @return array Order
     */
    public function cancelOrder($orderId, $reason = null)
    {
        $data = [];
        if ($reason !== null) {
            $data['reason'] = $reason;
        }
        return $this->request('POST', "/orders/{$orderId}/cancel", $data);
    }

    // ========================================================================
    // Inventory
    // ========================================================================

    /**
     * List inventory
     *
     * @param array $params Query parameters
     * @return array Paginated response
     */
    public function listInventory($params = [])
    {
        $query = http_build_query($params);
        return $this->request('GET', "/inventory?{$query}");
    }

    /**
     * Get low stock items
     *
     * @return array Inventory items
     */
    public function getLowStockItems()
    {
        return $this->request('GET', '/inventory/low-stock');
    }

    /**
     * Reserve inventory
     *
     * @param string $inventoryId Inventory item ID
     * @param int $quantity Quantity to reserve
     * @param string $orderId Order ID
     * @return array Response
     */
    public function reserveInventory($inventoryId, $quantity, $orderId)
    {
        return $this->request('POST', "/inventory/{$inventoryId}/reserve", [
            'quantity' => $quantity,
            'order_id' => $orderId,
            'expires_at' => date('c', strtotime('+24 hours')),
        ]);
    }

    // ========================================================================
    // Shopping Cart
    // ========================================================================

    /**
     * Create cart
     *
     * @param string|null $customerId Customer ID
     * @return array Cart
     */
    public function createCart($customerId = null)
    {
        $data = ['session_id' => $this->generateUuid()];
        if ($customerId !== null) {
            $data['customer_id'] = $customerId;
        }
        return $this->request('POST', '/carts', $data);
    }

    /**
     * Get cart
     *
     * @param string $cartId Cart ID
     * @return array Cart
     */
    public function getCart($cartId)
    {
        return $this->request('GET', "/carts/{$cartId}");
    }

    /**
     * Add item to cart
     *
     * @param string $cartId Cart ID
     * @param array $item Item data
     * @return array Cart
     */
    public function addItemToCart($cartId, $item)
    {
        return $this->request('POST', "/carts/{$cartId}/items", $item);
    }

    /**
     * Update cart item
     *
     * @param string $cartId Cart ID
     * @param string $itemId Item ID
     * @param int $quantity New quantity
     * @return array Cart
     */
    public function updateCartItem($cartId, $itemId, $quantity)
    {
        return $this->request('PUT', "/carts/{$cartId}/items/{$itemId}", [
            'quantity' => $quantity,
        ]);
    }

    /**
     * Remove cart item
     *
     * @param string $cartId Cart ID
     * @param string $itemId Item ID
     * @return array Cart
     */
    public function removeCartItem($cartId, $itemId)
    {
        return $this->request('DELETE', "/carts/{$cartId}/items/{$itemId}");
    }

    // ========================================================================
    // Customers
    // ========================================================================

    /**
     * Create customer
     *
     * @param array $data Customer data
     * @return array Customer
     */
    public function createCustomer($data)
    {
        return $this->request('POST', '/customers', $data);
    }

    /**
     * List customers
     *
     * @param array $params Query parameters
     * @return array Paginated response
     */
    public function listCustomers($params = [])
    {
        $query = http_build_query($params);
        return $this->request('GET', "/customers?{$query}");
    }

    /**
     * Get customer
     *
     * @param string $customerId Customer ID
     * @return array Customer
     */
    public function getCustomer($customerId)
    {
        return $this->request('GET', "/customers/{$customerId}");
    }

    // ========================================================================
    // Returns
    // ========================================================================

    /**
     * Create return
     *
     * @param array $data Return data
     * @return array Return
     */
    public function createReturn($data)
    {
        return $this->request('POST', '/returns', $data);
    }

    /**
     * List returns
     *
     * @param array $params Query parameters
     * @return array Paginated response
     */
    public function listReturns($params = [])
    {
        $query = http_build_query($params);
        return $this->request('GET', "/returns?{$query}");
    }

    /**
     * Approve return
     *
     * @param string $returnId Return ID
     * @return array Return
     */
    public function approveReturn($returnId)
    {
        return $this->request('POST', "/returns/{$returnId}/approve");
    }

    // ========================================================================
    // Shipments
    // ========================================================================

    /**
     * Create shipment
     *
     * @param array $data Shipment data
     * @return array Shipment
     */
    public function createShipment($data)
    {
        return $this->request('POST', '/shipments', $data);
    }

    /**
     * Mark as shipped
     *
     * @param string $shipmentId Shipment ID
     * @param string $trackingNumber Tracking number
     * @return array Shipment
     */
    public function markAsShipped($shipmentId, $trackingNumber)
    {
        return $this->request('POST', "/shipments/{$shipmentId}/ship", [
            'tracking_number' => $trackingNumber,
            'shipped_at' => date('c'),
        ]);
    }

    /**
     * Track shipment
     *
     * @param string $trackingNumber Tracking number
     * @return array Tracking info
     */
    public function trackShipment($trackingNumber)
    {
        return $this->request('GET', "/shipments/track/{$trackingNumber}");
    }

    // ========================================================================
    // Analytics
    // ========================================================================

    /**
     * Get dashboard metrics
     *
     * @return array Metrics
     */
    public function getDashboardMetrics()
    {
        return $this->request('GET', '/analytics/dashboard');
    }

    /**
     * Get sales trends
     *
     * @param array $params Query parameters
     * @return array Sales trends
     */
    public function getSalesTrends($params = [])
    {
        $query = http_build_query($params);
        return $this->request('GET', "/analytics/sales/trends?{$query}");
    }

    // ========================================================================
    // Health
    // ========================================================================

    /**
     * Get health status
     *
     * @return array Health status
     */
    public function getHealth()
    {
        return $this->request('GET', '/health');
    }

    // ========================================================================
    // Utilities
    // ========================================================================

    /**
     * Generate UUID v4
     *
     * @return string UUID
     */
    private function generateUuid()
    {
        return sprintf(
            '%04x%04x-%04x-%04x-%04x-%04x%04x%04x',
            mt_rand(0, 0xffff),
            mt_rand(0, 0xffff),
            mt_rand(0, 0xffff),
            mt_rand(0, 0x0fff) | 0x4000,
            mt_rand(0, 0x3fff) | 0x8000,
            mt_rand(0, 0xffff),
            mt_rand(0, 0xffff),
            mt_rand(0, 0xffff)
        );
    }
}

// ============================================================================
// Example Usage
// ============================================================================

function main()
{
    $client = new StateSetClient('http://localhost:8080/api/v1');

    echo "🚀 StateSet API PHP Example\n\n";

    try {
        // 1. Authentication
        echo "1️⃣  Authenticating...\n";
        $client->login('admin@stateset.com', 'your-password');
        echo "✅ Logged in successfully\n\n";

        // 2. Check API health
        echo "2️⃣  Checking API health...\n";
        $health = $client->getHealth();
        echo "✅ API Status: {$health['status']}\n\n";

        // 3. Create a customer
        echo "3️⃣  Creating customer...\n";
        $customer = $client->createCustomer([
            'email' => 'test-' . time() . '@example.com',
            'first_name' => 'John',
            'last_name' => 'Doe',
            'phone' => '+1-555-0100',
        ]);
        echo "✅ Customer created: {$customer['id']}\n\n";

        // 4. Create a shopping cart
        echo "4️⃣  Creating shopping cart...\n";
        $cart = $client->createCart($customer['id']);
        echo "✅ Cart created: {$cart['id']}\n\n";

        // 5. Add items to cart
        echo "5️⃣  Adding items to cart...\n";
        $client->addItemToCart($cart['id'], [
            'product_id' => (new StateSetClient())->generateUuid(),
            'sku' => 'WIDGET-001',
            'quantity' => 2,
            'price' => 99.99,
            'name' => 'Premium Widget',
        ]);
        echo "✅ Items added to cart\n\n";

        // 6. Get cart details
        $updatedCart = $client->getCart($cart['id']);
        echo "📦 Cart total: \${$updatedCart['total']}\n\n";

        // 7. List orders
        echo "7️⃣  Listing orders...\n";
        $orders = $client->listOrders(['page' => 1, 'limit' => 5]);
        $count = isset($orders['data']) ? count($orders['data']) : 0;
        echo "✅ Found {$count} orders\n\n";

        // 8. Check low stock items
        echo "8️⃣  Checking low stock items...\n";
        $lowStock = $client->getLowStockItems();
        $count = isset($lowStock['data']) ? count($lowStock['data']) : 0;
        echo "📊 Low stock items: {$count}\n\n";

        // 9. Get dashboard metrics
        echo "9️⃣  Fetching analytics...\n";
        $dashboard = $client->getDashboardMetrics();
        echo "📈 Dashboard metrics retrieved\n\n";

        // 10. Create API key
        echo "🔟 Creating API key...\n";
        $apiKey = $client->createApiKey('Test API Key', [
            'orders:read',
            'inventory:read',
        ]);
        $keyPreview = substr($apiKey['key'], 0, 20) . '...';
        echo "✅ API Key created: {$keyPreview}\n\n";

        echo "✨ All examples completed successfully!\n";
    } catch (Exception $e) {
        echo "❌ Error: {$e->getMessage()}\n";
        exit(1);
    }
}

// Run the example
if (php_sapi_name() === 'cli') {
    main();
}
