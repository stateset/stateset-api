use crate::{
    errors::ServiceError,
    models::{stablepay_reconciliation, stablepay_transaction},
};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

/// External transaction data from provider statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTransaction {
    pub external_id: String,
    pub amount: Decimal,
    pub currency: String,
    pub date: chrono::DateTime<Utc>,
    pub status: String,
}

/// Reconciliation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationRequest {
    pub provider_id: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub external_transactions: Vec<ExternalTransaction>,
}

/// Reconciliation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationResult {
    pub id: Uuid,
    pub reconciliation_number: String,
    pub provider_id: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub total_transactions: i32,
    pub matched_transactions: i32,
    pub unmatched_transactions: i32,
    pub discrepancy_count: i32,
    pub discrepancy_amount: Decimal,
    pub match_rate: Decimal,
    pub status: String,
}

/// Matched transaction pair
#[derive(Debug, Clone)]
struct TransactionMatch {
    internal_id: Uuid,
    external_id: String,
    match_score: Decimal,
    amount_difference: Decimal,
}

/// StablePay Reconciliation Service - Auto-reconciliation of payments
pub struct StablePayReconciliationService {
    db: Arc<DatabaseConnection>,
}

impl StablePayReconciliationService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Run reconciliation for a period
    #[instrument(skip(self, request))]
    pub async fn reconcile(
        &self,
        request: ReconciliationRequest,
    ) -> Result<ReconciliationResult, ServiceError> {
        info!(
            provider_id = %request.provider_id,
            period_start = %request.period_start,
            period_end = %request.period_end,
            external_count = request.external_transactions.len(),
            "Starting reconciliation"
        );

        // Fetch internal transactions for the period
        let internal_transactions = self
            .fetch_internal_transactions(
                &request.provider_id,
                &request.period_start,
                &request.period_end,
            )
            .await?;

        info!(
            internal_count = internal_transactions.len(),
            "Fetched internal transactions"
        );

        // Match transactions
        let matches =
            self.match_transactions(&internal_transactions, &request.external_transactions);

        info!(matched_count = matches.len(), "Matched transactions");

        // Calculate summary statistics
        let total_internal = internal_transactions.len() as i32;
        let matched_count = matches.len() as i32;
        let unmatched_count = total_internal - matched_count;

        let (discrepancy_count, discrepancy_amount) = self.calculate_discrepancies(&matches);

        let total_amount: Decimal = internal_transactions.iter().map(|t| t.amount).sum();
        let total_fees: Decimal = internal_transactions.iter().map(|t| t.total_fees).sum();

        // Generate reconciliation number
        let reconciliation_number = self.generate_reconciliation_number().await?;

        // Determine status
        let status = if discrepancy_count > 0 || unmatched_count > 0 {
            "requires_review"
        } else {
            "completed"
        };

        // Create reconciliation record
        let reconciliation_id = Uuid::new_v4();
        let now = Utc::now();

        let reconciliation_model = stablepay_reconciliation::ActiveModel {
            id: Set(reconciliation_id),
            reconciliation_number: Set(reconciliation_number.clone()),
            period_start: Set(request.period_start),
            period_end: Set(request.period_end),
            provider_id: Set(request.provider_id),
            total_transactions: Set(total_internal),
            total_amount: Set(total_amount),
            total_fees: Set(total_fees),
            matched_transactions: Set(matched_count),
            unmatched_transactions: Set(unmatched_count),
            discrepancy_amount: Set(discrepancy_amount),
            discrepancy_count: Set(discrepancy_count),
            status: Set(status.to_string()),
            provider_statement_url: Set(None),
            reconciliation_report_url: Set(None),
            started_at: Set(Some(now)),
            completed_at: Set(Some(now)),
            metadata: Set(None),
            notes: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
            created_by: Set(None),
        };

        let reconciliation = reconciliation_model
            .insert(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        // Update matched transactions
        for transaction_match in matches {
            self.mark_transaction_reconciled(transaction_match.internal_id, reconciliation_id)
                .await?;
        }

        let match_rate = if total_internal > 0 {
            Decimal::from(matched_count) / Decimal::from(total_internal) * dec!(100)
        } else {
            Decimal::ZERO
        };

        info!(
            reconciliation_id = %reconciliation_id,
            matched = matched_count,
            unmatched = unmatched_count,
            discrepancies = discrepancy_count,
            match_rate = %match_rate,
            "Reconciliation completed"
        );

        Ok(ReconciliationResult {
            id: reconciliation_id,
            reconciliation_number,
            provider_id: request.provider_id,
            period_start: request.period_start,
            period_end: request.period_end,
            total_transactions: total_internal,
            matched_transactions: matched_count,
            unmatched_transactions: unmatched_count,
            discrepancy_count,
            discrepancy_amount,
            match_rate,
            status: status.to_string(),
        })
    }

    /// Get reconciliation by ID
    pub async fn get_reconciliation(&self, id: Uuid) -> Result<ReconciliationResult, ServiceError> {
        let reconciliation = stablepay_reconciliation::Entity::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound("Reconciliation not found".to_string()))?;

        let match_rate = if reconciliation.total_transactions > 0 {
            Decimal::from(reconciliation.matched_transactions)
                / Decimal::from(reconciliation.total_transactions)
                * dec!(100)
        } else {
            Decimal::ZERO
        };

        Ok(ReconciliationResult {
            id: reconciliation.id,
            reconciliation_number: reconciliation.reconciliation_number,
            provider_id: reconciliation.provider_id,
            period_start: reconciliation.period_start,
            period_end: reconciliation.period_end,
            total_transactions: reconciliation.total_transactions,
            matched_transactions: reconciliation.matched_transactions,
            unmatched_transactions: reconciliation.unmatched_transactions,
            discrepancy_count: reconciliation.discrepancy_count,
            discrepancy_amount: reconciliation.discrepancy_amount,
            match_rate,
            status: reconciliation.status,
        })
    }

    /// List reconciliations for a provider
    pub async fn list_reconciliations(
        &self,
        provider_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<ReconciliationResult>, ServiceError> {
        let reconciliations = stablepay_reconciliation::Entity::find()
            .filter(stablepay_reconciliation::Column::ProviderId.eq(provider_id))
            .order_by_desc(stablepay_reconciliation::Column::CreatedAt)
            .limit(Some(limit))
            .offset(Some(offset))
            .all(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        let mut results = Vec::new();
        for reconciliation in reconciliations {
            let match_rate = if reconciliation.total_transactions > 0 {
                Decimal::from(reconciliation.matched_transactions)
                    / Decimal::from(reconciliation.total_transactions)
                    * dec!(100)
            } else {
                Decimal::ZERO
            };

            results.push(ReconciliationResult {
                id: reconciliation.id,
                reconciliation_number: reconciliation.reconciliation_number,
                provider_id: reconciliation.provider_id,
                period_start: reconciliation.period_start,
                period_end: reconciliation.period_end,
                total_transactions: reconciliation.total_transactions,
                matched_transactions: reconciliation.matched_transactions,
                unmatched_transactions: reconciliation.unmatched_transactions,
                discrepancy_count: reconciliation.discrepancy_count,
                discrepancy_amount: reconciliation.discrepancy_amount,
                match_rate,
                status: reconciliation.status,
            });
        }

        Ok(results)
    }

    // Private helper methods

    async fn fetch_internal_transactions(
        &self,
        provider_id: &Uuid,
        period_start: &NaiveDate,
        period_end: &NaiveDate,
    ) -> Result<Vec<stablepay_transaction::Model>, ServiceError> {
        let start_datetime = period_start
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Utc)
            .unwrap();
        let end_datetime = period_end
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_local_timezone(Utc)
            .unwrap();

        stablepay_transaction::Entity::find()
            .filter(stablepay_transaction::Column::ProviderId.eq(*provider_id))
            .filter(stablepay_transaction::Column::ProcessedAt.gte(start_datetime))
            .filter(stablepay_transaction::Column::ProcessedAt.lte(end_datetime))
            .filter(stablepay_transaction::Column::Status.eq("succeeded"))
            .all(&*self.db)
            .await
            .map_err(ServiceError::db_error)
    }

    fn match_transactions(
        &self,
        internal: &[stablepay_transaction::Model],
        external: &[ExternalTransaction],
    ) -> Vec<TransactionMatch> {
        let mut matches = Vec::new();

        for internal_tx in internal {
            if let Some(external_tx) = self.find_best_match(internal_tx, external) {
                let match_score = self.calculate_match_score(internal_tx, &external_tx);

                // Only consider matches with score > 70%
                if match_score > dec!(70) {
                    let amount_difference = (internal_tx.amount - external_tx.amount).abs();

                    matches.push(TransactionMatch {
                        internal_id: internal_tx.id,
                        external_id: external_tx.external_id.clone(),
                        match_score,
                        amount_difference,
                    });
                }
            }
        }

        matches
    }

    fn find_best_match<'a>(
        &self,
        internal: &stablepay_transaction::Model,
        external: &'a [ExternalTransaction],
    ) -> Option<&'a ExternalTransaction> {
        let mut best_match: Option<(&ExternalTransaction, Decimal)> = None;

        for ext in external {
            let score = self.calculate_match_score(internal, ext);

            match best_match {
                None => best_match = Some((ext, score)),
                Some((_, best_score)) => {
                    if score > best_score {
                        best_match = Some((ext, score));
                    }
                }
            }
        }

        best_match.map(|(tx, _)| tx)
    }

    fn calculate_match_score(
        &self,
        internal: &stablepay_transaction::Model,
        external: &ExternalTransaction,
    ) -> Decimal {
        let mut score = Decimal::ZERO;

        // Exact amount match: 50 points
        if internal.amount == external.amount {
            score += dec!(50);
        } else {
            // Partial points for close amounts (within 1%)
            let diff_percentage =
                ((internal.amount - external.amount).abs() / internal.amount) * dec!(100);
            if diff_percentage < dec!(1) {
                score += dec!(40);
            } else if diff_percentage < dec!(5) {
                score += dec!(20);
            }
        }

        // Currency match: 20 points
        if internal.currency == external.currency {
            score += dec!(20);
        }

        // Date proximity: up to 30 points
        if let Some(processed_at) = internal.processed_at {
            let time_diff_hours = (processed_at - external.date).num_hours().abs();
            if time_diff_hours < 24 {
                score += dec!(30);
            } else if time_diff_hours < 72 {
                score += dec!(20);
            } else if time_diff_hours < 168 {
                score += dec!(10);
            }
        }

        score
    }

    fn calculate_discrepancies(&self, matches: &[TransactionMatch]) -> (i32, Decimal) {
        let mut discrepancy_count = 0;
        let mut discrepancy_amount = Decimal::ZERO;

        for transaction_match in matches {
            // Discrepancy if amount difference > $0.01
            if transaction_match.amount_difference > dec!(0.01) {
                discrepancy_count += 1;
                discrepancy_amount += transaction_match.amount_difference;
            }
        }

        (discrepancy_count, discrepancy_amount)
    }

    async fn mark_transaction_reconciled(
        &self,
        transaction_id: Uuid,
        reconciliation_id: Uuid,
    ) -> Result<(), ServiceError> {
        stablepay_transaction::ActiveModel {
            id: Set(transaction_id),
            is_reconciled: Set(true),
            reconciled_at: Set(Some(Utc::now())),
            reconciliation_id: Set(Some(reconciliation_id)),
            updated_at: Set(Some(Utc::now())),
            ..Default::default()
        }
        .update(&*self.db)
        .await
        .map_err(ServiceError::db_error)?;

        Ok(())
    }

    async fn generate_reconciliation_number(&self) -> Result<String, ServiceError> {
        let timestamp = Utc::now().format("%Y%m%d");
        let random = Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap()
            .to_uppercase();
        Ok(format!("REC-{}-{}", timestamp, random))
    }

    /// Get reconciliation statistics
    pub async fn get_reconciliation_stats(
        &self,
        provider_id: Uuid,
        days: i64,
    ) -> Result<ReconciliationStats, ServiceError> {
        let cutoff_date = (Utc::now() - chrono::Duration::days(days)).date_naive();

        let reconciliations = stablepay_reconciliation::Entity::find()
            .filter(stablepay_reconciliation::Column::ProviderId.eq(provider_id))
            .filter(stablepay_reconciliation::Column::PeriodStart.gte(cutoff_date))
            .all(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        let total_reconciliations = reconciliations.len() as i32;
        let total_transactions: i32 = reconciliations.iter().map(|r| r.total_transactions).sum();
        let total_matched: i32 = reconciliations.iter().map(|r| r.matched_transactions).sum();
        let total_discrepancies: i32 = reconciliations.iter().map(|r| r.discrepancy_count).sum();
        let total_discrepancy_amount: Decimal =
            reconciliations.iter().map(|r| r.discrepancy_amount).sum();

        let average_match_rate = if total_transactions > 0 {
            Decimal::from(total_matched) / Decimal::from(total_transactions) * dec!(100)
        } else {
            Decimal::ZERO
        };

        Ok(ReconciliationStats {
            total_reconciliations,
            total_transactions,
            total_matched,
            total_discrepancies,
            total_discrepancy_amount,
            average_match_rate,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationStats {
    pub total_reconciliations: i32,
    pub total_transactions: i32,
    pub total_matched: i32,
    pub total_discrepancies: i32,
    pub total_discrepancy_amount: Decimal,
    pub average_match_rate: Decimal,
}
