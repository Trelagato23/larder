use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::ManagerUser;
use crate::AppState;

#[derive(Deserialize)]
pub struct ImportRequest {
    pub url: String,
}

#[derive(Deserialize)]
pub struct ImportJsonRequest {
    pub data: serde_json::Value,
}

#[derive(serde::Serialize)]
pub struct ImportResponse {
    pub id: Uuid,
    pub name: String,
    pub servings: u32,
    pub ingredient_count: usize,
    pub step_count: usize,
}

#[derive(serde::Serialize)]
pub struct ImportJsonResponse {
    pub recipes_imported: usize,
    pub ingredients_upserted: usize,
    pub location_prices_set: usize,
}

pub async fn handler(
    _manager: ManagerUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ImportRequest>,
) -> Result<Json<ImportResponse>, (StatusCode, String)> {
    let imported = state
        .importer
        .import_from_url(&body.url)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let mut recipe = imported.recipe;
    recipe.user_id = Uuid::nil();
    let recipe_id = state
        .recipes
        .create_recipe(&recipe)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let ing_count = imported.ingredients.len();
    let step_count = imported.steps.len();

    for mut ing in imported.ingredients {
        ing.recipe_id = recipe_id;
        state
            .recipes
            .add_ingredient(&ing)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    for mut step in imported.steps {
        step.recipe_id = recipe_id;
        state
            .recipes
            .add_step(&step)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(Json(ImportResponse {
        id: recipe_id,
        name: recipe.name,
        servings: recipe.servings,
        ingredient_count: ing_count,
        step_count,
    }))
}

pub async fn json_handler(
    _manager: ManagerUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ImportJsonRequest>,
) -> Result<Json<ImportJsonResponse>, (StatusCode, String)> {
    let json_str = serde_json::to_string(&body.data)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let bundle = larder_core::services::BundleService::parse(&json_str)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if bundle.recipes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No recipes found in import data".into()));
    }

    let result = larder_core::services::BundleService::import(
        &state.recipes,
        &state.ingredients,
        &state.tags,
        &state.locations,
        &bundle,
        Uuid::nil(),
    )
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(ImportJsonResponse {
        recipes_imported: result.recipes_imported,
        ingredients_upserted: result.ingredients_upserted,
        location_prices_set: result.location_prices_set,
    }))
}
