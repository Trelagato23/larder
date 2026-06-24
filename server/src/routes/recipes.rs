use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use larder_core::models::{Difficulty, Recipe};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct CreateRecipeRequest {
    pub name: String,
    pub description: Option<String>,
    pub servings: Option<u32>,
    pub prep_time_minutes: Option<u32>,
    pub cook_time_minutes: Option<u32>,
    pub source_url: Option<String>,
    pub rating: Option<u8>,
    pub difficulty: Option<String>,
}

#[derive(Serialize)]
pub struct RecipeResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub servings: u32,
    pub prep_time_minutes: Option<u32>,
    pub cook_time_minutes: Option<u32>,
    pub total_time_minutes: Option<u32>,
    pub source_url: Option<String>,
    pub rating: Option<u8>,
    pub difficulty: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<Recipe> for RecipeResponse {
    fn from(r: Recipe) -> Self {
        let total = r.total_time();
        Self {
            id: r.id,
            name: r.name,
            description: r.description,
            servings: r.servings,
            prep_time_minutes: r.prep_time_minutes,
            cook_time_minutes: r.cook_time_minutes,
            total_time_minutes: total,
            source_url: r.source_url,
            rating: r.rating,
            difficulty: r.difficulty.map(|d| match d {
                Difficulty::Easy => "easy".to_string(),
                Difficulty::Medium => "medium".to_string(),
                Difficulty::Hard => "hard".to_string(),
            }),
            created_at: r.created_at,
        }
    }
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub tag: Option<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListQuery>,
) -> Json<Vec<RecipeResponse>> {
    let default_user = Uuid::nil();
    let recipes = if let Some(tag) = params.tag.filter(|t| !t.trim().is_empty()) {
        state
            .recipes
            .list_recipes_by_tag(&tag)
            .await
            .unwrap_or_default()
    } else {
        state
            .recipes
            .list_recipes(default_user)
            .await
            .unwrap_or_default()
    };
    Json(recipes.into_iter().map(RecipeResponse::from).collect())
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRecipeRequest>,
) -> (axum::http::StatusCode, Json<RecipeResponse>) {
    let difficulty = body.difficulty.as_deref().and_then(|d| match d {
        "easy" => Some(Difficulty::Easy),
        "medium" => Some(Difficulty::Medium),
        "hard" => Some(Difficulty::Hard),
        _ => None,
    });

    let recipe = Recipe {
        id: Uuid::new_v4(),
        name: body.name,
        description: body.description,
        image_url: None,
        servings: body.servings.unwrap_or(1),
        prep_time_minutes: body.prep_time_minutes,
        cook_time_minutes: body.cook_time_minutes,
        total_time_minutes: None,
        source_url: body.source_url,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        user_id: Uuid::nil(),
        rating: body.rating,
        difficulty,
    };

    let id = state
        .recipes
        .create_recipe(&recipe)
        .await
        .unwrap_or_default();
    let mut created = recipe;
    created.id = id;

    (
        axum::http::StatusCode::CREATED,
        Json(RecipeResponse::from(created)),
    )
}

pub async fn show(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RecipeResponse>, (axum::http::StatusCode, String)> {
    let recipe = state
        .recipes
        .get_recipe(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Recipe not found".to_string(),
        ))?;

    Ok(Json(RecipeResponse::from(recipe)))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateRecipeRequest>,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    let difficulty = body.difficulty.as_deref().and_then(|d| match d {
        "easy" => Some(Difficulty::Easy),
        "medium" => Some(Difficulty::Medium),
        "hard" => Some(Difficulty::Hard),
        _ => None,
    });

    let recipe = Recipe {
        id,
        name: body.name,
        description: body.description,
        image_url: None,
        servings: body.servings.unwrap_or(1),
        prep_time_minutes: body.prep_time_minutes,
        cook_time_minutes: body.cook_time_minutes,
        total_time_minutes: None,
        source_url: body.source_url,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        user_id: Uuid::nil(),
        rating: body.rating,
        difficulty,
    };

    state
        .recipes
        .update_recipe(&recipe)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok("updated")
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    state
        .recipes
        .delete_recipe(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok("deleted")
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): Query<SearchQuery>,
) -> Json<Vec<RecipeResponse>> {
    let recipes = state
        .recipes
        .search_recipes(&params.q)
        .await
        .unwrap_or_default();
    Json(recipes.into_iter().map(RecipeResponse::from).collect())
}

#[derive(Serialize)]
pub struct IngredientResponse {
    pub display: String,
    pub ingredient: String,
    pub quantity: Option<String>,
    pub unit: Option<String>,
    pub note: Option<String>,
}

pub async fn ingredients(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<IngredientResponse>>, (StatusCode, String)> {
    let ings = state
        .recipes
        .get_ingredients(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<IngredientResponse> = ings
        .into_iter()
        .map(|i| IngredientResponse {
            display: i.display,
            ingredient: i.ingredient,
            quantity: i.quantity.map(|q| q.to_string()),
            unit: i.unit,
            note: i.note,
        })
        .collect();

    Ok(Json(response))
}

#[derive(Serialize)]
pub struct StepResponse {
    pub position: u32,
    pub instruction: String,
    pub timer_seconds: Option<u32>,
}

pub async fn steps(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<StepResponse>>, (StatusCode, String)> {
    let stps = state
        .recipes
        .get_steps(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<StepResponse> = stps
        .into_iter()
        .map(|s| StepResponse {
            position: s.position,
            instruction: s.instruction,
            timer_seconds: s.timer_seconds,
        })
        .collect();

    Ok(Json(response))
}
