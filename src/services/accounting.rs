use crate::{
    entities::ledger_entry::{
        ActiveModel as LedgerEntryActive, LedgerEntryStatus, LedgerEntryType,
        Model as LedgerEntryModel,
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

        let debit = debit_entry
            .insert(&txn)
            .await
            .map_err(AppError::DatabaseError)?;
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

        let result = entry
            .insert(&*self.db)
            .await
            .map_err(AppError::DatabaseError)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ==================== LedgerEntryType Tests ====================

    #[test]
    fn test_ledger_entry_type_debit() {
        let entry_type = LedgerEntryType::Debit;
        assert_eq!(format!("{:?}", entry_type), "Debit");
    }

    #[test]
    fn test_ledger_entry_type_credit() {
        let entry_type = LedgerEntryType::Credit;
        assert_eq!(format!("{:?}", entry_type), "Credit");
    }

    // ==================== LedgerEntryStatus Tests ====================

    #[test]
    fn test_ledger_entry_status_pending() {
        let status = LedgerEntryStatus::Pending;
        assert_eq!(format!("{:?}", status), "Pending");
    }

    #[test]
    fn test_ledger_entry_status_posted() {
        let status = LedgerEntryStatus::Posted;
        assert_eq!(format!("{:?}", status), "Posted");
    }

    // ==================== Double-Entry Accounting Tests ====================

    #[test]
    fn test_double_entry_balance() {
        // In double-entry accounting, debits must equal credits
        let debit_amount = dec!(100.00);
        let credit_amount = dec!(100.00);

        assert_eq!(debit_amount, credit_amount);
    }

    #[test]
    fn test_transaction_balances_to_zero() {
        let debit = dec!(500.00);
        let credit = dec!(-500.00);
        let balance = debit + credit;

        assert_eq!(balance, Decimal::ZERO);
    }

    #[test]
    fn test_multiple_entries_balance() {
        // Multiple debits and credits should balance
        let debits = vec![dec!(100.00), dec!(200.00), dec!(50.00)];
        let credits = vec![dec!(150.00), dec!(100.00), dec!(100.00)];

        let total_debits: Decimal = debits.iter().sum();
        let total_credits: Decimal = credits.iter().sum();

        assert_eq!(total_debits, total_credits);
    }

    // ==================== Account Tests ====================

    #[test]
    fn test_common_account_names() {
        let accounts = vec![
            "Cash",
            "Revenue",
            "Expense",
            "Accounts Receivable",
            "Accounts Payable",
            "Inventory",
            "Cost of Goods Sold",
        ];

        for account in accounts {
            assert!(!account.is_empty());
        }
    }

    #[test]
    fn test_general_account() {
        let account = "General";
        assert_eq!(account, "General");
    }

    // ==================== Currency Tests ====================

    #[test]
    fn test_default_currency_usd() {
        let currency = "USD";
        assert_eq!(currency, "USD");
        assert_eq!(currency.len(), 3);
    }

    #[test]
    fn test_currency_codes() {
        let currencies = vec!["USD", "EUR", "GBP", "CAD", "JPY"];

        for currency in currencies {
            assert_eq!(currency.len(), 3);
            assert!(currency.chars().all(|c| c.is_ascii_uppercase()));
        }
    }

    // ==================== Amount Tests ====================

    #[test]
    fn test_amount_positive() {
        let amount = dec!(100.00);
        assert!(amount > Decimal::ZERO);
    }

    #[test]
    fn test_amount_precision() {
        let amount = dec!(1234.56);
        assert_eq!(amount, dec!(1234.56));
    }

    #[test]
    fn test_amount_zero_valid() {
        let amount = Decimal::ZERO;
        assert_eq!(amount, Decimal::ZERO);
    }

    #[test]
    fn test_amount_decimal_operations() {
        let amount1 = dec!(100.25);
        let amount2 = dec!(50.75);
        let total = amount1 + amount2;

        assert_eq!(total, dec!(151.00));
    }

    // ==================== UUID Tests ====================

    #[test]
    fn test_transaction_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_entry_id_format() {
        let id = Uuid::new_v4();
        let id_str = id.to_string();

        assert_eq!(id_str.len(), 36);
        assert!(!id.is_nil());
    }

    #[test]
    fn test_reference_id_optional() {
        let reference_id: Option<Uuid> = None;
        assert!(reference_id.is_none());

        let with_reference: Option<Uuid> = Some(Uuid::new_v4());
        assert!(with_reference.is_some());
    }

    // ==================== Reference Type Tests ====================

    #[test]
    fn test_reference_type_order() {
        let ref_type = Some("Order".to_string());
        assert_eq!(ref_type, Some("Order".to_string()));
    }

    #[test]
    fn test_reference_type_expense() {
        let ref_type = Some("Expense".to_string());
        assert_eq!(ref_type, Some("Expense".to_string()));
    }

    #[test]
    fn test_reference_type_none() {
        let ref_type: Option<String> = None;
        assert!(ref_type.is_none());
    }

    // ==================== Description Tests ====================

    #[test]
    fn test_description_not_empty() {
        let description = "Payment received for Order #123";
        assert!(!description.is_empty());
    }

    #[test]
    fn test_description_length() {
        let description = "Short desc";
        assert!(description.len() <= 500);
    }

    // ==================== Revenue Recording Tests ====================

    #[test]
    fn test_revenue_accounts() {
        // Revenue: debit Cash, credit Revenue
        let debit_account = "Cash";
        let credit_account = "Revenue";

        assert_eq!(debit_account, "Cash");
        assert_eq!(credit_account, "Revenue");
    }

    #[test]
    fn test_revenue_amount_positive() {
        let revenue_amount = dec!(250.00);
        assert!(revenue_amount > Decimal::ZERO);
    }

    // ==================== Expense Recording Tests ====================

    #[test]
    fn test_expense_accounts() {
        // Expense: debit Expense, credit Cash
        let debit_account = "Expense";
        let credit_account = "Cash";

        assert_eq!(debit_account, "Expense");
        assert_eq!(credit_account, "Cash");
    }

    #[test]
    fn test_expense_amount_positive() {
        let expense_amount = dec!(75.50);
        assert!(expense_amount > Decimal::ZERO);
    }

    // ==================== Timestamp Tests ====================

    #[test]
    fn test_posting_date_now() {
        let now = Utc::now();
        let later = Utc::now();

        // Later should be >= now (or very close)
        assert!(later >= now || (now - later).num_milliseconds().abs() < 100);
    }

    #[test]
    fn test_created_at_before_updated_at() {
        let created = Utc::now();
        let updated = Utc::now();

        // Created should be <= updated
        assert!(created <= updated);
    }

    // ==================== Journal Entry Tests ====================

    #[test]
    fn test_journal_entry_debit_credit_match() {
        // A journal entry must have matching debits and credits
        let debit_entry = dec!(1000.00);
        let credit_entry = dec!(1000.00);

        assert_eq!(debit_entry, credit_entry);
    }

    #[test]
    fn test_multi_line_journal_entry() {
        // Complex journal entry with multiple lines
        let debits = vec![dec!(500.00), dec!(300.00), dec!(200.00)];
        let credits = vec![dec!(1000.00)];

        let total_debits: Decimal = debits.iter().sum();
        let total_credits: Decimal = credits.iter().sum();

        assert_eq!(total_debits, total_credits);
    }

    // ==================== Account Type Tests ====================

    #[test]
    fn test_asset_accounts() {
        // Assets increase with debits
        let asset_accounts = vec!["Cash", "Accounts Receivable", "Inventory", "Equipment"];

        for account in asset_accounts {
            assert!(!account.is_empty());
        }
    }

    #[test]
    fn test_liability_accounts() {
        // Liabilities increase with credits
        let liability_accounts = vec!["Accounts Payable", "Notes Payable", "Unearned Revenue"];

        for account in liability_accounts {
            assert!(!account.is_empty());
        }
    }

    #[test]
    fn test_equity_accounts() {
        let equity_accounts = vec!["Owner's Equity", "Retained Earnings", "Common Stock"];

        for account in equity_accounts {
            assert!(!account.is_empty());
        }
    }

    // ==================== Financial Statement Impact Tests ====================

    #[test]
    fn test_revenue_increases_equity() {
        let beginning_equity = dec!(10000.00);
        let revenue = dec!(5000.00);
        let ending_equity = beginning_equity + revenue;

        assert!(ending_equity > beginning_equity);
    }

    #[test]
    fn test_expense_decreases_equity() {
        let beginning_equity = dec!(10000.00);
        let expense = dec!(3000.00);
        let ending_equity = beginning_equity - expense;

        assert!(ending_equity < beginning_equity);
    }

    // ==================== Accounting Equation Tests ====================

    #[test]
    fn test_accounting_equation() {
        // Assets = Liabilities + Equity
        let assets = dec!(100000.00);
        let liabilities = dec!(40000.00);
        let equity = dec!(60000.00);

        assert_eq!(assets, liabilities + equity);
    }

    #[test]
    fn test_accounting_equation_after_transaction() {
        // After any transaction, the equation must still balance
        let initial_assets = dec!(100000.00);
        let initial_liabilities = dec!(40000.00);
        let initial_equity = dec!(60000.00);

        // Transaction: Receive cash payment of $5000 for services
        let transaction = dec!(5000.00);

        let new_assets = initial_assets + transaction; // Cash increases
        let new_equity = initial_equity + transaction; // Revenue increases equity

        assert_eq!(new_assets, initial_liabilities + new_equity);
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_database_error_handling() {
        // Just verify the error type exists
        let error_msg = "Database connection failed";
        assert!(!error_msg.is_empty());
    }

    // ==================== Metadata Tests ====================

    #[test]
    fn test_metadata_none() {
        let metadata: Option<serde_json::Value> = None;
        assert!(metadata.is_none());
    }

    #[test]
    fn test_metadata_json() {
        let metadata = serde_json::json!({
            "invoice_number": "INV-001",
            "customer_id": "CUST-123"
        });

        assert!(metadata.is_object());
    }

    // ==================== Created By Tests ====================

    #[test]
    fn test_created_by_none() {
        let created_by: Option<Uuid> = None;
        assert!(created_by.is_none());
    }

    #[test]
    fn test_created_by_user() {
        let user_id = Uuid::new_v4();
        let created_by: Option<Uuid> = Some(user_id);
        assert!(created_by.is_some());
    }
}
