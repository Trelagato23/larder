use axum::{Json, extract::State};
use larder_core::models::Location;
use serde::Serialize;
use std::sync::Arc;

use crate::auth::AuthUser;
use crate::AppState;

#[derive(Serialize)]
pub struct LocationResponse {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
}

impl From<Location> for LocationResponse {
    fn from(l: Location) -> Self {
        Self {
            id: l.id,
            slug: l.slug,
            name: l.name,
        }
    }
}

pub async fn list(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<LocationResponse>>, (axum::http::StatusCode, String)> {
    let rows = state
        .locations
        .list_for_user(user.id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows.into_iter().map(LocationResponse::from).collect()))
}
