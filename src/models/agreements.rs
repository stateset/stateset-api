use chrono::NaiveDate;
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

// Agreement Model (updated to include relation to line items)
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "agreements")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub agreement_name: String,
    pub agreement_number: String,
    pub agreement_hash: Option<String>,
    pub agreement_status: Option<String>,
    pub agreement_type: Option<String>,
    pub total_agreement_value: Option<String>,
    pub attachments: Option<String>,
    pub party: Option<String>,
    pub counterparty: Option<String>,
    pub linear_id: Option<String>,
    pub opportunity_id: Option<String>,
    pub user_id: Option<String>,
    pub price_list_id: Option<String>,
    pub order_id: Option<String>,
    pub account_id: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub vendor_id: Option<String>,
    pub supplier_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::agreement_line_item::Entity")]
    AgreementLineItems,
}

impl Related<super::agreement_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AgreementLineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    // ... (previous methods remain the same)

    pub fn add_line_item(
        &self,
        line_item: super::agreement_line_item::Model,
    ) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item.validate().map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    pub async fn calculate_total_value(&self, db: &DatabaseConnection) -> Result<Decimal, DbErr> {
        let line_items = self
            .find_related(super::agreement_line_item::Entity)
            .all(db)
            .await?;
        let total = line_items
            .iter()
            .fold(Decimal::ZERO, |acc, item| acc + item.total_price);
        Ok(total)
    }
}
