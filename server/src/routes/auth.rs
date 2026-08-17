use axum::{Json, extract::State, http::StatusCode};
use larder_core::models::UserPublic;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::AuthUser;
use crate::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserPublic,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let user = state
        .users
        .authenticate(&body.email, &body.password)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::UNAUTHORIZED, "invalid email or password".into()))?;

    let token = state
        .jwt
        .issue(&user)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(LoginResponse {
        token,
        user: UserPublic::from(&user),
    }))
}

pub async fn me(user: AuthUser) -> Json<UserPublic> {
    Json(user.public())
}

pub async fn logout() -> StatusCode {
    // Client discards JWT; nothing server-side to revoke in L0.
    StatusCode::NO_CONTENT
}
