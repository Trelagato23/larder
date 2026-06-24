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
pub struct CookbookResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub public: bool,
}

#[derive(Deserialize)]
pub struct CreateCookbookRequest {
    pub name: String,
    pub description: Option<String>,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Json<Vec<CookbookResponse>> {
    let user_id = Uuid::nil();
    let books = state.cookbooks.list_cookbooks(user_id).await.unwrap_or_default();
    Json(
        books
            .into_iter()
            .map(|b| CookbookResponse {
                id: b.id,
                name: b.name,
                description: b.description,
                public: b.public,
            })
            .collect(),
    )
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateCookbookRequest>,
) -> Result<(StatusCode, Json<CookbookResponse>), (StatusCode, String)> {
    let user_id = Uuid::nil();
    let id = state
        .cookbooks
        .create_cookbook(
            &body.name,
            body.description.as_deref().unwrap_or(""),
            user_id,
            false,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CookbookResponse {
            id,
            name: body.name,
            description: body.description,
            public: false,
        }),
    ))
}

#[derive(Serialize)]
pub struct CookbookRecipeResponse {
    pub recipe_id: Uuid,
    pub recipe_name: Option<String>,
    pub position: u32,
}

pub async fn recipes(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<CookbookRecipeResponse>>, (StatusCode, String)> {
    let entries = state
        .cookbooks
        .get_recipes(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut response = Vec::new();
    for entry in entries {
        let recipe_name = state
            .recipes
            .get_recipe(entry.recipe_id)
            .await
            .ok()
            .flatten()
            .map(|r| r.name);
        response.push(CookbookRecipeResponse {
            recipe_id: entry.recipe_id,
            recipe_name,
            position: entry.position,
        });
    }

    Ok(Json(response))
}

#[derive(Deserialize)]
pub struct AddRecipeRequest {
    pub recipe_id: Uuid,
}

pub async fn add_recipe(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddRecipeRequest>,
) -> Result<&'static str, (StatusCode, String)> {
    state
        .cookbooks
        .add_recipe(id, body.recipe_id, 0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok("added")
}
