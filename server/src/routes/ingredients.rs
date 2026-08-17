use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{AuthUser, LocationContext, ManagerUser};
use crate::AppState;

#[derive(Serialize)]
pub struct IngredientResponse {
    pub id: Uuid,
    pub name: String,
    pub default_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_unit: Option<String>,
    pub pack_size: Option<String>,
    pub pack_unit: Option<String>,
    pub notes: Option<String>,
    pub g_per_cup: Option<String>,
    pub used_in_recipes: i64,
}

#[derive(Deserialize)]
pub struct CreateIngredientRequest {
    pub name: String,
    pub default_unit: Option<String>,
    pub cost_per_unit: Option<String>,
    pub pack_size: Option<String>,
    pub pack_unit: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateIngredientRequest {
    pub name: Option<String>,
    pub default_unit: Option<String>,
    pub cost_per_unit: Option<String>,
    #[serde(default)]
    pub clear_cost: bool,
    pub pack_size: Option<String>,
    pub pack_unit: Option<String>,
    pub notes: Option<String>,
    pub g_per_cup: Option<String>,
    #[serde(default)]
    pub clear_g_per_cup: bool,
}

#[derive(Serialize)]
pub struct UsageResponse {
    pub recipe_id: Uuid,
    pub recipe_name: String,
}

#[derive(Serialize)]
pub struct BackfillResponse {
    pub lines_linked: u64,
    pub masters_created: u64,
}

pub async fn list(
    user: AuthUser,
    loc: LocationContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<IngredientResponse>>, (StatusCode, String)> {
    loc.validate(&user, &state).await?;
    let items = state
        .ingredients
        .list(loc.0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut out = Vec::new();
    for m in items {
        let used = state.ingredients.usage_count(m.id).await.unwrap_or(0);
        out.push(IngredientResponse {
            id: m.id,
            name: m.name,
            default_unit: m.default_unit,
            cost_per_unit: user.role.filter_cost(m.cost_per_unit.map(|p| p.to_string())),
            pack_size: m.pack_size.map(|p| p.to_string()),
            pack_unit: m.pack_unit,
            notes: m.notes,
            g_per_cup: m.g_per_cup.map(|p| p.to_string()),
            used_in_recipes: used,
        });
    }
    Ok(Json(out))
}

pub async fn show(
    user: AuthUser,
    loc: LocationContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<IngredientResponse>, (StatusCode, String)> {
    loc.validate(&user, &state).await?;
    let m = state
        .ingredients
        .get(id, loc.0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "ingredient not found".into()))?;
    let used = state.ingredients.usage_count(m.id).await.unwrap_or(0);
    Ok(Json(IngredientResponse {
        id: m.id,
        name: m.name,
        default_unit: m.default_unit,
        cost_per_unit: user.role.filter_cost(m.cost_per_unit.map(|p| p.to_string())),
        pack_size: m.pack_size.map(|p| p.to_string()),
        pack_unit: m.pack_unit,
        notes: m.notes,
            g_per_cup: m.g_per_cup.map(|p| p.to_string()),
        used_in_recipes: used,
    }))
}

pub async fn create(
    _manager: ManagerUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateIngredientRequest>,
) -> Result<(StatusCode, Json<IngredientResponse>), (StatusCode, String)> {
    let m = state
        .ingredients
        .create(
            &body.name,
            body.default_unit.as_deref(),
            body.cost_per_unit.and_then(|p| p.parse().ok()),
            body.pack_size.and_then(|p| p.parse().ok()),
            body.pack_unit.as_deref(),
            body.notes.as_deref(),
        )
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(IngredientResponse {
            id: m.id,
            name: m.name,
            default_unit: m.default_unit,
            cost_per_unit: m.cost_per_unit.map(|p| p.to_string()),
            pack_size: m.pack_size.map(|p| p.to_string()),
            pack_unit: m.pack_unit,
            notes: m.notes,
            g_per_cup: m.g_per_cup.map(|p| p.to_string()),
            used_in_recipes: 0,
        }),
    ))
}

pub async fn update(
    user: ManagerUser,
    loc: LocationContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateIngredientRequest>,
) -> Result<Json<IngredientResponse>, (StatusCode, String)> {
    loc.validate(&user.0, &state).await?;

    let cost = if body.clear_cost {
        Some(None)
    } else if let Some(ref c) = body.cost_per_unit {
        let parsed: Decimal = c
            .parse()
            .map_err(|_| (StatusCode::BAD_REQUEST, "invalid cost_per_unit".into()))?;
        Some(Some(parsed))
    } else {
        None
    };

    let has_meta = body.name.is_some()
        || body.default_unit.is_some()
        || body.pack_size.is_some()
        || body.pack_unit.is_some()
        || body.notes.is_some()
        || body.g_per_cup.is_some()
        || body.clear_g_per_cup;

    let density = if body.clear_g_per_cup {
        Some(None)
    } else if let Some(ref g) = body.g_per_cup {
        let parsed: Decimal = g
            .parse()
            .map_err(|_| (StatusCode::BAD_REQUEST, "invalid g_per_cup".into()))?;
        Some(Some(parsed))
    } else {
        None
    };

    if has_meta {
        state
            .ingredients
            .update(
                id,
                body.name.as_deref(),
                body.default_unit
                    .as_ref()
                    .map(|s| if s.is_empty() { None } else { Some(s.as_str()) }),
                None,
                body.pack_size.as_ref().map(|s| {
                    if s.is_empty() {
                        None
                    } else {
                        s.parse().ok()
                    }
                }),
                body.pack_unit
                    .as_ref()
                    .map(|s| if s.is_empty() { None } else { Some(s.as_str()) }),
                body.notes
                    .as_ref()
                    .map(|s| if s.is_empty() { None } else { Some(s.as_str()) }),
                density,
            )
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("not found") {
                    (StatusCode::NOT_FOUND, msg)
                } else {
                    (StatusCode::BAD_REQUEST, msg)
                }
            })?;
    }

    if let Some(cost_val) = cost {
        if let Some(loc_id) = loc.0 {
            state
                .locations
                .set_ingredient_price(loc_id, id, cost_val)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        } else {
            state
                .ingredients
                .update(id, None, None, Some(cost_val), None, None, None, None)
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("not found") {
                        (StatusCode::NOT_FOUND, msg)
                    } else {
                        (StatusCode::BAD_REQUEST, msg)
                    }
                })?;
        }
    }

    let m = state
        .ingredients
        .get(id, loc.0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "ingredient not found".into()))?;

    let used = state.ingredients.usage_count(m.id).await.unwrap_or(0);
    Ok(Json(IngredientResponse {
        id: m.id,
        name: m.name,
        default_unit: m.default_unit,
        cost_per_unit: m.cost_per_unit.map(|p| p.to_string()),
        pack_size: m.pack_size.map(|p| p.to_string()),
        pack_unit: m.pack_unit,
        notes: m.notes,
            g_per_cup: m.g_per_cup.map(|p| p.to_string()),
        used_in_recipes: used,
    }))
}

pub async fn delete(
    _manager: ManagerUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .ingredients
        .delete(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn usage(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<UsageResponse>>, (StatusCode, String)> {
    let rows = state
        .ingredients
        .recipe_usage(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(|(recipe_id, recipe_name)| UsageResponse {
                recipe_id,
                recipe_name,
            })
            .collect(),
    ))
}

pub async fn backfill(
    _manager: ManagerUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<BackfillResponse>, (StatusCode, String)> {
    let (lines_linked, masters_created) = state
        .ingredients
        .backfill_from_recipe_lines()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(BackfillResponse {
        lines_linked,
        masters_created,
    }))
}
