use chrono::{DateTime, Datelike, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ChangeType {
    #[sea_orm(string_value = "component_change")]
    ComponentChange,
    #[sea_orm(string_value = "process_change")]
    ProcessChange,
    #[sea_orm(string_value = "documentation")]
    Documentation,
    #[sea_orm(string_value = "safety")]
    Safety,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum EcoStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "review")]
    Review,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "released")]
    Released,
    #[sea_orm(string_value = "rejected")]
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum Priority {
    #[sea_orm(string_value = "low")]
    Low,
    #[sea_orm(string_value = "normal")]
    Normal,
    #[sea_orm(string_value = "high")]
    High,
    #[sea_orm(string_value = "critical")]
    Critical,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "engineering_change_orders")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub eco_number: String,
    pub title: String,
    pub description: Option<String>,
    pub reason: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub affected_bom_ids: Option<Vec<Uuid>>,
    #[sea_orm(column_type = "JsonBinary")]
    pub affected_product_ids: Option<Vec<Uuid>>,
    pub change_type: Option<ChangeType>,
    pub priority: Priority,
    pub status: EcoStatus,
    pub requested_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub effective_date: Option<NaiveDate>,
    pub implementation_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now();

        if insert {
            if let ActiveValue::NotSet = self.id {
                self.id = ActiveValue::Set(Uuid::new_v4());
            }

            if let ActiveValue::NotSet = self.created_at {
                self.created_at = ActiveValue::Set(now);
            }

            if let ActiveValue::NotSet = self.status {
                self.status = ActiveValue::Set(EcoStatus::Draft);
            }

            if let ActiveValue::NotSet = self.priority {
                self.priority = ActiveValue::Set(Priority::Normal);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Generate ECO number
    /// Format: ECO-{YEAR}{MONTH}-{SEQUENCE}
    pub fn generate_eco_number(sequence: u32) -> String {
        let now = Utc::now();
        format!("ECO-{:04}{:02}-{:05}", now.date_naive().year(), now.date_naive().month(), sequence)
    }

    /// Check if ECO can be released
    pub fn can_release(&self) -> bool {
        matches!(self.status, EcoStatus::Approved) && self.approved_by.is_some()
    }

    /// Approve ECO
    pub fn approve(&mut self, approved_by: Uuid) {
        self.status = EcoStatus::Approved;
        self.approved_by = Some(approved_by);
    }

    /// Release ECO
    pub fn release(&mut self) {
        if self.can_release() {
            self.status = EcoStatus::Released;
        }
    }
}
