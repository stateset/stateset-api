// schema.rs

// Existing tables (with some modifications)
table! {
    users (id) {
        id -> Uuid,
        username -> Varchar,
        email -> Varchar,
        password_hash -> Varchar,
        first_name -> Varchar,
        last_name -> Varchar,
        phone_number -> Nullable<Varchar>,
        is_active -> Bool,
        role -> Varchar, // New field for user roles (e.g., customer, admin, warehouse staff)
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    products (id) {
        id -> Uuid,
        sku -> Varchar,
        name -> Varchar,
        description -> Nullable<Text>,
        price -> Numeric,
        stock_quantity -> Int4,
        category_id -> Integer, // Changed to reference product_categories
        weight -> Nullable<Float8>,
        dimensions -> Nullable<Varchar>,
        is_active -> Bool,
        reorder_point -> Int4, // New field for inventory management
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    orders (id) {
        id -> Uuid,
        user_id -> Uuid,
        order_number -> Varchar,
        total_amount -> Numeric,
        status -> Varchar,
        shipping_address_id -> Uuid, // Changed to reference addresses table
        billing_address_id -> Uuid, // Changed to reference addresses table
        payment_method -> Varchar,
        shipping_method -> Varchar,
        notes -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

// Existing tables (unchanged)
table! {
    order_items (id) {
        id -> Uuid,
        order_id -> Uuid,
        product_id -> Uuid,
        quantity -> Int4,
        unit_price -> Numeric,
        total_price -> Numeric,
    }
}

table! {
    returns (id) {
        id -> Uuid,
        order_id -> Uuid,
        user_id -> Uuid,
        status -> Varchar,
        reason -> Text,
        requested_at -> Timestamp,
        processed_at -> Nullable<Timestamp>,
        refund_amount -> Nullable<Numeric>,
        notes -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    return_items (id) {
        id -> Uuid,
        return_id -> Uuid,
        product_id -> Uuid,
        quantity -> Int4,
        reason -> Varchar,
    }
}

table! {
    warranties (id) {
        id -> Integer,
        order_id -> Integer,
        customer_id -> Integer,
        product_id -> Integer,
        warranty_number -> Text,
        status -> Text,
        start_date -> Timestamp,
        end_date -> Timestamp,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    warranty_claims (id) {
        id -> Integer,
        warranty_id -> Integer,
        claim_date -> Timestamp,
        description -> Text,
        status -> Text,
        resolution -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    shipments (id) {
        id -> Uuid,
        order_id -> Uuid,
        tracking_number -> Varchar,
        carrier -> Varchar,
        status -> Varchar,
        shipping_address_id -> Uuid, // Changed to reference addresses table
        shipped_at -> Nullable<Timestamp>,
        estimated_delivery -> Nullable<Timestamp>,
        actual_delivery -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    inventory_transactions (id) {
        id -> Uuid,
        product_id -> Uuid,
        quantity_change -> Int4,
        transaction_type -> Varchar,
        reference_id -> Nullable<Uuid>,
        notes -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

table! {
    work_orders (id) {
        id -> Uuid,
        order_id -> Uuid,
        status -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    product_categories (id) {
        id -> Integer,
        name -> Varchar,
        description -> Nullable<Text>,
        parent_category_id -> Nullable<Integer>, // New field for hierarchical categories
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

// New tables for OMS and WMS

table! {
    addresses (id) {
        id -> Uuid,
        user_id -> Uuid,
        address_type -> Varchar, // e.g., "shipping", "billing"
        street_address -> Varchar,
        city -> Varchar,
        state -> Varchar,
        postal_code -> Varchar,
        country -> Varchar,
        is_default -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    warehouses (id) {
        id -> Uuid,
        name -> Varchar,
        address_id -> Uuid,
        manager_id -> Uuid, // References users table
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    warehouse_inventory (id) {
        id -> Uuid,
        warehouse_id -> Uuid,
        product_id -> Uuid,
        quantity -> Int4,
        location -> Varchar, // e.g., "A1-B2-C3" for aisle-shelf-bin
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    suppliers (id) {
        id -> Uuid,
        name -> Varchar,
        contact_name -> Varchar,
        email -> Varchar,
        phone -> Varchar,
        address_id -> Uuid,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    purchase_orders (id) {
        id -> Uuid,
        supplier_id -> Uuid,
        status -> Varchar,
        order_date -> Timestamp,
        expected_delivery_date -> Timestamp,
        total_amount -> Numeric,
        notes -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    purchase_order_items (id) {
        id -> Uuid,
        purchase_order_id -> Uuid,
        product_id -> Uuid,
        quantity -> Int4,
        unit_price -> Numeric,
        total_price -> Numeric,
    }
}

table! {
    picking_lists (id) {
        id -> Uuid,
        order_id -> Uuid,
        warehouse_id -> Uuid,
        status -> Varchar,
        picker_id -> Nullable<Uuid>, // References users table
        started_at -> Nullable<Timestamp>,
        completed_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    picking_list_items (id) {
        id -> Uuid,
        picking_list_id -> Uuid,
        product_id -> Uuid,
        warehouse_inventory_id -> Uuid,
        quantity -> Int4,
        picked_quantity -> Int4,
        status -> Varchar,
    }
}

table! {
    inventory_adjustments (id) {
        id -> Uuid,
        product_id -> Uuid,
        warehouse_id -> Uuid,
        adjustment_type -> Varchar, // e.g., "cycle count", "damaged", "lost"
        quantity_change -> Int4,
        reason -> Text,
        performed_by -> Uuid, // References users table
        approved_by -> Nullable<Uuid>, // References users table
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    promotions (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        promotion_type -> Varchar, // e.g., "percentage", "fixed amount", "buy one get one"
        discount_value -> Numeric,
        start_date -> Timestamp,
        end_date -> Timestamp,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    promotion_products (id) {
        id -> Uuid,
        promotion_id -> Uuid,
        product_id -> Uuid,
    }
}

table! {
    coupons (id) {
        id -> Uuid,
        code -> Varchar,
        description -> Text,
        discount_type -> Varchar, // e.g., "percentage", "fixed amount"
        discount_value -> Numeric,
        minimum_purchase_amount -> Nullable<Numeric>,
        usage_limit -> Nullable<Int4>,
        times_used -> Int4,
        start_date -> Timestamp,
        end_date -> Timestamp,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    order_coupons (id) {
        id -> Uuid,
        order_id -> Uuid,
        coupon_id -> Uuid,
        discount_amount -> Numeric,
    }
}

table! {
    product_reviews (id) {
        id -> Uuid,
        product_id -> Uuid,
        user_id -> Uuid,
        rating -> Int4,
        review_text -> Text,
        is_verified_purchase -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    customer_support_tickets (id) {
        id -> Uuid,
        user_id -> Uuid,
        order_id -> Nullable<Uuid>,
        subject -> Varchar,
        description -> Text,
        status -> Varchar, // e.g., "open", "in progress", "resolved"
        priority -> Varchar, // e.g., "low", "medium", "high"
        assigned_to -> Nullable<Uuid>, // References users table
        resolution -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    support_ticket_messages (id) {
        id -> Uuid,
        ticket_id -> Uuid,
        user_id -> Uuid,
        message -> Text,
        created_at -> Timestamp,
    }
}

table! {
    product_bundles (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        price -> Numeric,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    product_bundle_items (id) {
        id -> Uuid,
        bundle_id -> Uuid,
        product_id -> Uuid,
        quantity -> Int4,
    }
}

table! {
    delivery_zones (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    delivery_zone_regions (id) {
        id -> Uuid,
        zone_id -> Uuid,
        country -> Varchar,
        state -> Nullable<Varchar>,
        city -> Nullable<Varchar>,
        postal_code -> Nullable<Varchar>,
    }
}

table! {
    shipping_methods (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        base_cost -> Numeric,
        cost_per_kg -> Numeric,
        minimum_weight -> Float8,
        maximum_weight -> Float8,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    zone_shipping_methods (id) {
        id -> Uuid,
        zone_id -> Uuid,
        shipping_method_id -> Uuid,
        additional_cost -> Numeric,
    }
}

table! {
    subscriptions (id) {
        id -> Uuid,
        user_id -> Uuid,
        product_id -> Uuid,
        quantity -> Int4,
        frequency -> Varchar, // e.g., "weekly", "monthly", "quarterly"
        next_delivery_date -> Timestamp,
        status -> Varchar, // e.g., "active", "paused", "cancelled"
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    subscription_history (id) {
        id -> Uuid,
        subscription_id -> Uuid,
        order_id -> Uuid,
        processed_at -> Timestamp,
    }
}

table! {
    gift_cards (id) {
        id -> Uuid,
        code -> Varchar,
        initial_balance -> Numeric,
        current_balance -> Numeric,
        expiration_date -> Nullable<Timestamp>,
        is_active -> Bool,
        created_by -> Uuid, // References users table
        created_for -> Nullable<Uuid>, // References users table
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    gift_card_transactions (id) {
        id -> Uuid,
        gift_card_id -> Uuid,
        order_id -> Nullable<Uuid>,
        amount -> Numeric,
        transaction_type -> Varchar, // e.g., "purchase", "refund", "activation"
        created_at -> Timestamp,
    }
}

table! {
    inventory_batches (id) {
        id -> Uuid,
        product_id -> Uuid,
        warehouse_id -> Uuid,
        batch_number -> Varchar,
        quantity -> Int4,
        cost_per_unit -> Numeric,
        manufacturing_date -> Nullable<Timestamp>,
        expiry_date -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    third_party_logistics (id) {
        id -> Uuid,
        name -> Varchar,
        api_key -> Varchar,
        api_secret -> Varchar,
        integration_type -> Varchar, // e.g., "shipstation", "shipbob", "custom"
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    third_party_warehouses (id) {
        id -> Uuid,
        third_party_logistics_id -> Uuid,
        name -> Varchar,
        external_id -> Varchar,
        address_id -> Uuid,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    product_suppliers (id) {
        id -> Uuid,
        product_id -> Uuid,
        supplier_id -> Uuid,
        supplier_sku -> Nullable<Varchar>,
        lead_time_days -> Int4,
        minimum_order_quantity -> Int4,
        cost_per_unit -> Numeric,
        is_preferred_supplier -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    stock_replenishment_rules (id) {
        id -> Uuid,
        product_id -> Uuid,
        warehouse_id -> Uuid,
        reorder_point -> Int4,
        reorder_quantity -> Int4,
        target_stock_level -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    product_kits (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        sku -> Varchar,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    product_kit_items (id) {
        id -> Uuid,
        kit_id -> Uuid,
        product_id -> Uuid,
        quantity -> Int4,
    }
}

table! {
    invoice_items (id) {
        id -> Uuid,
        invoice_id -> Uuid,
        product_id -> Uuid,
        quantity -> Int4,
        unit_price -> Numeric,
        total_price -> Numeric,
        tax_rate -> Numeric,
        tax_amount -> Numeric,
    }
}

table! {
    currencies (id) {
        id -> Uuid,
        code -> Varchar,
        name -> Varchar,
        symbol -> Varchar,
        exchange_rate -> Numeric, // Rate relative to base currency
        is_base_currency -> Bool,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    exchange_rate_history (id) {
        id -> Uuid,
        currency_id -> Uuid,
        rate -> Numeric,
        effective_date -> Timestamp,
        created_at -> Timestamp,
    }
}

table! {
    tax_rates (id) {
        id -> Uuid,
        name -> Varchar,
        rate -> Numeric,
        country -> Varchar,
        state -> Nullable<Varchar>,
        zip_code -> Nullable<Varchar>,
        is_compound -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    product_tax_classes (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    product_tax_rates (id) {
        id -> Uuid,
        product_id -> Uuid,
        tax_rate_id -> Uuid,
        tax_class_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    reports (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        query -> Text,
        created_by -> Uuid,
        is_public -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    report_schedules (id) {
        id -> Uuid,
        report_id -> Uuid,
        frequency -> Varchar, // e.g., "daily", "weekly", "monthly"
        next_run -> Timestamp,
        recipients -> Varchar, // Comma-separated list of email addresses
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    user_roles (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    user_role_permissions (id) {
        id -> Uuid,
        role_id -> Uuid,
        permission -> Varchar,
        created_at -> Timestamp,
    }
}

table! {
    user_role_assignments (id) {
        id -> Uuid,
        user_id -> Uuid,
        role_id -> Uuid,
        created_at -> Timestamp,
    }
}

table! {
    loyalty_programs (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        points_currency_ratio -> Numeric,
        minimum_points_redemption -> Int4,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    loyalty_tiers (id) {
        id -> Uuid,
        program_id -> Uuid,
        name -> Varchar,
        minimum_points -> Int4,
        points_earning_multiplier -> Numeric,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    user_loyalty_accounts (id) {
        id -> Uuid,
        user_id -> Uuid,
        program_id -> Uuid,
        tier_id -> Uuid,
        points_balance -> Int4,
        lifetime_points -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    loyalty_transactions (id) {
        id -> Uuid,
        user_loyalty_account_id -> Uuid,
        order_id -> Nullable<Uuid>,
        points_earned -> Int4,
        points_redeemed -> Int4,
        transaction_type -> Varchar, // e.g., "earn", "redeem", "expire", "adjust"
        description -> Text,
        created_at -> Timestamp,
    }
}

table! {
    return_reasons (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    return_item_conditions (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        refund_percentage -> Numeric,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    return_items (id) {
        id -> Uuid,
        return_id -> Uuid,
        order_item_id -> Uuid,
        quantity -> Int4,
        reason_id -> Uuid,
        condition_id -> Uuid,
        refund_amount -> Numeric,
        notes -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    refunds (id) {
        id -> Uuid,
        order_id -> Uuid,
        amount -> Numeric,
        refund_method -> Varchar, // e.g., "credit_card", "store_credit", "bank_transfer"
        status -> Varchar, // e.g., "pending", "processed", "failed"
        reason -> Text,
        created_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    cross_sell_products (id) {
        id -> Uuid,
        product_id -> Uuid,
        related_product_id -> Uuid,
        relationship_type -> Varchar, // e.g., "cross_sell", "upsell", "related"
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    locales (id) {
        id -> Uuid,
        code -> Varchar,
        name -> Varchar,
        language_code -> Varchar,
        country_code -> Varchar,
        is_default -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    translations (id) {
        id -> Uuid,
        locale_id -> Uuid,
        translatable_type -> Varchar, // e.g., "product", "category", "shipping_method"
        translatable_id -> Uuid,
        field -> Varchar,
        content -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    inventory_forecasts (id) {
        id -> Uuid,
        product_id -> Uuid,
        warehouse_id -> Uuid,
        forecast_date -> Timestamp,
        forecasted_demand -> Int4,
        confidence_level -> Numeric,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    vendor_performance_metrics (id) {
        id -> Uuid,
        supplier_id -> Uuid,
        metric_type -> Varchar, // e.g., "on_time_delivery", "quality", "price_competitiveness"
        value -> Numeric,
        period_start -> Timestamp,
        period_end -> Timestamp,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    quality_control_inspections (id) {
        id -> Uuid,
        product_id -> Uuid,
        batch_id -> Nullable<Uuid>,
        inspector_id -> Uuid,
        inspection_date -> Timestamp,
        status -> Varchar, // e.g., "passed", "failed", "pending"
        notes -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    quality_control_criteria (id) {
        id -> Uuid,
        product_id -> Uuid,
        name -> Varchar,
        description -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    quality_control_results (id) {
        id -> Uuid,
        inspection_id -> Uuid,
        criteria_id -> Uuid,
        result -> Varchar, // e.g., "pass", "fail", "n/a"
        notes -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

table! {
    customer_segments (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        criteria -> Jsonb, // Store segment criteria as JSON
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    customer_segment_members (id) {
        id -> Uuid,
        segment_id -> Uuid,
        user_id -> Uuid,
        created_at -> Timestamp,
    }
}

table! {
    personalized_recommendations (id) {
        id -> Uuid,
        user_id -> Uuid,
        product_id -> Uuid,
        score -> Numeric,
        recommendation_type -> Varchar, // e.g., "personal", "trending", "frequently_bought_together"
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    external_marketplaces (id) {
        id -> Uuid,
        name -> Varchar,
        api_key -> Varchar,
        api_secret -> Varchar,
        integration_type -> Varchar, // e.g., "amazon", "ebay", "walmart"
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    marketplace_listings (id) {
        id -> Uuid,
        marketplace_id -> Uuid,
        product_id -> Uuid,
        external_id -> Varchar,
        status -> Varchar, // e.g., "active", "inactive", "pending"
        price -> Numeric,
        quantity -> Int4,
        last_synced_at -> Timestamp,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    shipping_carriers (id) {
        id -> Uuid,
        name -> Varchar,
        api_key -> Nullable<Varchar>,
        api_secret -> Nullable<Varchar>,
        account_number -> Nullable<Varchar>,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    shipping_rules (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        conditions -> Jsonb, // Store rule conditions as JSON
        actions -> Jsonb, // Store rule actions as JSON
        priority -> Int4,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    carrier_service_types (id) {
        id -> Uuid,
        carrier_id -> Uuid,
        name -> Varchar,
        code -> Varchar,
        description -> Text,
        estimated_delivery_time -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    analytics_events (id) {
        id -> Uuid,
        event_type -> Varchar,
        event_data -> Jsonb,
        user_id -> Nullable<Uuid>,
        session_id -> Varchar,
        ip_address -> Varchar,
        user_agent -> Text,
        occurred_at -> Timestamp,
        created_at -> Timestamp,
    }
}

table! {
    data_warehouse_syncs (id) {
        id -> Uuid,
        table_name -> Varchar,
        last_sync_at -> Timestamp,
        rows_synced -> Int8,
        status -> Varchar,
        error_message -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    workflows (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Text,
        trigger_event -> Varchar,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    workflow_steps (id) {
        id -> Uuid,
        workflow_id -> Uuid,
        step_type -> Varchar,
        step_data -> Jsonb,
        order -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    workflow_logs (id) {
        id -> Uuid,
        workflow_id -> Uuid,
        triggered_by_type -> Varchar,
        triggered_by_id -> Uuid,
        status -> Varchar,
        started_at -> Timestamp,
        completed_at -> Nullable<Timestamp>,
        error_message -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

table! {
    supplier_portal_users (id) {
        id -> Uuid,
        supplier_id -> Uuid,
        name -> Varchar,
        email -> Varchar,
        password_hash -> Varchar,
        is_active -> Bool,
        last_login_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    supplier_portal_notifications (id) {
        id -> Uuid,
        supplier_id -> Uuid,
        notification_type -> Varchar,
        content -> Text,
        is_read -> Bool,
        created_at -> Timestamp,
        read_at -> Nullable<Timestamp>,
    }
}

table! {
    inventory_transfers (id) {
        id -> Uuid,
        source_warehouse_id -> Uuid,
        destination_warehouse_id -> Uuid,
        status -> Varchar,
        requested_by -> Uuid,
        approved_by -> Nullable<Uuid>,
        requested_at -> Timestamp,
        approved_at -> Nullable<Timestamp>,
        completed_at -> Nullable<Timestamp>,
        notes -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    inventory_transfer_items (id) {
        id -> Uuid,
        transfer_id -> Uuid,
        product_id -> Uuid,
        requested_quantity -> Int4,
        approved_quantity -> Nullable<Int4>,
        transferred_quantity -> Nullable<Int4>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    product_configurations (id) {
        id -> Uuid,
        product_id -> Uuid,
        name -> Varchar,
        description -> Text,
        is_active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    configuration_options (id) {
        id -> Uuid,
        configuration_id -> Uuid,
        name -> Varchar,
        option_type -> Varchar, // e.g., "select", "radio", "checkbox"
        is_required -> Bool,
        display_order -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    configuration_option_values (id) {
        id -> Uuid,
        option_id -> Uuid,
        value -> Varchar,
        price_adjustment -> Numeric,
        display_order -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    consignment_inventory (id) {
        id -> Uuid,
        supplier_id -> Uuid,
        product_id -> Uuid,
        warehouse_id -> Uuid,
        quantity -> Int4,
        price_per_unit -> Numeric,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    consignment_sales (id) {
        id -> Uuid,
        consignment_inventory_id -> Uuid,
        order_item_id -> Uuid,
        quantity_sold -> Int4,
        sale_price -> Numeric,
        commission_rate -> Numeric,
        commission_amount -> Numeric,
        created_at -> Timestamp,
    }
}

table! {
    iot_devices (id) {
        id -> Uuid,
        name -> Varchar,
        device_type -> Varchar,
        warehouse_id -> Uuid,
        status -> Varchar,
        last_ping -> Timestamp,
        metadata -> Jsonb,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    iot_device_readings (id) {
        id -> Uuid,
        device_id -> Uuid,
        reading_type -> Varchar,
        reading_value -> Jsonb,
        timestamp -> Timestamp,
        created_at -> Timestamp,
    }
}

// Add more joinable! statements for the new tables
joinable!(addresses -> users (user_id));
joinable!(orders -> addresses (shipping_address_id));
joinable!(orders -> addresses (billing_address_id));
joinable!(shipments -> addresses (shipping_address_id));
joinable!(warehouses -> addresses (address_id));
joinable!(warehouses -> users (manager_id));
joinable!(warehouse_inventory -> warehouses (warehouse_id));
joinable!(warehouse_inventory -> products (product_id));
joinable!(suppliers -> addresses (address_id));
joinable!(purchase_orders -> suppliers (supplier_id));
joinable!(purchase_order_items -> purchase_orders (purchase_order_id));
joinable!(purchase_order_items -> products (product_id));
joinable!(picking_lists -> orders (order_id));
joinable!(picking_lists -> warehouses (warehouse_id));
joinable!(picking_lists -> users (picker_id));
joinable!(picking_list_items -> picking_lists (picking_list_id));
joinable!(picking_list_items -> products (product_id));
joinable!(picking_list_items -> warehouse_inventory (warehouse_inventory_id));
joinable!(products -> product_categories (category_id));
joinable!(inventory_adjustments -> products (product_id));
joinable!(inventory_adjustments -> warehouses (warehouse_id));
joinable!(inventory_adjustments -> users (performed_by));
joinable!(inventory_adjustments -> users (approved_by));
joinable!(promotion_products -> promotions (promotion_id));
joinable!(promotion_products -> products (product_id));
joinable!(order_coupons -> orders (order_id));
joinable!(order_coupons -> coupons (coupon_id));
joinable!(product_reviews -> products (product_id));
joinable!(product_reviews -> users (user_id));
joinable!(customer_support_tickets -> users (user_id));
joinable!(customer_support_tickets -> orders (order_id));
joinable!(customer_support_tickets -> users (assigned_to));
joinable!(support_ticket_messages -> customer_support_tickets (ticket_id));
joinable!(support_ticket_messages -> users (user_id));
joinable!(product_bundle_items -> product_bundles (bundle_id));
joinable!(product_bundle_items -> products (product_id));
joinable!(delivery_zone_regions -> delivery_zones (zone_id));
joinable!(zone_shipping_methods -> delivery_zones (zone_id));
joinable!(zone_shipping_methods -> shipping_methods (shipping_method_id));
joinable!(subscriptions -> users (user_id));
joinable!(subscriptions -> products (product_id));
joinable!(subscription_history -> subscriptions (subscription_id));
joinable!(subscription_history -> orders (order_id));
joinable!(gift_cards -> users (created_by));
joinable!(gift_cards -> users (created_for));
joinable!(gift_card_transactions -> gift_cards (gift_card_id));
joinable!(gift_card_transactions -> orders (order_id));
joinable!(inventory_batches -> products (product_id));
joinable!(inventory_batches -> warehouses (warehouse_id));
joinable!(third_party_warehouses -> third_party_logistics (third_party_logistics_id));
joinable!(third_party_warehouses -> addresses (address_id));
joinable!(product_suppliers -> products (product_id));
joinable!(product_suppliers -> suppliers (supplier_id));
joinable!(stock_replenishment_rules -> products (product_id));
joinable!(stock_replenishment_rules -> warehouses (warehouse_id));
joinable!(product_kit_items -> product_kits (kit_id));
joinable!(product_kit_items -> products (product_id));
joinable!(invoice_items -> invoices (invoice_id));
joinable!(invoice_items -> products (product_id));
joinable!(invoices -> orders (order_id));
joinable!(exchange_rate_history -> currencies (currency_id));
joinable!(product_tax_rates -> products (product_id));
joinable!(product_tax_rates -> tax_rates (tax_rate_id));
joinable!(product_tax_rates -> product_tax_classes (tax_class_id));
joinable!(reports -> users (created_by));
joinable!(report_schedules -> reports (report_id));
joinable!(user_role_permissions -> user_roles (role_id));
joinable!(user_role_assignments -> users (user_id));
joinable!(user_role_assignments -> user_roles (role_id));
joinable!(loyalty_tiers -> loyalty_programs (program_id));
joinable!(user_loyalty_accounts -> users (user_id));
joinable!(user_loyalty_accounts -> loyalty_programs (program_id));
joinable!(user_loyalty_accounts -> loyalty_tiers (tier_id));
joinable!(loyalty_transactions -> user_loyalty_accounts (user_loyalty_account_id));
joinable!(loyalty_transactions -> orders (order_id));
joinable!(return_items -> returns (return_id));
joinable!(return_items -> order_items (order_item_id));
joinable!(return_items -> return_reasons (reason_id));
joinable!(return_items -> return_item_conditions (condition_id));
joinable!(refunds -> orders (order_id));
joinable!(refunds -> users (created_by));
joinable!(cross_sell_products -> products (product_id));
joinable!(cross_sell_products -> products (related_product_id));
joinable!(translations -> locales (locale_id));
joinable!(inventory_forecasts -> products (product_id));
joinable!(inventory_forecasts -> warehouses (warehouse_id));
joinable!(vendor_performance_metrics -> suppliers (supplier_id));
joinable!(quality_control_inspections -> products (product_id));
joinable!(quality_control_inspections -> inventory_batches (batch_id));
joinable!(quality_control_inspections -> users (inspector_id));
joinable!(quality_control_criteria -> products (product_id));
joinable!(quality_control_results -> quality_control_inspections (inspection_id));
joinable!(quality_control_results -> quality_control_criteria (criteria_id));
joinable!(customer_segment_members -> customer_segments (segment_id));
joinable!(customer_segment_members -> users (user_id));
joinable!(personalized_recommendations -> users (user_id));
joinable!(personalized_recommendations -> products (product_id));
joinable!(marketplace_listings -> external_marketplaces (marketplace_id));
joinable!(marketplace_listings -> products (product_id));
joinable!(carrier_service_types -> shipping_carriers (carrier_id));
joinable!(analytics_events -> users (user_id));
joinable!(workflow_steps -> workflows (workflow_id));
joinable!(workflow_logs -> workflows (workflow_id));
joinable!(supplier_portal_users -> suppliers (supplier_id));
joinable!(supplier_portal_notifications -> suppliers (supplier_id));
joinable!(inventory_transfers -> warehouses (source_warehouse_id));
joinable!(inventory_transfers -> warehouses (destination_warehouse_id));
joinable!(inventory_transfers -> users (requested_by));
joinable!(inventory_transfers -> users (approved_by));
joinable!(inventory_transfer_items -> inventory_transfers (transfer_id));
joinable!(inventory_transfer_items -> products (product_id));
joinable!(product_configurations -> products (product_id));
joinable!(configuration_options -> product_configurations (configuration_id));
joinable!(configuration_option_values -> configuration_options (option_id));
joinable!(consignment_inventory -> suppliers (supplier_id));
joinable!(consignment_inventory -> products (product_id));
joinable!(consignment_inventory -> warehouses (warehouse_id));
joinable!(consignment_sales -> consignment_inventory (consignment_inventory_id));
joinable!(consignment_sales -> order_items (order_item_id));
joinable!(iot_devices -> warehouses (warehouse_id));
joinable!(iot_device_readings -> iot_devices (device_id));

// Allow all tables to appear in the same query
allow_tables_to_appear_in_same_query!(
    users,
    products,
    orders,
    order_items,
    returns,
    return_items,
    warranties,
    warranty_claims,
    shipments,
    inventory_transactions,
    work_orders,
    product_categories,
    addresses,
    warehouses,
    warehouse_inventory,
    suppliers,
    purchase_orders,
    purchase_order_items,
    picking_lists,
    picking_list_items,
    inventory_adjustments,
    promotions,
    promotion_products,
    coupons,
    order_coupons,
    product_reviews,
    customer_support_tickets,
    support_ticket_messages,
    product_bundles,
    product_bundle_items,
    delivery_zones,
    delivery_zone_regions,
    shipping_methods,
    zone_shipping_methods,
    subscriptions,
    subscription_history,
    gift_cards,
    gift_card_transactions,
    inventory_batches,
    third_party_logistics,
    third_party_warehouses,
    product_suppliers,
    stock_replenishment_rules,
    product_kits,
    product_kit_items,
    invoices,
    invoice_items,
    currencies,
    exchange_rate_history,
    tax_rates,
    product_tax_classes,
    product_tax_rates,
    reports,
    report_schedules,
    user_roles,
    user_role_permissions,
    user_role_assignments,
    loyalty_programs,
    loyalty_tiers,
    user_loyalty_accounts,
    loyalty_transactions,
    return_reasons,
    return_item_conditions,
    return_items,
    refunds,
    cross_sell_products,
    locales,
    translations,
    inventory_forecasts,
    vendor_performance_metrics,
    quality_control_inspections,
    quality_control_criteria,
    quality_control_results,
    customer_segments,
    customer_segment_members,
    personalized_recommendations,
    external_marketplaces,
    marketplace_listings,
    shipping_carriers,
    shipping_rules,
    carrier_service_types,
    analytics_events,
    data_warehouse_syncs,
    workflows,
    workflow_steps,
    workflow_logs,
    supplier_portal_users,
    supplier_portal_notifications,
    inventory_transfers,
    inventory_transfer_items,
    product_configurations,
    configuration_options,
    configuration_option_values,
    consignment_inventory,
    consignment_sales,
    iot_devices,
    iot_device_readings
);