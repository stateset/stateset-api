use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Select, PaginatorTrait, ConnectionTrait,
};
use std::fmt::Debug;

/// Helper struct for building optimized queries
pub struct QueryBuilder<E: EntityTrait> {
    query: Select<E>,
    page: u64,
    limit: u64,
}

impl<E: EntityTrait> QueryBuilder<E> {
    /// Create a new query builder
    pub fn new() -> Self {
        Self {
            query: E::find(),
            page: 1,
            limit: 20,
        }
    }
    
    /// Add pagination
    pub fn paginate(mut self, page: u64, limit: u64) -> Self {
        self.page = page.max(1);
        self.limit = limit.min(100); // Cap at 100 to prevent abuse
        self
    }
    
    /// Add a filter condition
    pub fn filter<C>(mut self, condition: C) -> Self
    where
        C: Into<Condition>,
    {
        self.query = self.query.filter(condition);
        self
    }
    
    /// Add ordering
    pub fn order_by<C>(mut self, column: C, desc: bool) -> Self
    where
        C: ColumnTrait,
    {
        self.query = if desc {
            self.query.order_by_desc(column)
        } else {
            self.query.order_by_asc(column)
        };
        self
    }
    
    /// Limit columns selected (projection)
    pub fn select_columns<C, I>(mut self, columns: I) -> Self
    where
        C: ColumnTrait,
        I: IntoIterator<Item = C>,
    {
        for column in columns {
            self.query = self.query.column(column);
        }
        self
    }
    
    /// Execute the query and return paginated results
    pub async fn execute(self, db: &DatabaseConnection) -> Result<(Vec<E::Model>, u64), sea_orm::DbErr>
    where
        E::Model: Send + Sync,
    {
        let paginator = self.query.paginate(db, self.limit);
        let total = paginator.num_items().await?;
        let items = paginator.fetch_page(self.page - 1).await?;
        
        Ok((items, total))
    }
    
    /// Execute and return only the count
    pub async fn count(self, db: &DatabaseConnection) -> Result<u64, sea_orm::DbErr>
    {
        let paginator = self.query.paginate(db, 1);
        paginator.num_items().await
    }
}

/// Trait for adding query optimization hints
pub trait QueryOptimization {
    /// Add index hint for query optimization
    fn with_index_hint(self, index_name: &str) -> Self;
    
    /// Set query timeout
    fn with_timeout(self, seconds: u32) -> Self;
}

/// Helper for building complex search conditions
pub struct SearchBuilder {
    conditions: Vec<Condition>,
}

impl SearchBuilder {
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
        }
    }
    
    /// Add a LIKE condition for text search
    pub fn add_like<C: ColumnTrait>(mut self, column: C, pattern: &str) -> Self {
        if !pattern.is_empty() {
            self.conditions.push(
                Condition::all().add(column.contains(pattern))
            );
        }
        self
    }
    
    /// Add an exact match condition
    pub fn add_eq<C: ColumnTrait, V>(mut self, column: C, value: V) -> Self
    where
        V: Into<sea_orm::Value>,
    {
        self.conditions.push(
            Condition::all().add(column.eq(value))
        );
        self
    }
    
    /// Add a range condition
    pub fn add_between<C: ColumnTrait, V>(mut self, column: C, min: V, max: V) -> Self
    where
        V: Into<sea_orm::Value>,
    {
        self.conditions.push(
            Condition::all()
                .add(column.gte(min))
                .add(column.lte(max))
        );
        self
    }
    
    /// Build the final condition
    pub fn build(self) -> Option<Condition> {
        if self.conditions.is_empty() {
            None
        } else {
            Some(
                self.conditions
                    .into_iter()
                    .fold(Condition::any(), |acc, cond| acc.add(cond))
            )
        }
    }
}

/// Macro for building queries with automatic error handling
#[macro_export]
macro_rules! query_with_retry {
    ($entity:ty, $db:expr, $builder:expr) => {{
        use $crate::middleware::retry::{with_retry, RetryConfig, DbRetryPolicy};
        
        with_retry(
            &RetryConfig::default(),
            DbRetryPolicy,
            || async {
                $builder.execute($db).await
            }
        ).await
    }};
}