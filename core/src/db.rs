pub use sqlx::SqlitePool;

use anyhow::Result;
use sqlx::{SqlitePool as Pool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use tracing::info;
use uuid::Uuid;

/// Single local user for personal installs (no auth yet).
pub const DEFAULT_USER_ID: Uuid = Uuid::from_bytes([0; 16]);

pub async fn init_db(database_url: &str) -> Result<Pool> {
    info!("Connecting to database: {}", database_url);
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = Pool::connect_with(options).await?;
    run_migrations(&pool).await?;
    ensure_default_user(&pool).await?;
    Ok(pool)
}

async fn run_migrations(pool: &Pool) -> Result<()> {
    info!("Running migrations...");

    sqlx::query(include_str!("../../migrations/001_initial_schema.sql"))
        .execute(pool)
        .await?;

    info!("Migrations complete");
    Ok(())
}

async fn ensure_default_user(pool: &Pool) -> Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO users (id, email, name, password_hash) VALUES (?, ?, ?, ?)",
    )
    .bind(DEFAULT_USER_ID.to_string())
    .bind("local@localhost")
    .bind("local")
    .bind("!")
    .execute(pool)
    .await?;
    Ok(())
}
