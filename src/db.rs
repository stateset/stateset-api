use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub fn establish_connection(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.")
}