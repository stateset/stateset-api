// schema.rs

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
        category -> Varchar,
        weight -> Nullable<Float8>,
        dimensions -> Nullable<Varchar>,
        is_active -> Bool,
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
        shipping_address -> Text,
        billing_address -> Text,
        payment_method -> Varchar,
        shipping_method -> Varchar,
        notes -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

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
        id -> Uuid,
        user_id -> Uuid,
        product_id -> Uuid,
        order_id -> Uuid,
        warranty_number -> Varchar,
        start_date -> Timestamp,
        end_date -> Timestamp,
        status -> Varchar,
        terms -> Text,
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
        shipping_address -> Text,
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

joinable!(orders -> users (user_id));
joinable!(order_items -> orders (order_id));
joinable!(order_items -> products (product_id));
joinable!(returns -> orders (order_id));
joinable!(returns -> users (user_id));
joinable!(return_items -> returns (return_id));
joinable!(return_items -> products (product_id));
joinable!(warranties -> users (user_id));
joinable!(warranties -> products (product_id));
joinable!(warranties -> orders (order_id));
joinable!(shipments -> orders (order_id));
joinable!(inventory_transactions -> products (product_id));

allow_tables_to_appear_in_same_query!(
    users,
    products,
    orders,
    order_items,
    returns,
    return_items,
    warranties,
    shipments,
    inventory_transactions
);