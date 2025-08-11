pub use sea_orm_migration::prelude::*;

mod m20230101_000001_create_orders_table;
mod m20230101_000002_create_order_items_table;
mod m20230101_000011_create_warranty_table;
mod m20230101_000012_create_inventory_transactions_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230101_000001_create_orders_table::Migration),
            Box::new(m20230101_000002_create_order_items_table::Migration),
            Box::new(m20230101_000011_create_warranty_table::Migration),
            Box::new(m20230101_000012_create_inventory_transactions_table::Migration),
        ]
    }
}
