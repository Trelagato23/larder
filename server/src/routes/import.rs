use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct ImportRequest {
    pub url: String,
}

#[derive(serde::Serialize)]
pub struct ImportResponse {
    pub id: Uuid,
    pub name: String,
    pub servings: u32,
    pub ingredient_count: usize,
    pub step_count: usize,
}

pub async fn handler(
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
        step_count: step_count,
    }))
}
