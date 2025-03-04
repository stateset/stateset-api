// Import diesel macros and prelude
use diesel::prelude::*;

// This will be replaced with SeaORM entities but is kept for backward compatibility
table! {
    customers (id) {
        id -> Text,
        name -> Text,
        email -> Nullable<Text>,
        phone -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    inventory_items (id) {
        id -> Text,
        sku -> Text, 
        name -> Text,
        description -> Nullable<Text>,
        quantity -> Integer,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    orders (id) {
        id -> Text,
        customer_id -> Text,
        status -> Text,
        total_amount -> Numeric,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    returns (id) {
        id -> Text,
        order_id -> Text,
        status -> Text,
        reason -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

// Add missing model modules for basic persistence
allow_tables_to_appear_in_same_query!(
    customers,
    inventory_items,
    orders,
    returns,
);