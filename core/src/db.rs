pub use sqlx::SqlitePool;

use anyhow::Result;
use sqlx::{SqlitePool as Pool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use tracing::info;
use uuid::Uuid;

use crate::models::Role;
use crate::services::location::{ELMWOOD_LOCATION_ID, HERTEL_LOCATION_ID, LocationService};
use crate::services::UserService;

/// Default / legacy single-user id (nil UUID). Recipes are owned by this user.
pub const DEFAULT_USER_ID: Uuid = Uuid::from_bytes([0; 16]);

/// Fixed kitchen demo account id.
pub const KITCHEN_USER_ID: Uuid = Uuid::from_bytes([
    0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
]);

pub async fn init_db(database_url: &str) -> Result<Pool> {
    info!("Connecting to database: {}", database_url);
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = Pool::connect_with(options).await?;
    run_migrations(&pool).await?;
    ensure_demo_users(&pool).await?;
    ensure_locations(&pool).await?;
    crate::seed::seed_if_empty(&pool).await?;
    crate::seed::ensure_bakery_demo(&pool).await?;
    crate::seed::ensure_stable_catalog(&pool).await?;
    crate::seed::ensure_department_tags(&pool).await?;
    Ok(pool)
}

async fn run_migrations(pool: &Pool) -> Result<()> {
    info!("Running migrations...");

    sqlx::query(include_str!("../../migrations/001_initial_schema.sql"))
        .execute(pool)
        .await?;

    for stmt in include_str!("../../migrations/002_costs_and_pricing.sql")
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let _ = sqlx::query(stmt).execute(pool).await;
    }

    run_soft_migration(pool, "003", include_str!("../../migrations/003_roles.sql")).await;
    run_soft_migration(
        pool,
        "004",
        include_str!("../../migrations/004_ingredient_master.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "005",
        include_str!("../../migrations/005_locations.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "006",
        include_str!("../../migrations/006_yield_waste.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "007",
        include_str!("../../migrations/007_production.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "008",
        include_str!("../../migrations/008_author_calories.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "009",
        include_str!("../../migrations/009_allergens.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "010",
        include_str!("../../migrations/010_fts.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "011",
        include_str!("../../migrations/011_g_per_cup.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "012",
        include_str!("../../migrations/012_recipe_opens.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "013",
        include_str!("../../migrations/013_recipe_notes.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "015",
        include_str!("../../migrations/015_service_station_tags.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "016",
        include_str!("../../migrations/016_meta_cleanup.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "017",
        include_str!("../../migrations/017_board_posts.sql"),
    )
    .await;
    run_soft_migration(
        pool,
        "018",
        include_str!("../../migrations/018_drop_weekly_menu.sql"),
    )
    .await;

    // Rebuild FTS after soft migrations so existing DBs get a fresh index.
    if let Err(e) = crate::services::recipe::RecipeService::new(pool.clone())
        .rebuild_fts()
        .await
    {
        tracing::debug!("recipe FTS rebuild skipped: {e}");
    }

    info!("Migrations complete");
    Ok(())
}

async fn run_soft_migration(pool: &Pool, label: &str, sql: &str) {
    for stmt in sql
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Err(e) = sqlx::query(stmt).execute(pool).await {
            tracing::debug!("migration {label} stmt skipped or applied: {e}");
        }
    }
}

async fn ensure_demo_users(pool: &Pool) -> Result<()> {
    let users = UserService::new(pool.clone());
    // Manager owns the shared recipe book (nil UUID, matches existing recipe.user_id)
    users
        .ensure_user(
            DEFAULT_USER_ID,
            "manager@larder.local",
            "Manager",
            "manager",
            Role::Manager,
        )
        .await?;
    users
        .ensure_user(
            KITCHEN_USER_ID,
            "kitchen@larder.local",
            "Kitchen",
            "kitchen",
            Role::Kitchen,
        )
        .await?;
    Ok(())
}

async fn ensure_locations(pool: &Pool) -> Result<()> {
    let locations = LocationService::new(pool.clone());
    locations.ensure_seed_locations().await?;
    for user_id in [DEFAULT_USER_ID, KITCHEN_USER_ID] {
        for loc_id in [ELMWOOD_LOCATION_ID, HERTEL_LOCATION_ID] {
            locations.ensure_user_location(user_id, loc_id).await?;
        }
    }
    Ok(())
}
