use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "contacts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub controller: Option<String>,
    pub processor: Option<String>,
    pub account_id: Option<String>,
    pub user_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub birthdate: Option<String>,
    pub department: Option<String>,
    pub lead_source: Option<String>,
    pub mailing_city: Option<String>,
    pub mailing_country: Option<String>,
    pub mailing_geocode_accuracy: Option<String>,
    pub mailing_state: Option<String>,
    pub mailing_street: Option<String>,
    pub photo_url: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub next_meeting: Option<NaiveDate>,
    pub avatar: Option<String>,
    pub handle: Option<String>,
    pub assistant: Option<String>,
    pub assistant_phone: Option<String>,
    pub owner: Option<String>,
    pub created_by: Option<String>,
    pub do_not_call: Option<bool>,
    pub email_opt_out: Option<bool>,
    pub fax: Option<String>,
    pub fax_opt_out: Option<bool>,
    pub languages: Option<String>,
    pub invalid_contact: Option<bool>,
    pub last_modified_by: Option<String>,
    pub vendor_id: Option<String>,
    pub supplier_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::account::Entity",
        from = "Column::AccountId",
        to = "super::account::Column::Id"
    )]
    Account,
}

impl Related<super::account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(id: String, first_name: String, last_name: String) -> Self {
        Self {
            id,
            first_name,
            last_name,
            phone: None,
            email: None,
            controller: None,
            processor: None,
            account_id: None,
            user_id: None,
            title: None,
            description: None,
            birthdate: None,
            department: None,
            lead_source: None,
            mailing_city: None,
            mailing_country: None,
            mailing_geocode_accuracy: None,
            mailing_state: None,
            mailing_street: None,
            photo_url: None,
            created_at: Some(Utc::now()),
            next_meeting: None,
            avatar: None,
            handle: None,
            assistant: None,
            assistant_phone: None,
            owner: None,
            created_by: None,
            do_not_call: None,
            email_opt_out: None,
            fax: None,
            fax_opt_out: None,
            languages: None,
            invalid_contact: None,
            last_modified_by: None,
            vendor_id: None,
            supplier_id: None,
        }
    }

    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    pub fn set_mailing_address(
        &mut self,
        street: String,
        city: String,
        state: String,
        country: String,
    ) {
        self.mailing_street = Some(street);
        self.mailing_city = Some(city);
        self.mailing_state = Some(state);
        self.mailing_country = Some(country);
    }

    pub fn set_communication_preferences(
        &mut self,
        do_not_call: bool,
        email_opt_out: bool,
        fax_opt_out: bool,
    ) {
        self.do_not_call = Some(do_not_call);
        self.email_opt_out = Some(email_opt_out);
        self.fax_opt_out = Some(fax_opt_out);
    }

    pub fn set_next_meeting(&mut self, date: NaiveDate) {
        self.next_meeting = Some(date);
    }

    pub fn is_valid(&self) -> bool {
        self.invalid_contact.unwrap_or(false) == false
    }

    pub fn set_languages(&mut self, languages: &[&str]) {
        self.languages = Some(languages.join(", "));
    }
}
