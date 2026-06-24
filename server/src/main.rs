use anyhow::Result;
use larder_core::db::init_db;
use larder_core::services::{
    CookbookService, ImportService, MealPlanService, RecipeService, ShoppingListService,
    TagService,
};
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod middleware;
mod routes;

pub struct AppState {
    pub recipes: RecipeService,
    pub importer: ImportService,
    pub meal_plans: MealPlanService,
    pub shopping: ShoppingListService,
    pub tags: TagService,
    pub cookbooks: CookbookService,
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

    let state = Arc::new(AppState {
        recipes: RecipeService::new(pool.clone()),
        importer: ImportService::new(),
        meal_plans: MealPlanService::new(pool.clone()),
        shopping: ShoppingListService::new(pool.clone()),
        tags: TagService::new(pool.clone()),
        cookbooks: CookbookService::new(pool),
    });

    let app = routes::create_router(state);

    let addr = std::env::var("LARDER_ADDR")
        .or_else(|_| std::env::var("PORT").map(|port| format!("0.0.0.0:{port}")))
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse::<std::net::SocketAddr>()?;
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
