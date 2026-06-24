pub use sqlx::SqlitePool;

use anyhow::Result;
use sqlx::{SqlitePool as Pool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use tracing::info;

pub async fn init_db(database_url: &str) -> Result<Pool> {
    info!("Connecting to database: {}", database_url);
    let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);
    let pool = Pool::connect_with(options).await?;
    run_migrations(&pool).await?;
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
