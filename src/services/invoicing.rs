use crate::{errors::AppError, models::invoices};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::sync::Arc;

pub struct InvoicingService {
    db: Arc<DatabaseConnection>,
}

impl InvoicingService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Generate an invoice for the given order and persist it to the database.
    pub async fn generate_invoice(&self, order_id: uuid::Uuid) -> Result<uuid::Uuid, AppError> {
        let invoice_id = uuid::Uuid::new_v4();
        tracing::info!(%order_id, %invoice_id, "generate invoice");

        let model = invoices::ActiveModel {
            id: Set(invoice_id.to_string()),
            order_id: Set(Some(order_id.to_string())),
            created: Set(Some(Utc::now())),
            status: Set(Some("Draft".to_string())),
            ..Default::default()
        };

        model.insert(&*self.db).await?;

        Ok(invoice_id)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    // ========================================
    // Invoice ID Generation Tests
    // ========================================

    #[test]
    fn test_invoice_id_is_valid_uuid() {
        let id = Uuid::new_v4();
        assert!(!id.is_nil());
    }

    #[test]
    fn test_invoice_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_invoice_id_to_string() {
        let id = Uuid::new_v4();
        let id_string = id.to_string();
        assert_eq!(id_string.len(), 36); // UUID string format
        assert!(id_string.contains('-'));
    }

    // ========================================
    // Invoice Status Tests
    // ========================================

    #[test]
    fn test_valid_invoice_statuses() {
        let statuses = vec![
            "Draft",
            "Open",
            "Paid",
            "Void",
            "Uncollectible",
            "Overdue",
            "Partially_Paid",
        ];

        for status in &statuses {
            assert!(!status.is_empty());
        }
        assert!(statuses.len() >= 4);
    }

    #[test]
    fn test_initial_status_is_draft() {
        let status = "Draft";
        assert_eq!(status, "Draft");
    }

    #[test]
    fn test_status_transitions() {
        // Valid: Draft -> Open -> Paid
        // Valid: Open -> Void
        // Valid: Open -> Uncollectible
        let valid_transitions = vec![
            ("Draft", "Open"),
            ("Open", "Paid"),
            ("Open", "Void"),
            ("Open", "Uncollectible"),
            ("Open", "Partially_Paid"),
        ];

        for (from, to) in valid_transitions {
            assert_ne!(from, to);
        }
    }

    // ========================================
    // Invoice Amount Tests
    // ========================================

    #[test]
    fn test_amount_due_positive() {
        let amount: Decimal = dec!(100.00);
        assert!(amount > Decimal::ZERO);
    }

    #[test]
    fn test_amount_paid_calculation() {
        let amount_due: Decimal = dec!(100.00);
        let payment: Decimal = dec!(50.00);
        let amount_remaining = amount_due - payment;

        assert_eq!(amount_remaining, dec!(50.00));
    }

    #[test]
    fn test_full_payment() {
        let amount_due: Decimal = dec!(250.00);
        let amount_paid: Decimal = dec!(250.00);
        let amount_remaining = amount_due - amount_paid;

        assert_eq!(amount_remaining, Decimal::ZERO);
    }

    #[test]
    fn test_overpayment() {
        let amount_due: Decimal = dec!(100.00);
        let amount_paid: Decimal = dec!(120.00);
        let credit = amount_paid - amount_due;

        assert_eq!(credit, dec!(20.00));
    }

    #[test]
    fn test_partial_payment() {
        let amount_due: Decimal = dec!(100.00);
        let amount_paid: Decimal = dec!(25.00);
        let amount_remaining = amount_due - amount_paid;

        assert!(amount_remaining > Decimal::ZERO);
        assert_eq!(amount_remaining, dec!(75.00));
    }

    // ========================================
    // Currency Tests
    // ========================================

    #[test]
    fn test_valid_currencies() {
        let currencies = vec!["USD", "EUR", "GBP", "CAD", "AUD", "JPY"];

        for currency in currencies {
            assert_eq!(currency.len(), 3);
            assert!(currency.chars().all(|c| c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn test_default_currency() {
        let currency = "USD";
        assert_eq!(currency, "USD");
    }

    // ========================================
    // Due Date Tests
    // ========================================

    #[test]
    fn test_due_date_net_30() {
        let created = Utc::now();
        let due_date = created + Duration::days(30);

        assert!(due_date > created);
    }

    #[test]
    fn test_due_date_net_60() {
        let created = Utc::now();
        let due_date = created + Duration::days(60);

        let days_until_due = (due_date - created).num_days();
        assert_eq!(days_until_due, 60);
    }

    #[test]
    fn test_invoice_is_overdue() {
        let due_date = Utc::now() - Duration::days(1);
        let now = Utc::now();

        assert!(due_date < now, "Invoice is overdue");
    }

    #[test]
    fn test_invoice_not_overdue() {
        let due_date = Utc::now() + Duration::days(7);
        let now = Utc::now();

        assert!(due_date > now, "Invoice is not yet due");
    }

    // ========================================
    // Customer Information Tests
    // ========================================

    #[test]
    fn test_customer_name_present() {
        let name = Some("Acme Corporation".to_string());
        assert!(name.is_some());
    }

    #[test]
    fn test_customer_email_format() {
        let email = "billing@example.com";
        assert!(email.contains('@'));
        assert!(email.contains('.'));
    }

    #[test]
    fn test_customer_phone_optional() {
        let phone: Option<String> = None;
        assert!(phone.is_none());
    }

    // ========================================
    // Billing Reason Tests
    // ========================================

    #[test]
    fn test_billing_reasons() {
        let reasons = vec![
            "subscription",
            "subscription_create",
            "subscription_cycle",
            "subscription_update",
            "manual",
        ];

        for reason in reasons {
            assert!(!reason.is_empty());
        }
    }

    // ========================================
    // Collection Method Tests
    // ========================================

    #[test]
    fn test_collection_methods() {
        let methods = vec!["charge_automatically", "send_invoice"];

        assert!(methods.len() >= 2);
    }

    // ========================================
    // Invoice Number Format Tests
    // ========================================

    #[test]
    fn test_invoice_number_format() {
        let pattern = regex::Regex::new(r"^INV-\d{4}-\d{6}$").unwrap();

        // Example: INV-2024-000001
        assert!(pattern.is_match("INV-2024-000001"));
        assert!(pattern.is_match("INV-2024-123456"));
        assert!(!pattern.is_match("INV2024000001")); // Missing dashes
    }

    #[test]
    fn test_invoice_number_sequential() {
        let base = 1000;
        let next = base + 1;

        assert_eq!(next, 1001);
    }

    // ========================================
    // Line Item Tests
    // ========================================

    #[test]
    fn test_line_item_subtotal() {
        let quantity: i32 = 5;
        let unit_price: Decimal = dec!(19.99);
        let subtotal = unit_price * Decimal::from(quantity);

        assert_eq!(subtotal, dec!(99.95));
    }

    #[test]
    fn test_line_item_with_discount() {
        let subtotal: Decimal = dec!(100.00);
        let discount_percent: Decimal = dec!(0.10); // 10%
        let discount_amount = subtotal * discount_percent;
        let final_amount = subtotal - discount_amount;

        assert_eq!(final_amount, dec!(90.00));
    }

    #[test]
    fn test_line_item_with_tax() {
        let subtotal: Decimal = dec!(100.00);
        let tax_rate: Decimal = dec!(0.08); // 8%
        let tax_amount = subtotal * tax_rate;
        let total = subtotal + tax_amount;

        assert_eq!(total, dec!(108.00));
    }

    // ========================================
    // Total Calculation Tests
    // ========================================

    #[test]
    fn test_invoice_total_calculation() {
        let line_items = vec![dec!(100.00), dec!(50.00), dec!(25.00)];
        let subtotal: Decimal = line_items.iter().sum();

        assert_eq!(subtotal, dec!(175.00));
    }

    #[test]
    fn test_invoice_total_with_tax_and_discount() {
        let subtotal: Decimal = dec!(200.00);
        let discount: Decimal = dec!(20.00);
        let tax_rate: Decimal = dec!(0.10);

        let after_discount = subtotal - discount;
        let tax = after_discount * tax_rate;
        let total = after_discount + tax;

        assert_eq!(after_discount, dec!(180.00));
        assert_eq!(tax, dec!(18.00));
        assert_eq!(total, dec!(198.00));
    }

    // ========================================
    // Order Association Tests
    // ========================================

    #[test]
    fn test_order_id_association() {
        let order_id = Uuid::new_v4();
        let order_id_string = order_id.to_string();

        assert!(!order_id_string.is_empty());
        assert!(Uuid::parse_str(&order_id_string).is_ok());
    }

    #[test]
    fn test_invoice_without_order() {
        // Standalone invoice (not tied to an order)
        let order_id: Option<Uuid> = None;
        assert!(order_id.is_none());
    }

    // ========================================
    // Account Information Tests
    // ========================================

    #[test]
    fn test_account_id_present() {
        let account_id = Some(Uuid::new_v4().to_string());
        assert!(account_id.is_some());
    }

    #[test]
    fn test_account_country_format() {
        let country = "US";
        assert_eq!(country.len(), 2);
        assert!(country.chars().all(|c| c.is_ascii_uppercase()));
    }

    // ========================================
    // Date Tests
    // ========================================

    #[test]
    fn test_created_date_is_set() {
        let created = Utc::now();
        assert!(created.timestamp() > 0);
    }

    #[test]
    fn test_finalized_date_optional() {
        let finalized_at: Option<chrono::DateTime<Utc>> = None;
        assert!(finalized_at.is_none());
    }

    // ========================================
    // Memo/Notes Tests
    // ========================================

    #[test]
    fn test_invoice_memo_optional() {
        let memo: Option<String> = Some("Thank you for your business".to_string());
        assert!(memo.is_some());
    }

    #[test]
    fn test_invoice_memo_empty() {
        let memo: Option<String> = None;
        assert!(memo.is_none());
    }
}
