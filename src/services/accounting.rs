use crate::{
    entities::ledger_entry::{
        ActiveModel as LedgerEntryActive, Entity as LedgerEntry, LedgerEntryStatus,
        LedgerEntryType, Model as LedgerEntryModel,
    },
    errors::AppError,
};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, TransactionTrait};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

pub struct AccountingService {
    db: Arc<DatabaseConnection>,
}

impl AccountingService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Record a simple transaction in the accounting ledger (single entry).
    /// This creates a pending ledger entry that needs to be part of a balanced transaction.
    ///
    /// # Arguments
    /// * `description` - Description of the transaction
    /// * `amount` - Transaction amount
    #[instrument(skip(self))]
    pub async fn record_transaction(
        &self,
        description: &str,
        amount: Decimal,
    ) -> Result<LedgerEntryModel, AppError> {
        let transaction_id = Uuid::new_v4();
        let entry = self
            .create_ledger_entry(
                transaction_id,
                "General",
                LedgerEntryType::Debit,
                amount,
                "USD",
                description,
                None,
                None,
            )
            .await?;

        info!(
            transaction_id = %transaction_id,
            description,
            %amount,
            "Recorded accounting transaction"
        );

        Ok(entry)
    }

    /// Create a balanced double-entry transaction (debit and credit).
    ///
    /// This ensures proper accounting by creating matching debit and credit entries
    /// that balance to zero.
    ///
    /// # Arguments
    /// * `debit_account` - Account to debit
    /// * `credit_account` - Account to credit
    /// * `amount` - Transaction amount
    /// * `description` - Transaction description
    /// * `reference_id` - Optional reference to related entity (e.g., order_id)
    /// * `reference_type` - Optional reference type (e.g., "Order", "Invoice")
    #[instrument(skip(self))]
    pub async fn record_double_entry(
        &self,
        debit_account: &str,
        credit_account: &str,
        amount: Decimal,
        description: &str,
        reference_id: Option<Uuid>,
        reference_type: Option<String>,
    ) -> Result<(LedgerEntryModel, LedgerEntryModel), AppError> {
        let txn = self.db.begin().await.map_err(AppError::DatabaseError)?;

        let transaction_id = Uuid::new_v4();
        let now = Utc::now();

        // Create debit entry
        let debit_entry = LedgerEntryActive {
            id: Set(Uuid::new_v4()),
            transaction_id: Set(transaction_id),
            account: Set(debit_account.to_string()),
            entry_type: Set(LedgerEntryType::Debit),
            amount: Set(amount),
            currency: Set("USD".to_string()),
            description: Set(description.to_string()),
            reference_id: Set(reference_id),
            reference_type: Set(reference_type.clone()),
            status: Set(LedgerEntryStatus::Posted),
            posting_date: Set(now),
            metadata: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            created_by: Set(None),
        };

        // Create credit entry
        let credit_entry = LedgerEntryActive {
            id: Set(Uuid::new_v4()),
            transaction_id: Set(transaction_id),
            account: Set(credit_account.to_string()),
            entry_type: Set(LedgerEntryType::Credit),
            amount: Set(amount),
            currency: Set("USD".to_string()),
            description: Set(description.to_string()),
            reference_id: Set(reference_id),
            reference_type: Set(reference_type),
            status: Set(LedgerEntryStatus::Posted),
            posting_date: Set(now),
            metadata: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            created_by: Set(None),
        };

        let debit = debit_entry.insert(&txn).await.map_err(AppError::DatabaseError)?;
        let credit = credit_entry
            .insert(&txn)
            .await
            .map_err(AppError::DatabaseError)?;

        txn.commit().await.map_err(AppError::DatabaseError)?;

        info!(
            transaction_id = %transaction_id,
            debit_account,
            credit_account,
            %amount,
            description,
            "Recorded double-entry transaction"
        );

        Ok((debit, credit))
    }

    /// Create a ledger entry
    #[instrument(skip(self))]
    pub async fn create_ledger_entry(
        &self,
        transaction_id: Uuid,
        account: &str,
        entry_type: LedgerEntryType,
        amount: Decimal,
        currency: &str,
        description: &str,
        reference_id: Option<Uuid>,
        reference_type: Option<String>,
    ) -> Result<LedgerEntryModel, AppError> {
        let entry_type_for_log = format!("{:?}", entry_type);
        let entry = LedgerEntryActive {
            id: Set(Uuid::new_v4()),
            transaction_id: Set(transaction_id),
            account: Set(account.to_string()),
            entry_type: Set(entry_type),
            amount: Set(amount),
            currency: Set(currency.to_string()),
            description: Set(description.to_string()),
            reference_id: Set(reference_id),
            reference_type: Set(reference_type),
            status: Set(LedgerEntryStatus::Pending),
            posting_date: Set(Utc::now()),
            metadata: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            created_by: Set(None),
        };

        let result = entry.insert(&*self.db).await.map_err(AppError::DatabaseError)?;

        info!(
            entry_id = %result.id,
            transaction_id = %transaction_id,
            account,
            entry_type = %entry_type_for_log,
            %amount,
            "Created ledger entry"
        );

        Ok(result)
    }

    /// Record a revenue transaction (debit Cash, credit Revenue)
    pub async fn record_revenue(
        &self,
        amount: Decimal,
        description: &str,
        reference_id: Option<Uuid>,
    ) -> Result<(LedgerEntryModel, LedgerEntryModel), AppError> {
        self.record_double_entry(
            "Cash",
            "Revenue",
            amount,
            description,
            reference_id,
            Some("Order".to_string()),
        )
        .await
    }

    /// Record an expense transaction (debit Expense, credit Cash)
    pub async fn record_expense(
        &self,
        amount: Decimal,
        description: &str,
        reference_id: Option<Uuid>,
    ) -> Result<(LedgerEntryModel, LedgerEntryModel), AppError> {
        self.record_double_entry(
            "Expense",
            "Cash",
            amount,
            description,
            reference_id,
            Some("Expense".to_string()),
        )
        .await
    }
}
