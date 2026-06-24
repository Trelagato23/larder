use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize)]
pub struct TagResponse {
    pub id: Uuid,
    pub name: String,
    pub color: Option<String>,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Json<Vec<TagResponse>> {
    let tags = state.tags.list_all().await.unwrap_or_default();
    Json(
        tags.into_iter()
            .map(|t| TagResponse {
                id: t.id,
                name: t.name,
                color: t.color,
            })
            .collect(),
    )
}

pub async fn recipe_tags(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TagResponse>>, (StatusCode, String)> {
    let tags = state
        .recipes
        .get_tags(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        tags.into_iter()
            .map(|t| TagResponse {
                id: t.id,
                name: t.name,
                color: t.color,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct AddTagRequest {
    pub name: String,
}

pub async fn add_recipe_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, String)> {
    let tag = state
        .tags
        .add_to_recipe(id, &body.name)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(TagResponse {
        id: tag.id,
        name: tag.name,
        color: tag.color,
    }))
}

pub async fn remove_recipe_tag(
    State(state): State<Arc<AppState>>,
    Path((recipe_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<&'static str, (StatusCode, String)> {
    state
        .tags
        .remove_from_recipe(recipe_id, tag_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok("removed")
}
