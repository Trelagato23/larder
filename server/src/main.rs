use anyhow::Result;
use larder_core::db::init_db;
use larder_core::services::{
    CookbookService, ImportService, IngredientMasterService, LocationService, MealPlanService,
    ProductionService, RecipeNoteService, RecipeService, ShoppingListService, TagService,
    UserService,
};
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod routes;

use auth::JwtKeys;

pub struct AppState {
    pub recipes: RecipeService,
    pub recipe_notes: RecipeNoteService,
    pub importer: ImportService,
    pub meal_plans: MealPlanService,
    pub shopping: ShoppingListService,
    pub tags: TagService,
    pub cookbooks: CookbookService,
    pub users: UserService,
    pub ingredients: IngredientMasterService,
    pub locations: LocationService,
    pub production: ProductionService,
    pub jwt: JwtKeys,
}

impl AppState {
    pub(crate) fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            recipes: RecipeService::new(pool.clone()),
            recipe_notes: RecipeNoteService::new(pool.clone()),
            importer: ImportService::new(),
            meal_plans: MealPlanService::new(pool.clone()),
            shopping: ShoppingListService::new(pool.clone()),
            tags: TagService::new(pool.clone()),
            cookbooks: CookbookService::new(pool.clone()),
            users: UserService::new(pool.clone()),
            ingredients: IngredientMasterService::new(pool.clone()),
            locations: LocationService::new(pool.clone()),
            production: ProductionService::new(pool.clone()),
            jwt: JwtKeys::from_env(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,larder_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:larder.db".to_string());
    let pool = init_db(&database_url).await?;

    let state = Arc::new(AppState::new(pool));

    let app = routes::create_router(state);

    let addr = std::env::var("LARDER_ADDR")
        .or_else(|_| std::env::var("PORT").map(|port| format!("0.0.0.0:{port}")))
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse::<std::net::SocketAddr>()?;
    tracing::info!("Listening on {}", addr);
    tracing::info!("Demo logins: manager@larder.local / manager · kitchen@larder.local / kitchen");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
