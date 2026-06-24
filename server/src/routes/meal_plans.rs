use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use chrono::{Datelike, NaiveDate};
use larder_core::models::MealType;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct WeekQuery {
    pub week_start: Option<String>,
}

#[derive(Serialize)]
pub struct MealPlanResponse {
    pub id: Uuid,
    pub date: String,
    pub meal_type: String,
    pub recipe_id: Option<Uuid>,
    pub recipe_name: Option<String>,
    pub title: Option<String>,
}

#[derive(Deserialize)]
pub struct SetMealRequest {
    pub date: String,
    pub meal_type: String,
    pub recipe_id: Uuid,
}

fn parse_meal_type(value: &str) -> Option<MealType> {
    match value {
        "breakfast" => Some(MealType::Breakfast),
        "lunch" => Some(MealType::Lunch),
        "dinner" => Some(MealType::Dinner),
        "snack" => Some(MealType::Snack),
        _ => None,
    }
}

fn week_start_from_query(week_start: Option<String>) -> NaiveDate {
    if let Some(value) = week_start {
        if let Ok(date) = NaiveDate::parse_from_str(&value, "%Y-%m-%d") {
            return date;
        }
    }
    let today = chrono::Local::now().date_naive();
    today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64)
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<WeekQuery>,
) -> Result<Json<Vec<MealPlanResponse>>, (StatusCode, String)> {
    let user_id = Uuid::nil();
    let week_start = week_start_from_query(params.week_start);
    let meals = state
        .meal_plans
        .get_week(user_id, week_start)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut response = Vec::new();
    for meal in meals {
        let recipe_name = if let Some(recipe_id) = meal.recipe_id {
            state
                .recipes
                .get_recipe(recipe_id)
                .await
                .ok()
                .flatten()
                .map(|r| r.name)
        } else {
            None
        };

        response.push(MealPlanResponse {
            id: meal.id,
            date: meal.date.to_string(),
            meal_type: meal.meal_type.to_string().to_lowercase(),
            recipe_id: meal.recipe_id,
            recipe_name,
            title: meal.title,
        });
    }

    Ok(Json(response))
}

pub async fn set_meal(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetMealRequest>,
) -> Result<Json<MealPlanResponse>, (StatusCode, String)> {
    let user_id = Uuid::nil();
    let date = NaiveDate::parse_from_str(&body.date, "%Y-%m-%d")
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid date".to_string()))?;
    let meal_type = parse_meal_type(&body.meal_type)
        .ok_or((StatusCode::BAD_REQUEST, "Invalid meal type".to_string()))?;

    let id = state
        .meal_plans
        .set_recipe(user_id, date, meal_type, body.recipe_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let recipe_name = state
        .recipes
        .get_recipe(body.recipe_id)
        .await
        .ok()
        .flatten()
        .map(|r| r.name);

    Ok(Json(MealPlanResponse {
        id,
        date: body.date,
        meal_type: body.meal_type,
        recipe_id: Some(body.recipe_id),
        recipe_name,
        title: None,
    }))
}

#[derive(Deserialize)]
pub struct ClearMealRequest {
    pub date: String,
    pub meal_type: String,
}

pub async fn clear_meal(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ClearMealRequest>,
) -> Result<&'static str, (StatusCode, String)> {
    let user_id = Uuid::nil();
    let date = NaiveDate::parse_from_str(&body.date, "%Y-%m-%d")
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid date".to_string()))?;
    let meal_type = parse_meal_type(&body.meal_type)
        .ok_or((StatusCode::BAD_REQUEST, "Invalid meal type".to_string()))?;

    state
        .meal_plans
        .clear_slot(user_id, date, meal_type)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok("cleared")
}

#[derive(Deserialize)]
pub struct GenerateShoppingRequest {
    pub week_start: Option<String>,
}

pub async fn generate_shopping(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GenerateShoppingRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let user_id = Uuid::nil();
    let week_start = week_start_from_query(body.week_start);
    let count = state
        .shopping
        .generate_from_meal_plan(user_id, week_start)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "added": count })))
}
