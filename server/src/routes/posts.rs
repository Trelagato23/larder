use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use larder_core::models::{NoteAuthorRole, NoteSeverity, Role};
use larder_core::services::FeedPost;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::AppState;
use crate::routes::recipes::{CreateNoteRequest, SignNoteRequest, UpdateNoteRequest};

#[derive(Serialize)]
pub struct FeedPostResponse {
    pub id: String,
    pub kind: String,
    pub recipe_id: Option<String>,
    pub recipe_name: Option<String>,
    pub body: String,
    pub severity: String,
    pub author_role: String,
    pub author_name: String,
    pub author_user_id: Option<String>,
    pub signature: Option<String>,
    pub signed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<FeedPost> for FeedPostResponse {
    fn from(n: FeedPost) -> Self {
        Self {
            id: n.id.to_string(),
            kind: n.kind,
            recipe_id: n.recipe_id.map(|u| u.to_string()),
            recipe_name: n.recipe_name,
            body: n.body,
            severity: n.severity.as_str().to_string(),
            author_role: n.author_role.as_str().to_string(),
            author_name: n.author_name,
            author_user_id: n.author_user_id.map(|u| u.to_string()),
            signature: n.signature,
            signed_at: n.signed_at.map(|t| t.to_rfc3339()),
            created_at: n.created_at.to_rfc3339(),
            updated_at: n.updated_at.to_rfc3339(),
        }
    }
}

fn default_author_role(role: Role) -> NoteAuthorRole {
    match role {
        Role::Manager => NoteAuthorRole::Manager,
        Role::Kitchen => NoteAuthorRole::Team,
    }
}

pub async fn list(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<FeedPostResponse>>, (StatusCode, String)> {
    let posts = state
        .recipe_notes
        .list_feed()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(posts.into_iter().map(FeedPostResponse::from).collect()))
}

pub async fn create(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<FeedPostResponse>), (StatusCode, String)> {
    let severity = body
        .severity
        .as_deref()
        .and_then(NoteSeverity::parse)
        .unwrap_or(NoteSeverity::Subtle);
    let mut author_role = body
        .author_role
        .as_deref()
        .and_then(NoteAuthorRole::parse)
        .unwrap_or_else(|| default_author_role(user.role));
    if !user.role.can_edit_recipes() {
        author_role = NoteAuthorRole::Team;
    }
    let signature = body
        .signature
        .as_deref()
        .or(body.author_name.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or((StatusCode::BAD_REQUEST, "signature required".into()))?;
    let post = state
        .recipe_notes
        .create_board(
            &body.body,
            severity,
            author_role,
            signature,
            Some(user.id),
        )
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(FeedPostResponse::from(post))))
}

pub async fn update(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateNoteRequest>,
) -> Result<Json<FeedPostResponse>, (StatusCode, String)> {
    let existing = state
        .recipe_notes
        .get_board(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Post not found".into()))?;
    let is_author = existing.author_user_id == Some(user.id);
    if !user.role.can_edit_recipes() && !is_author {
        return Err((StatusCode::FORBIDDEN, "not allowed".into()));
    }
    let severity = body.severity.as_deref().and_then(NoteSeverity::parse);
    let post = state
        .recipe_notes
        .update_board(id, body.body.as_deref(), severity)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(FeedPostResponse::from(post)))
}

pub async fn delete(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<&'static str, (StatusCode, String)> {
    let existing = state
        .recipe_notes
        .get_board(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Post not found".into()))?;
    let is_author = existing.author_user_id == Some(user.id);
    if !user.role.can_edit_recipes() && !is_author {
        return Err((StatusCode::FORBIDDEN, "not allowed".into()));
    }
    let ok = state
        .recipe_notes
        .delete_board(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !ok {
        return Err((StatusCode::NOT_FOUND, "Post not found".into()));
    }
    Ok("deleted")
}

pub async fn sign(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<SignNoteRequest>,
) -> Result<Json<FeedPostResponse>, (StatusCode, String)> {
    let _ = state
        .recipe_notes
        .get_board(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Post not found".into()))?;
    let sig = body.signature.trim();
    let signature = if sig.is_empty() {
        user.name.clone()
    } else {
        sig.to_string()
    };
    let post = state
        .recipe_notes
        .sign_board(id, &signature)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(FeedPostResponse::from(post)))
}
