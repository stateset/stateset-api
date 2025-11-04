/*!
 * Transaction Helper Utilities
 *
 * Provides convenient helpers for database transactions to ensure ACID guarantees
 */

use sea_orm::{DatabaseConnection, DatabaseTransaction, DbErr, TransactionError, TransactionTrait};
use std::future::Future;
use std::pin::Pin;

/// Type alias for boxed future used in transactions
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Execute a function within a database transaction
///
/// This helper ensures:
/// - Automatic rollback on error
/// - Automatic commit on success
/// - Proper error handling and conversion
///
/// # Example
///
/// ```rust,ignore
/// use crate::db::transaction::with_transaction;
///
/// let result = with_transaction(&db, |txn| {
///     Box::pin(async move {
///         // Create order
///         let order = OrderEntity::insert(order_data).exec(txn).await?;
///
///         // Create order items
///         for item in items {
///             OrderItemEntity::insert(item).exec(txn).await?;
///         }
///
///         Ok(order)
///     })
/// }).await?;
/// ```
pub async fn with_transaction<F, T, E>(
    db: &DatabaseConnection,
    f: F,
) -> Result<T, E>
where
    F: for<'a> FnOnce(&'a DatabaseTransaction) -> BoxFuture<'a, Result<T, E>>,
    E: From<DbErr>,
{
    db.transaction(|txn| {
        Box::pin(async move {
            f(txn).await.map_err(|e| DbErr::Custom(format!("{:?}", e)))
        })
    })
    .await
    .map_err(|e| match e {
        TransactionError::Connection(db_err) => E::from(db_err),
        TransactionError::Transaction(db_err) => E::from(db_err),
    })
}

/// Execute multiple operations in a transaction with automatic rollback
///
/// # Example
///
/// ```rust,ignore
/// transaction_scope(&db, |txn| async move {
///     // All operations here are in a transaction
///     let order = create_order(txn, data).await?;
///     reserve_inventory(txn, order.id).await?;
///     send_notification(txn, order.id).await?;
///     Ok(order)
/// }).await?;
/// ```
pub async fn transaction_scope<F, T, E>(
    db: &DatabaseConnection,
    f: impl FnOnce(&DatabaseTransaction) -> F,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
    E: From<DbErr>,
{
    db.transaction(|txn| {
        Box::pin(async move {
            f(txn).await.map_err(|e| DbErr::Custom(format!("{:?}", e)))
        })
    })
    .await
    .map_err(|e| match e {
        TransactionError::Connection(db_err) => E::from(db_err),
        TransactionError::Transaction(db_err) => E::from(db_err),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ServiceError;

    #[tokio::test]
    async fn test_transaction_commit() {
        // This would require a test database setup
        // Placeholder for actual tests
    }

    #[tokio::test]
    async fn test_transaction_rollback_on_error() {
        // This would require a test database setup
        // Placeholder for actual tests
    }
}
