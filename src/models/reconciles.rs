use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, Utc, NaiveDate};
use uuid::Uuid;
use rust_decimal::Decimal;

// Reconcile Model (updated to include relation to line items)
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "reconciles")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub number: i32,
    pub site: String,
    pub order_number: String,
    pub bill_number: String,
    pub vendor: String,
    pub vendor_so_number: String,
    pub terms: String,
    pub priority: String,
    pub assigned_user: String,
    #[sea_orm(column_name = "type")]
    pub reconcile_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub bill_date: NaiveDate,
    pub bill_due_date: NaiveDate,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub billed_total_cost: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub landed_total_cost: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub po_total_cost: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub bill_balance: Decimal,
    pub memo: String,
    pub categories: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::reconcile_line_item::Entity")]
    ReconcileLineItems,
}

impl Related<super::reconcile_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReconcileLineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// ... (previous Reconcile model methods remain the same)

impl Model {
    // ... (previous methods remain the same)

    pub async fn add_line_item(&self, line_item: ReconcileLineItem, db: &DatabaseConnection) -> Result<(), DbErr> {
        let line_item = reconcile_line_item::ActiveModel {
            id: Set(Uuid::new_v4()),
            reconcile_number: Set(self.number),
            line_type: Set(line_item.line_type),
            status: Set(line_item.status),
            part_number: Set(line_item.part_number),
            part_name: Set(line_item.part_name),
            vendor_part_number: Set(line_item.vendor_part_number),
            vendor_part_name: Set(line_item.vendor_part_name),
            order: Set(line_item.order),
            quantity_billed: Set(line_item.quantity_billed),
            billed_unit_cost: Set(line_item.billed_unit_cost),
            billed_total_cost: Set(line_item.billed_total_cost),
            vendor: Set(line_item.vendor),
        };
        ReconcileLineItem::insert(line_item).exec(db).await?;
        Ok(())
    }

    pub async fn get_line_items(&self, db: &DatabaseConnection) -> Result<Vec<reconcile_line_item::Model>, DbErr> {
        ReconcileLineItem::find()
            .filter(reconcile_line_item::Column::ReconcileNumber.eq(self.number))
            .all(db)
            .await
    }

    pub async fn calculate_totals(&mut self, db: &DatabaseConnection) -> Result<(), DbErr> {
        let line_items = self.get_line_items(db).await?;
        self.billed_total_cost = line_items.iter().map(|item| item.billed_total_cost).sum();
        self.bill_balance = self.billed_total_cost;
        self.updated_at = Utc::now();
        Ok(())
    }
}

// Reconcile Line Item Model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "reconcile_line_items")]
pub struct ReconcileLineItem {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub line_type: String,
    pub status: String,
    pub part_number: String,
    pub part_name: String,
    pub vendor_part_number: String,
    pub vendor_part_name: String,
    pub order: String,
    pub quantity_billed: i32,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub billed_unit_cost: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub billed_total_cost: Decimal,
    pub vendor: String,
    pub reconcile_number: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum ReconcileLineItemRelation {
    #[sea_orm(
        belongs_to = "super::reconcile::Entity",
        from = "Column::ReconcileNumber",
        to = "super::reconcile::Column::Number"
    )]
    Reconcile,
}

impl Related<super::reconcile::Entity> for ReconcileLineItem {
    fn to() -> RelationDef {
        ReconcileLineItemRelation::Reconcile.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl ReconcileLineItem {
    pub fn new(
        line_type: String,
        status: String,
        part_number: String,
        part_name: String,
        vendor_part_number: String,
        vendor_part_name: String,
        order: String,
        quantity_billed: i32,
        billed_unit_cost: Decimal,
        vendor: String,
        reconcile_number: i32,
    ) -> Self {
        let billed_total_cost = Decimal::from(quantity_billed) * billed_unit_cost;
        Self {
            id: Uuid::new_v4(),
            line_type,
            status,
            part_number,
            part_name,
            vendor_part_number,
            vendor_part_name,
            order,
            quantity_billed,
            billed_unit_cost,
            billed_total_cost,
            vendor,
            reconcile_number,
        }
    }

    pub fn update_quantity(&mut self, new_quantity: i32) {
        self.quantity_billed = new_quantity;
        self.billed_total_cost = Decimal::from(self.quantity_billed) * self.billed_unit_cost;
    }

    pub fn update_unit_cost(&mut self, new_unit_cost: Decimal) {
        self.billed_unit_cost = new_unit_cost;
        self.billed_total_cost = Decimal::from(self.quantity_billed) * self.billed_unit_cost;
    }
}