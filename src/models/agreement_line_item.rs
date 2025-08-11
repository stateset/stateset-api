use chrono::NaiveDate;
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

// Agreement Line Item Model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "agreement_line_items")]
pub struct Model {
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
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::agreements::Entity",
        from = "Column::AgreementId",
        to = "super::agreements::Column::Id"
    )]
    Agreement,
}

impl Related<super::agreements::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Agreement.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
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
