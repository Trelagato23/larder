use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::Utc;
use larder_core::models::ShoppingListItem;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize)]
pub struct ShoppingItemResponse {
    pub id: Uuid,
    pub item: String,
    pub quantity: Option<String>,
    pub unit: Option<String>,
    pub category: Option<String>,
    pub checked: bool,
}

#[derive(Deserialize)]
pub struct AddItemRequest {
    pub item: String,
    pub category: Option<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ShoppingItemResponse>>, (StatusCode, String)> {
    let user_id = Uuid::nil();
    let items = state
        .shopping
        .get_list(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        items
            .into_iter()
            .map(|i| ShoppingItemResponse {
                id: i.id,
                item: i.item,
                quantity: i.quantity.map(|q| q.to_string()),
                unit: i.unit,
                category: i.category,
                checked: i.checked,
            })
            .collect(),
    ))
}

pub async fn add_item(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddItemRequest>,
) -> Result<(StatusCode, Json<ShoppingItemResponse>), (StatusCode, String)> {
    let user_id = Uuid::nil();
    let id = Uuid::new_v4();
    let item = ShoppingListItem {
        id,
        user_id,
        item: body.item.trim().to_string(),
        quantity: None,
        unit: None,
        category: body.category.or(Some("Other".to_string())),
        checked: false,
        recipe_id: None,
        created_at: Utc::now(),
    };

    state
        .shopping
        .add_item(&item)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(ShoppingItemResponse {
            id: item.id,
            item: item.item,
            quantity: None,
            unit: None,
            category: item.category,
            checked: false,
        }),
    ))
}

pub async fn toggle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<&'static str, (StatusCode, String)> {
    state
        .shopping
        .toggle_checked(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok("toggled")
}

pub async fn delete_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<&'static str, (StatusCode, String)> {
    state
        .shopping
        .delete_item(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok("deleted")
}

pub async fn clear_checked(
    State(state): State<Arc<AppState>>,
) -> Result<&'static str, (StatusCode, String)> {
    let user_id = Uuid::nil();
    state
        .shopping
        .clear_checked(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok("cleared")
}
