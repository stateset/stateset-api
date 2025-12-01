use crate::{
    errors::ServiceError,
    models::promotion_entity::{Entity as Promotion, Model as PromotionModel, PromotionStatus, PromotionType},
};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::{debug, warn};

#[derive(Clone)]
pub struct PromotionService {
    db: DatabaseConnection,
}

impl PromotionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Find an active promotion by code
    pub async fn find_active_promotion(
        &self,
        promo_code: &str,
    ) -> Result<Option<PromotionModel>, ServiceError> {
        use crate::models::promotion_entity::Column;

        let now = Utc::now();

        let promotion = Promotion::find()
            .filter(Column::PromotionCode.eq(promo_code))
            .filter(Column::Status.eq(PromotionStatus::Active))
            .filter(Column::StartDate.lte(now))
            .filter(Column::EndDate.gte(now))
            .one(&self.db)
            .await
            .map_err(ServiceError::from)?;

        if let Some(ref promo) = promotion {
            // Check usage limit
            if let Some(limit) = promo.usage_limit {
                if promo.usage_count >= limit {
                    warn!("Promotion {} has reached usage limit", promo_code);
                    return Ok(None);
                }
            }
        }

        Ok(promotion)
    }

    /// Calculate discount amount for a subtotal
    pub fn calculate_discount(
        &self,
        promotion: &PromotionModel,
        subtotal_cents: i64,
    ) -> Result<i64, ServiceError> {
        let subtotal = Decimal::from(subtotal_cents) / Decimal::from(100);

        // Check minimum order amount
        if let Some(min_amount) = promotion.min_order_amount {
            if subtotal < min_amount {
                debug!(
                    "Subtotal {} is below minimum order amount {}",
                    subtotal, min_amount
                );
                return Ok(0);
            }
        }

        let discount = match promotion.promotion_type {
            PromotionType::Percentage => {
                let discount_rate = promotion.discount_value / Decimal::from(100);
                subtotal * discount_rate
            }
            PromotionType::FixedAmount => promotion.discount_value,
            PromotionType::FreeShipping => {
                // Free shipping discount - handled separately, return 0 for subtotal discount
                return Ok(0);
            }
            PromotionType::BuyOneGetOne => {
                // BOGO logic - this would need item-level information
                // For now, just apply a simple discount
                subtotal / Decimal::from(2)
            }
        };

        // Apply maximum discount cap if set
        let capped_discount = if let Some(max_discount) = promotion.max_discount_amount {
            discount.min(max_discount)
        } else {
            discount
        };

        // Convert back to cents and ensure non-negative
        use rust_decimal::prelude::ToPrimitive;
        let discount_cents = (capped_discount * Decimal::from(100))
            .to_i64()
            .unwrap_or(0)
            .max(0);

        // Ensure discount doesn't exceed subtotal
        Ok(discount_cents.min(subtotal_cents))
    }

    /// Check if promotion provides free shipping
    pub fn provides_free_shipping(&self, promotion: &PromotionModel) -> bool {
        matches!(promotion.promotion_type, PromotionType::FreeShipping)
    }

    /// Increment usage count for a promotion (call after successful order)
    pub async fn increment_usage_count(
        &self,
        promotion_id: uuid::Uuid,
    ) -> Result<(), ServiceError> {
        use crate::models::promotion_entity::ActiveModel;
        use sea_orm::{ActiveModelTrait, Set};

        let promotion = Promotion::find_by_id(promotion_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Promotion {} not found", promotion_id)))?;

        let current_usage_count = promotion.usage_count;
        let mut active_promotion: ActiveModel = promotion.into();
        active_promotion.usage_count = Set(current_usage_count + 1);
        active_promotion.updated_at = Set(Utc::now());
        active_promotion.update(&self.db).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_percentage_discount() {
        let promotion = PromotionModel {
            id: uuid::Uuid::new_v4(),
            name: "Test Promo".to_string(),
            description: None,
            promotion_code: "TEST10".to_string(),
            promotion_type: PromotionType::Percentage,
            discount_value: Decimal::from(10), // 10%
            min_order_amount: None,
            max_discount_amount: None,
            usage_limit: None,
            usage_count: 0,
            start_date: Utc::now(),
            end_date: Utc::now() + chrono::Duration::days(30),
            status: PromotionStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let service = PromotionService {
            db: DatabaseConnection::default(),
        };

        // $100 order with 10% discount = $10 discount
        let discount = service.calculate_discount(&promotion, 10000).unwrap();
        assert_eq!(discount, 1000); // $10 in cents
    }

    #[test]
    fn test_calculate_fixed_discount() {
        let promotion = PromotionModel {
            id: uuid::Uuid::new_v4(),
            name: "Test Promo".to_string(),
            description: None,
            promotion_code: "SAVE20".to_string(),
            promotion_type: PromotionType::FixedAmount,
            discount_value: Decimal::from(20), // $20
            min_order_amount: None,
            max_discount_amount: None,
            usage_limit: None,
            usage_count: 0,
            start_date: Utc::now(),
            end_date: Utc::now() + chrono::Duration::days(30),
            status: PromotionStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let service = PromotionService {
            db: DatabaseConnection::default(),
        };

        // $100 order with $20 discount
        let discount = service.calculate_discount(&promotion, 10000).unwrap();
        assert_eq!(discount, 2000); // $20 in cents
    }
}
