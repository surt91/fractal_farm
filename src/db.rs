use std::env;

use diesel::sqlite::SqliteConnection;
use diesel::r2d2::ConnectionManager;

use std::ops::{Deref, DerefMut};
use axum::extract::{FromRef, FromRequestParts};
use axum::http::{request::Parts, StatusCode};

// An alias to the type for a pool of Diesel SQLite connections.
pub type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

/// Initializes a database pool.
pub fn init_pool() -> Pool {
    let db_url: &str = &env::var("DATABASE_URL")
                        .expect("DATABASE_URL must be set");

    let manager = ConnectionManager::<SqliteConnection>::new(db_url);
    r2d2::Pool::new(manager).expect("db pool")
}

// Connection extractor: a wrapper around an r2d2 pooled connection.
pub struct DbConn(pub r2d2::PooledConnection<ConnectionManager<SqliteConnection>>);

/// Attempts to retrieve a single connection from the database pool in the
/// application state. If no connections are available, fails with a
/// `ServiceUnavailable` status.
impl<S> FromRequestParts<S> for DbConn
where
    Pool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = Pool::from_ref(state);
        // r2d2 blocks while waiting for a free connection
        tokio::task::spawn_blocking(move || pool.get())
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .map(DbConn)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
    }
}

// For the convenience of using an &DbConn as an &SqliteConnection.
impl Deref for DbConn {
    type Target = SqliteConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}


// For the convenience of using an &DbConn as an &SqliteConnection.
impl DerefMut for DbConn {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
