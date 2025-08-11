use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub account_name: String,
    pub account_type: Option<String>,
    pub industry: Option<String>,
    pub rating: Option<String>,
    pub phone: Option<String>,
    pub contact_id: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub is_public: Option<bool>,
    pub controller: Option<String>,
    pub processor: Option<String>,
    pub is_active: Option<bool>,
    pub user_id: Option<String>,
    pub order_id: Option<String>,
    pub account_notes: Option<String>,
    pub annual_revenue: Option<Decimal>,
    pub billing_city: Option<String>,
    pub billing_country: Option<String>,
    pub billing_latitude: Option<String>,
    pub billing_longitude: Option<String>,
    pub billing_state: Option<String>,
    pub billing_street: Option<String>,
    pub number_of_employees: Option<Decimal>,
    pub ownership: Option<String>,
    pub shipping_city: Option<String>,
    pub shipping_country: Option<String>,
    pub shipping_latitude: Option<String>,
    pub shipping_longitude: Option<String>,
    pub shipping_state: Option<String>,
    pub shipping_street: Option<String>,
    pub website: Option<String>,
    pub year_started: Option<String>,
    pub description: Option<String>,
    pub employees: Option<i32>,
    pub shop: Option<String>,
    pub access_token: Option<String>,
    pub account_subtype: Option<String>,
    pub item_id: Option<String>,
    pub institution_id: Option<String>,
    pub institution_name: Option<String>,
    pub avatar: Option<String>,
    pub stock_ticker: Option<String>,
    pub account_owner: Option<String>,
    pub account_source: Option<String>,
    pub fax: Option<i32>,
    pub last_modified_by: Option<String>,
    pub parent_account: Option<String>,
    pub partner_account: Option<bool>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    // Define relations here if needed
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(id: String, account_name: String) -> Self {
        Self {
            id,
            account_name,
            account_type: None,
            industry: None,
            rating: None,
            phone: None,
            contact_id: None,
            created_at: Some(Utc::now()),
            is_public: None,
            controller: None,
            processor: None,
            is_active: Some(true),
            user_id: None,
            order_id: None,
            account_notes: None,
            annual_revenue: None,
            billing_city: None,
            billing_country: None,
            billing_latitude: None,
            billing_longitude: None,
            billing_state: None,
            billing_street: None,
            number_of_employees: None,
            ownership: None,
            shipping_city: None,
            shipping_country: None,
            shipping_latitude: None,
            shipping_longitude: None,
            shipping_state: None,
            shipping_street: None,
            website: None,
            year_started: None,
            description: None,
            employees: None,
            shop: None,
            access_token: None,
            account_subtype: None,
            item_id: None,
            institution_id: None,
            institution_name: None,
            avatar: None,
            stock_ticker: None,
            account_owner: None,
            account_source: None,
            fax: None,
            last_modified_by: None,
            parent_account: None,
            partner_account: None,
        }
    }

    pub fn set_billing_address(
        &mut self,
        street: String,
        city: String,
        state: String,
        country: String,
    ) {
        self.billing_street = Some(street);
        self.billing_city = Some(city);
        self.billing_state = Some(state);
        self.billing_country = Some(country);
    }

    pub fn set_shipping_address(
        &mut self,
        street: String,
        city: String,
        state: String,
        country: String,
    ) {
        self.shipping_street = Some(street);
        self.shipping_city = Some(city);
        self.shipping_state = Some(state);
        self.shipping_country = Some(country);
    }

    pub fn update_revenue(&mut self, revenue: Decimal) {
        self.annual_revenue = Some(revenue);
    }

    pub fn deactivate(&mut self) {
        self.is_active = Some(false);
    }

    pub fn activate(&mut self) {
        self.is_active = Some(true);
    }
}
