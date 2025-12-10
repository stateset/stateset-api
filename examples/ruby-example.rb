#!/usr/bin/env ruby
# frozen_string_literal: true

# StateSet API - Ruby Client Example
#
# A comprehensive Ruby client for the StateSet API demonstrating:
# - Authentication (JWT & API Keys)
# - Order management
# - Inventory operations
# - Shopping cart & checkout
# - Returns & shipments
# - Customer management
# - Analytics
#
# Installation:
# gem install httparty securerandom
#
# Usage:
# ruby ruby-example.rb

require 'httparty'
require 'json'
require 'securerandom'
require 'time'

# ============================================================================
# StateSet API Client
# ============================================================================

class StateSetClient
  include HTTParty

  attr_accessor :access_token, :refresh_token

  def initialize(base_url = 'http://localhost:8080/api/v1')
    @base_url = base_url
    @access_token = nil
    @refresh_token = nil
    self.class.base_uri @base_url
    self.class.headers 'Content-Type' => 'application/json'
  end

  # ==========================================================================
  # Helper Methods
  # ==========================================================================

  def request(method, path, body: nil, headers: {})
    options = {
      headers: default_headers.merge(headers)
    }
    options[:body] = body.to_json if body

    response = self.class.send(method, path, options)

    if response.code >= 400
      handle_error(response)
    else
      response.parsed_response
    end
  rescue => e
    raise "Request failed: #{e.message}"
  end

  def default_headers
    headers = {}
    headers['Authorization'] = "Bearer #{@access_token}" if @access_token
    headers
  end

  def handle_error(response)
    error_data = response.parsed_response
    if error_data && error_data['error']
      error = error_data['error']
      raise "API Error: #{error['message']} (code: #{error['code']}, status: #{error['status']})"
    else
      raise "HTTP Error: #{response.code} #{response.message}"
    end
  end

  # ==========================================================================
  # Authentication
  # ==========================================================================

  def login(email, password)
    body = {
      email: email,
      password: password
    }

    response = request(:post, '/auth/login', body: body)
    @access_token = response['access_token']
    @refresh_token = response['refresh_token']

    response
  end

  def register(email, password, first_name, last_name)
    body = {
      email: email,
      password: password,
      first_name: first_name,
      last_name: last_name
    }

    response = request(:post, '/auth/register', body: body)
    @access_token = response['access_token']
    @refresh_token = response['refresh_token']

    response
  end

  def create_api_key(name, permissions)
    body = {
      name: name,
      permissions: permissions,
      expires_at: (Time.now + (365 * 24 * 60 * 60)).iso8601
    }

    request(:post, '/auth/api-keys', body: body)
  end

  def logout
    request(:post, '/auth/logout')
    @access_token = nil
    @refresh_token = nil
  end

  # ==========================================================================
  # Orders
  # ==========================================================================

  def create_order(customer_id:, items:, shipping_address: nil, billing_address: nil)
    body = {
      customer_id: customer_id,
      items: items
    }
    body[:shipping_address] = shipping_address if shipping_address
    body[:billing_address] = billing_address if billing_address

    request(:post, '/orders', body: body)
  end

  def list_orders(page: 1, limit: 10, status: nil, customer_id: nil)
    params = { page: page, limit: limit }
    params[:status] = status if status
    params[:customer_id] = customer_id if customer_id

    query_string = params.map { |k, v| "#{k}=#{v}" }.join('&')
    request(:get, "/orders?#{query_string}")
  end

  def get_order(order_id)
    request(:get, "/orders/#{order_id}")
  end

  def update_order_status(order_id, status, notes: nil)
    body = { status: status }
    body[:notes] = notes if notes

    request(:put, "/orders/#{order_id}/status", body: body)
  end

  def cancel_order(order_id, reason: nil)
    body = {}
    body[:reason] = reason if reason

    request(:post, "/orders/#{order_id}/cancel", body: body)
  end

  def refund_order(order_id, amount, reason)
    body = {
      amount: amount,
      reason: reason,
      idempotency_key: SecureRandom.uuid
    }

    request(:post, "/orders/#{order_id}/refund", body: body)
  end

  # ==========================================================================
  # Inventory
  # ==========================================================================

  def list_inventory(page: 1, limit: 10, location_id: nil)
    params = { page: page, limit: limit }
    params[:location_id] = location_id if location_id

    query_string = params.map { |k, v| "#{k}=#{v}" }.join('&')
    request(:get, "/inventory?#{query_string}")
  end

  def get_inventory_item(id)
    request(:get, "/inventory/#{id}")
  end

  def get_low_stock_items
    request(:get, '/inventory/low-stock')
  end

  def reserve_inventory(id, quantity, order_id)
    body = {
      quantity: quantity,
      order_id: order_id,
      expires_at: (Time.now + (24 * 60 * 60)).iso8601
    }

    request(:post, "/inventory/#{id}/reserve", body: body)
  end

  def release_inventory(reservation_id)
    request(:post, "/inventory/reservations/#{reservation_id}/cancel")
  end

  def adjust_inventory(id, quantity, reason)
    body = {
      quantity: quantity,
      reason: reason
    }

    request(:post, "/inventory/#{id}/adjust", body: body)
  end

  # ==========================================================================
  # Shopping Cart
  # ==========================================================================

  def create_cart(customer_id: nil)
    body = {
      session_id: SecureRandom.uuid
    }
    body[:customer_id] = customer_id if customer_id

    request(:post, '/carts', body: body)
  end

  def get_cart(cart_id)
    request(:get, "/carts/#{cart_id}")
  end

  def add_item_to_cart(cart_id, product_id:, sku:, quantity:, price:, name:)
    body = {
      product_id: product_id,
      sku: sku,
      quantity: quantity,
      price: price,
      name: name
    }

    request(:post, "/carts/#{cart_id}/items", body: body)
  end

  def update_cart_item(cart_id, item_id, quantity)
    body = { quantity: quantity }
    request(:put, "/carts/#{cart_id}/items/#{item_id}", body: body)
  end

  def remove_cart_item(cart_id, item_id)
    request(:delete, "/carts/#{cart_id}/items/#{item_id}")
  end

  def checkout(cart_id, customer_id:, shipping_address:, billing_address:, payment_method:)
    body = {
      cart_id: cart_id,
      customer_id: customer_id,
      shipping_address: shipping_address,
      billing_address: billing_address,
      payment_method: payment_method
    }

    request(:post, '/checkout', body: body)
  end

  # ==========================================================================
  # Customers
  # ==========================================================================

  def create_customer(email:, first_name:, last_name:, phone: nil)
    body = {
      email: email,
      first_name: first_name,
      last_name: last_name
    }
    body[:phone] = phone if phone

    request(:post, '/customers', body: body)
  end

  def list_customers(page: 1, limit: 10, search: nil)
    params = { page: page, limit: limit }
    params[:search] = search if search

    query_string = params.map { |k, v| "#{k}=#{v}" }.join('&')
    request(:get, "/customers?#{query_string}")
  end

  def get_customer(id)
    request(:get, "/customers/#{id}")
  end

  def update_customer(id, data)
    request(:put, "/customers/#{id}", body: data)
  end

  # ==========================================================================
  # Returns
  # ==========================================================================

  def create_return(order_id:, items:, customer_notes: nil)
    body = {
      order_id: order_id,
      items: items
    }
    body[:customer_notes] = customer_notes if customer_notes

    request(:post, '/returns', body: body)
  end

  def list_returns(page: 1, limit: 10, status: nil)
    params = { page: page, limit: limit }
    params[:status] = status if status

    query_string = params.map { |k, v| "#{k}=#{v}" }.join('&')
    request(:get, "/returns?#{query_string}")
  end

  def get_return(id)
    request(:get, "/returns/#{id}")
  end

  def approve_return(id)
    request(:post, "/returns/#{id}/approve")
  end

  def restock_return(id)
    request(:post, "/returns/#{id}/restock")
  end

  # ==========================================================================
  # Shipments
  # ==========================================================================

  def create_shipment(order_id:, carrier:, service_level:)
    body = {
      order_id: order_id,
      carrier: carrier,
      service_level: service_level
    }

    request(:post, '/shipments', body: body)
  end

  def mark_as_shipped(shipment_id, tracking_number)
    body = {
      tracking_number: tracking_number,
      shipped_at: Time.now.iso8601
    }

    request(:post, "/shipments/#{shipment_id}/ship", body: body)
  end

  def track_shipment(tracking_number)
    request(:get, "/shipments/track/#{tracking_number}")
  end

  def list_shipments(page: 1, limit: 10)
    params = { page: page, limit: limit }
    query_string = params.map { |k, v| "#{k}=#{v}" }.join('&')
    request(:get, "/shipments?#{query_string}")
  end

  # ==========================================================================
  # Analytics
  # ==========================================================================

  def get_dashboard_metrics
    request(:get, '/analytics/dashboard')
  end

  def get_sales_trends(start_date: nil, end_date: nil, interval: nil)
    params = {}
    params[:start_date] = start_date if start_date
    params[:end_date] = end_date if end_date
    params[:interval] = interval if interval

    query_string = params.any? ? '?' + params.map { |k, v| "#{k}=#{v}" }.join('&') : ''
    request(:get, "/analytics/sales/trends#{query_string}")
  end

  def get_inventory_analytics
    request(:get, '/analytics/inventory')
  end

  # ==========================================================================
  # Health & Status
  # ==========================================================================

  def get_health
    request(:get, '/health')
  end

  def get_status
    request(:get, '/status')
  end
end

# ============================================================================
# Example Usage
# ============================================================================

def main
  client = StateSetClient.new('http://localhost:8080/api/v1')

  puts "🚀 StateSet API Ruby Example\n\n"

  begin
    # 1. Authentication
    puts '1️⃣  Authenticating...'
    client.login('admin@stateset.com', 'your-password')
    puts "✅ Logged in successfully\n\n"

    # 2. Check API health
    puts '2️⃣  Checking API health...'
    health = client.get_health
    puts "✅ API Status: #{health['status']}\n\n"

    # 3. Create a customer
    puts '3️⃣  Creating customer...'
    customer = client.create_customer(
      email: "test-#{Time.now.to_i}@example.com",
      first_name: 'John',
      last_name: 'Doe',
      phone: '+1-555-0100'
    )
    puts "✅ Customer created: #{customer['id']}\n\n"

    # 4. Create a shopping cart
    puts '4️⃣  Creating shopping cart...'
    cart = client.create_cart(customer_id: customer['id'])
    puts "✅ Cart created: #{cart['id']}\n\n"

    # 5. Add items to cart
    puts '5️⃣  Adding items to cart...'
    client.add_item_to_cart(
      cart['id'],
      product_id: SecureRandom.uuid,
      sku: 'WIDGET-001',
      quantity: 2,
      price: 99.99,
      name: 'Premium Widget'
    )
    puts "✅ Items added to cart\n\n"

    # 6. Get cart details
    updated_cart = client.get_cart(cart['id'])
    puts "📦 Cart total: $#{updated_cart['total']}\n\n"

    # 7. List orders
    puts '7️⃣  Listing orders...'
    orders = client.list_orders(page: 1, limit: 5)
    puts "✅ Found #{orders['data'] ? orders['data'].length : 0} orders\n\n"

    # 8. Check low stock items
    puts '8️⃣  Checking low stock items...'
    low_stock = client.get_low_stock_items
    items_count = low_stock['data'] ? low_stock['data'].length : 0
    puts "📊 Low stock items: #{items_count}\n\n"

    # 9. Get dashboard metrics
    puts '9️⃣  Fetching analytics...'
    dashboard = client.get_dashboard_metrics
    puts "📈 Dashboard metrics retrieved\n\n"

    # 10. Create API key
    puts '🔟 Creating API key...'
    api_key = client.create_api_key('Test API Key', ['orders:read', 'inventory:read'])
    key_preview = api_key['key'][0...20] + '...'
    puts "✅ API Key created: #{key_preview}\n\n"

    puts '✨ All examples completed successfully!'

  rescue => e
    puts "❌ Error: #{e.message}"
    puts e.backtrace.take(5).join("\n") if ENV['DEBUG']
    exit 1
  end
end

# Run the example if this file is executed directly
main if __FILE__ == $PROGRAM_NAME
