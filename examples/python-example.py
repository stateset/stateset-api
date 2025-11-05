"""
StateSet API - Python Example

This example demonstrates common workflows using the StateSet API:
- Authentication
- Creating orders
- Managing inventory
- Processing returns
- Tracking shipments

Install dependencies: pip install requests
"""

import requests
import json
from datetime import datetime, timedelta
from typing import Dict, List, Optional
import uuid


class StateSetClient:
    """Python client for the StateSet API"""

    def __init__(self, base_url: str):
        self.base_url = base_url.rstrip('/')
        self.access_token = None
        self.refresh_token = None
        self.session = requests.Session()

    def _request(self, method: str, endpoint: str, data: Optional[Dict] = None,
                 headers: Optional[Dict] = None) -> Dict:
        """Make an authenticated request to the API"""
        url = f"{self.base_url}{endpoint}"

        request_headers = {
            'Content-Type': 'application/json'
        }

        if self.access_token:
            request_headers['Authorization'] = f'Bearer {self.access_token}'

        if headers:
            request_headers.update(headers)

        try:
            response = self.session.request(
                method=method,
                url=url,
                json=data,
                headers=request_headers
            )
            response.raise_for_status()
            return response.json()
        except requests.exceptions.HTTPError as e:
            error_data = e.response.json() if e.response.content else {}
            print(f"API Error: {error_data.get('message', str(e))}")
            raise
        except requests.exceptions.RequestException as e:
            print(f"Request Error: {str(e)}")
            raise

    # Authentication
    def login(self, email: str, password: str) -> Dict:
        """Login and obtain access token"""
        response = self._request('POST', '/auth/login', {
            'email': email,
            'password': password
        })

        self.access_token = response['data']['access_token']
        self.refresh_token = response['data']['refresh_token']

        print('✓ Logged in successfully')
        return response['data']

    def create_api_key(self, name: str, permissions: List[str]) -> Dict:
        """Create an API key for service-to-service auth"""
        expires_at = (datetime.now() + timedelta(days=365)).isoformat()

        response = self._request('POST', '/auth/api-keys', {
            'name': name,
            'permissions': permissions,
            'expires_at': expires_at
        })

        print(f"✓ API Key created: {response['data']['key']}")
        return response['data']

    # Orders
    def create_order(self, order_data: Dict) -> Dict:
        """Create a new order"""
        response = self._request('POST', '/orders', order_data)
        order_number = response['data'].get('order_number', response['data']['id'])
        print(f"✓ Order created: {order_number}")
        return response['data']

    def list_orders(self, **filters) -> Dict:
        """List orders with optional filters"""
        params = {
            'page': filters.get('page', 1),
            'limit': filters.get('limit', 20)
        }

        if 'status' in filters:
            params['status'] = filters['status']
        if 'customer_id' in filters:
            params['customer_id'] = filters['customer_id']

        query_string = '&'.join([f"{k}={v}" for k, v in params.items()])
        response = self._request('GET', f'/orders?{query_string}')

        total = response['data'].get('total', len(response['data']))
        print(f"✓ Found {total} orders")
        return response['data']

    def get_order(self, order_id: str) -> Dict:
        """Get order details by ID"""
        response = self._request('GET', f'/orders/{order_id}')
        return response['data']

    def update_order_status(self, order_id: str, status: str, notes: str = '') -> Dict:
        """Update order status"""
        response = self._request('PUT', f'/orders/{order_id}/status', {
            'status': status,
            'notes': notes
        })
        print(f"✓ Order status updated to: {status}")
        return response['data']

    def cancel_order(self, order_id: str, reason: str) -> Dict:
        """Cancel an order"""
        response = self._request('POST', f'/orders/{order_id}/cancel', {
            'reason': reason,
            'refund': True
        })
        print('✓ Order cancelled')
        return response['data']

    # Inventory
    def list_inventory(self, **filters) -> Dict:
        """List inventory items"""
        params = {
            'page': filters.get('page', 1),
            'limit': filters.get('limit', 20)
        }

        if 'product_id' in filters:
            params['product_id'] = filters['product_id']
        if 'location_id' in filters:
            params['location_id'] = filters['location_id']

        query_string = '&'.join([f"{k}={v}" for k, v in params.items()])
        response = self._request('GET', f'/inventory?{query_string}')

        total = response['data'].get('total', len(response['data']))
        print(f"✓ Found {total} inventory items")
        return response['data']

    def get_low_stock(self) -> List[Dict]:
        """Get low stock items"""
        response = self._request('GET', '/inventory/low-stock')
        print(f"✓ Found {len(response['data'])} low stock items")
        return response['data']

    def reserve_inventory(self, inventory_id: str, quantity: int, order_id: str) -> Dict:
        """Reserve inventory for an order"""
        response = self._request('POST', f'/inventory/{inventory_id}/reserve', {
            'quantity': quantity,
            'order_id': order_id
        })
        print(f"✓ Reserved {quantity} units")
        return response['data']

    def release_inventory(self, inventory_id: str, quantity: int, reason: str) -> Dict:
        """Release reserved inventory"""
        response = self._request('POST', f'/inventory/{inventory_id}/release', {
            'quantity': quantity,
            'reason': reason
        })
        print(f"✓ Released {quantity} units")
        return response['data']

    # Returns
    def create_return(self, return_data: Dict) -> Dict:
        """Create a return request"""
        response = self._request('POST', '/returns', return_data)
        rma = response['data'].get('rma_number', response['data']['id'])
        print(f"✓ Return created: {rma}")
        return response['data']

    def approve_return(self, return_id: str, refund_amount: float, notes: str = '') -> Dict:
        """Approve a return"""
        response = self._request('POST', f'/returns/{return_id}/approve', {
            'refund_amount': refund_amount,
            'notes': notes
        })
        print('✓ Return approved')
        return response['data']

    def restock_return(self, return_id: str, location_id: str) -> Dict:
        """Restock returned items"""
        response = self._request('POST', f'/returns/{return_id}/restock', {
            'location_id': location_id,
            'condition': 'good'
        })
        print('✓ Items restocked')
        return response['data']

    # Shipments
    def create_shipment(self, shipment_data: Dict) -> Dict:
        """Create a shipment"""
        response = self._request('POST', '/shipments', shipment_data)
        print(f"✓ Shipment created: {response['data']['id']}")
        return response['data']

    def mark_shipped(self, shipment_id: str, tracking_number: str) -> Dict:
        """Mark shipment as shipped"""
        response = self._request('POST', f'/shipments/{shipment_id}/ship', {
            'tracking_number': tracking_number,
            'shipped_at': datetime.now().isoformat()
        })
        print(f"✓ Marked as shipped with tracking: {tracking_number}")
        return response['data']

    def track_shipment(self, tracking_number: str) -> Dict:
        """Track a shipment"""
        response = self._request('GET', f'/shipments/track/{tracking_number}')
        print(f"✓ Shipment status: {response['data']['status']}")
        return response['data']

    # Payments
    def process_payment(self, payment_data: Dict) -> Dict:
        """Process a payment"""
        idempotency_key = str(uuid.uuid4())
        response = self._request('POST', '/payments', payment_data, {
            'Idempotency-Key': idempotency_key
        })
        print(f"✓ Payment processed: {response['data']['transaction_id']}")
        return response['data']

    def refund_payment(self, payment_id: str, amount: float, reason: str) -> Dict:
        """Refund a payment"""
        response = self._request('POST', '/payments/refund', {
            'payment_id': payment_id,
            'amount': amount,
            'reason': reason
        })
        print(f"✓ Refund processed: ${amount}")
        return response['data']

    # Analytics
    def get_dashboard(self) -> Dict:
        """Get dashboard metrics"""
        response = self._request('GET', '/analytics/dashboard')
        print('✓ Dashboard metrics retrieved')
        return response['data']

    def get_sales_trends(self, start_date: str, end_date: str, interval: str = 'month') -> List[Dict]:
        """Get sales trends"""
        params = f"start_date={start_date}&end_date={end_date}&interval={interval}"
        response = self._request('GET', f'/analytics/sales/trends?{params}')
        print('✓ Sales trends retrieved')
        return response['data']


def main():
    """Example usage of the StateSet API"""

    # Configuration
    BASE_URL = 'http://localhost:8080/api/v1'
    EMAIL = 'admin@stateset.com'
    PASSWORD = 'your-password'

    client = StateSetClient(BASE_URL)

    try:
        print('\n=== StateSet API Python Example ===\n')

        # 1. Login
        print('1. Authentication')
        client.login(EMAIL, PASSWORD)

        # 2. List orders
        print('\n2. List Orders')
        orders = client.list_orders(status='pending', limit=5)

        # 3. Check low stock items
        print('\n3. Check Low Stock')
        low_stock = client.get_low_stock()

        # 4. Create a new order
        print('\n4. Create Order')
        customer_id = '550e8400-e29b-41d4-a716-446655440001'  # Replace with actual customer ID
        product_id = '550e8400-e29b-41d4-a716-446655440002'   # Replace with actual product ID

        new_order = client.create_order({
            'customer_id': customer_id,
            'status': 'pending',
            'total_amount': 199.98,
            'currency': 'USD',
            'items': [{
                'product_id': product_id,
                'sku': 'WIDGET-001',
                'quantity': 2,
                'unit_price': 99.99,
                'name': 'Premium Widget'
            }],
            'shipping_address': {
                'street': '123 Main St',
                'city': 'San Francisco',
                'state': 'CA',
                'postal_code': '94105',
                'country': 'US'
            }
        })

        order_id = new_order['id']

        # 5. Update order status
        print('\n5. Update Order Status')
        client.update_order_status(order_id, 'processing', 'Payment confirmed')

        # 6. Create shipment
        print('\n6. Create Shipment')
        shipment = client.create_shipment({
            'order_id': order_id,
            'carrier': 'UPS',
            'service_level': 'ground',
            'items': [{
                'order_item_id': new_order['items'][0]['id'],
                'quantity': 2
            }]
        })

        # 7. Mark as shipped
        print('\n7. Mark as Shipped')
        tracking_number = '1Z999AA10123456784'
        client.mark_shipped(shipment['id'], tracking_number)

        # 8. Track shipment
        print('\n8. Track Shipment')
        client.track_shipment(tracking_number)

        # 9. Get analytics
        print('\n9. Dashboard Metrics')
        dashboard = client.get_dashboard()
        print('Dashboard:', json.dumps(dashboard, indent=2))

        print('\n=== Example completed successfully! ===\n')

    except Exception as e:
        print(f'\n❌ Error: {str(e)}')
        exit(1)


if __name__ == '__main__':
    main()
