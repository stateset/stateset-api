use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::NaiveDate;
use rust_decimal::Decimal;

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

// Agreement Line Item Model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "agreement_line_items")]
pub struct AgreementLineItem {
    #[sea_orm(primary_key)]
    pub id: String,
    pub agreement_id: String,
    pub item_name: String,
    pub item_description: Option<String>,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub total_price: Decimal,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub status: Option<String>,
    pub product_id: Option<String>,
    pub service_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum AgreementLineItemRelation {
    #[sea_orm(
        belongs_to = "super::agreement::Entity",
        from = "Column::AgreementId",
        to = "super::agreement::Column::Id"
    )]
    Agreement,
}

impl Related<super::agreement::Entity> for AgreementLineItem {
    fn to() -> RelationDef {
        AgreementLineItemRelation::Agreement.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    // ... (previous methods remain the same)

    pub fn add_line_item(&self, line_item: AgreementLineItem) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item.validate()?;
        Ok(())
    }

    pub fn calculate_total_value(&self, db: &DatabaseConnection) -> Result<Decimal, DbErr> {
        let line_items = self.find_related(super::agreement_line_item::Entity).all(db).await?;
        let total = line_items.iter().fold(Decimal::ZERO, |acc, item| acc + item.total_price);
        Ok(total)
    }
}

impl AgreementLineItem {
    pub fn new(
        agreement_id: String,
        item_name: String,
        quantity: Decimal,
        unit_price: Decimal,
    ) -> Self {
        let total_price = quantity * unit_price;
        Self {
            id: Uuid::new_v4().to_string(),
            agreement_id,
            item_name,
            item_description: None,
            quantity,
            unit_price,
            total_price,
            start_date: None,
            end_date: None,
            status: Some("Active".to_string()),
            product_id: None,
            service_id: None,
        }
    }

    pub fn set_dates(&mut self, start_date: NaiveDate, end_date: NaiveDate) {
        self.start_date = Some(start_date);
        self.end_date = Some(end_date);
    }

    pub fn update_quantity(&mut self, new_quantity: Decimal) {
        self.quantity = new_quantity;
        self.total_price = self.quantity * self.unit_price;
    }

    pub fn update_unit_price(&mut self, new_unit_price: Decimal) {
        self.unit_price = new_unit_price;
        self.total_price = self.quantity * self.unit_price;
    }

    pub fn is_active(&self) -> bool {
        match (self.start_date, self.end_date) {
            (Some(start), Some(end)) => {
                let today = chrono::Local::now().naive_local().date();
                today >= start && today <= end
            }
            _ => self.status.as_deref() == Some("Active"),
        }
    }
}