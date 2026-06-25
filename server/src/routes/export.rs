use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use larder_core::services::ExportService;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct ExportQuery {
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "json".to_string()
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ExportQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = Uuid::nil();
    let all = state
        .recipes
        .list_recipes(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut ingredients_map = Vec::new();
    let mut steps_map = Vec::new();

    for recipe in &all {
        let ings = state
            .recipes
            .get_ingredients(recipe.id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let stps = state
            .recipes
            .get_steps(recipe.id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        ingredients_map.push((recipe.id, ings));
        steps_map.push((recipe.id, stps));
    }

    match params.format.as_str() {
        "json" => {
            let body = ExportService::to_json(&all, &ingredients_map, &steps_map)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                "application/json".parse().unwrap(),
            );
            headers.insert(
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"recipes.json\"".parse().unwrap(),
            );
            Ok((headers, body).into_response())
        }
        "markdown" | "md" => {
            let body = ExportService::to_markdown(&all, &ingredients_map, &steps_map)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, "text/markdown".parse().unwrap());
            headers.insert(
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"recipes.md\"".parse().unwrap(),
            );
            Ok((headers, body).into_response())
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            "Unknown format. Use json or markdown.".to_string(),
        )),
    }
}

pub async fn count(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let user_id = Uuid::nil();
    let count = state
        .recipes
        .list_recipes(user_id)
        .await
        .map(|r| r.len())
        .unwrap_or(0);
    Json(serde_json::json!({ "recipes": count }))
}
