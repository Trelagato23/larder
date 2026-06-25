use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;
use tower_http::services::ServeDir;

use crate::AppState;

pub mod cookbooks;
pub mod export;
pub mod health;
pub mod import;
pub mod meal_plans;
pub mod recipes;
pub mod shopping;
pub mod tags;

pub fn create_router(state: Arc<AppState>) -> Router {
    let static_dir =
        std::env::var("LARDER_STATIC_DIR").unwrap_or_else(|_| "server/src/static".to_string());

    Router::new()
        .route("/", get(serve_index))
        .route("/health", get(health::handler))
        .route("/api/recipes", get(recipes::list).post(recipes::create))
        .route("/api/recipes/search", get(recipes::search))
        .route(
            "/api/recipes/{id}",
            get(recipes::show)
                .put(recipes::update)
                .delete(recipes::delete),
        )
        .route("/api/recipes/{id}/ingredients", get(recipes::ingredients))
        .route("/api/recipes/{id}/steps", get(recipes::steps))
        .route("/api/export", get(export::handler))
        .route("/api/stats", get(export::count))
        .route("/api/import", post(import::handler))
        .route("/api/meal-plans", get(meal_plans::list).post(meal_plans::set_meal))
        .route("/api/meal-plans/clear", post(meal_plans::clear_meal))
        .route(
            "/api/meal-plans/generate-shopping",
            post(meal_plans::generate_shopping),
        )
        .route("/api/shopping", get(shopping::list).post(shopping::add_item))
        .route("/api/shopping/clear-checked", post(shopping::clear_checked))
        .route("/api/shopping/{id}/toggle", post(shopping::toggle))
        .route(
            "/api/shopping/{id}",
            axum::routing::delete(shopping::delete_item),
        )
        .route("/api/tags", get(tags::list))
        .route("/api/recipes/{id}/tags", get(tags::recipe_tags).post(tags::add_recipe_tag))
        .route(
            "/api/recipes/{recipe_id}/tags/{tag_id}",
            axum::routing::delete(tags::remove_recipe_tag),
        )
        .route("/api/cookbooks", get(cookbooks::list).post(cookbooks::create))
        .route(
            "/api/cookbooks/{id}/recipes",
            get(cookbooks::recipes).post(cookbooks::add_recipe),
        )
        .fallback_service(ServeDir::new(static_dir))
        .with_state(state)
}

async fn serve_index() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../static/index.html"))
}
