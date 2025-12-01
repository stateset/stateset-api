use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum CertificationType {
    #[sea_orm(string_value = "CE")]
    CE,
    #[sea_orm(string_value = "UL")]
    UL,
    #[sea_orm(string_value = "ISO")]
    ISO,
    #[sea_orm(string_value = "RIA")]
    RIA,
    #[sea_orm(string_value = "CSA")]
    CSA,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum CertStatus {
    #[sea_orm(string_value = "valid")]
    Valid,
    #[sea_orm(string_value = "expired")]
    Expired,
    #[sea_orm(string_value = "pending_renewal")]
    PendingRenewal,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "robot_certifications")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub robot_serial_id: Uuid,
    pub certification_type: CertificationType,
    pub certification_number: Option<String>,
    pub issuing_authority: Option<String>,
    pub issue_date: NaiveDate,
    pub expiration_date: Option<NaiveDate>,
    pub certification_scope: Option<String>,
    pub certificate_document_url: Option<String>,
    pub status: CertStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::robot_serial_number::Entity",
        from = "Column::RobotSerialId",
        to = "super::robot_serial_number::Column::Id"
    )]
    Robot,
}

impl Related<super::robot_serial_number::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Robot.def()
    }
}

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
                self.status = ActiveValue::Set(CertStatus::Valid);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Check if certification is valid
    pub fn is_valid(&self) -> bool {
        if let Some(exp_date) = self.expiration_date {
            let today = Utc::now().date_naive();
            today <= exp_date && matches!(self.status, CertStatus::Valid)
        } else {
            matches!(self.status, CertStatus::Valid)
        }
    }

    /// Get days until expiration
    pub fn days_until_expiration(&self) -> Option<i64> {
        self.expiration_date.map(|exp_date| {
            let today = Utc::now().date_naive();
            (exp_date - today).num_days()
        })
    }

    /// Check if renewal is needed soon (within 90 days)
    pub fn needs_renewal(&self) -> bool {
        if let Some(days) = self.days_until_expiration() {
            days <= 90 && days > 0
        } else {
            false
        }
    }
}
