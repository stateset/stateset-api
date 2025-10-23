use super::reconcile_line_item;
use super::reconcile_line_item::Model as ReconcileLineItem;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

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

    pub async fn add_line_item(
        &self,
        line_item: ReconcileLineItem,
        db: &DatabaseConnection,
    ) -> Result<(), DbErr> {
        let mut am: reconcile_line_item::ActiveModel = line_item.into();
        am.reconcile_number = Set(self.number);
        am.insert(db).await?;
        Ok(())
    }

    pub async fn get_line_items(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<reconcile_line_item::Model>, DbErr> {
        reconcile_line_item::Entity::find()
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
