pub use sea_orm_migration::prelude::*;

mod m20230101_000001_create_orders_table;
mod m20230101_000002_create_order_items_table;
mod m20230101_000011_create_warranty_table;
mod m20230101_000012_create_inventory_transactions_table;
mod m20240101_000014_create_outbox_table;
mod m20240901_000013_create_auth_tables;
mod m20241005_000015_update_order_timestamps;
mod m20241105_000016_create_manufacturing_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230101_000001_create_orders_table::Migration),
            Box::new(m20230101_000002_create_order_items_table::Migration),
            Box::new(m20230101_000011_create_warranty_table::Migration),
            Box::new(m20230101_000012_create_inventory_transactions_table::Migration),
            Box::new(m20240901_000013_create_auth_tables::Migration),
            Box::new(m20240101_000014_create_outbox_table::Migration),
            Box::new(m20241005_000015_update_order_timestamps::Migration),
            Box::new(m20241105_000016_create_manufacturing_tables::Migration),
        ]
    }
}
