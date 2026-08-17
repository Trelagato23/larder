use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Html,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{AuthUser, LocationContext, ManagerUser};
use crate::AppState;

#[derive(Deserialize)]
pub struct ListQuery {
    pub date: Option<String>,
}

#[derive(Serialize)]
pub struct PlanResponse {
    pub id: Uuid,
    pub location_id: Option<Uuid>,
    pub plan_date: String,
    pub title: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct CreatePlanRequest {
    pub plan_date: String,
    pub title: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct AddItemRequest {
    pub recipe_id: Uuid,
    #[serde(default = "default_batches")]
    pub batches: String,
    pub servings_override: Option<u32>,
}

fn default_batches() -> String {
    "1".into()
}

pub async fn list(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<PlanResponse>>, (StatusCode, String)> {
    let date = q
        .date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Utc::now().date_naive());
    let rows = state
        .production
        .list_by_date(date)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(|p| PlanResponse {
                id: p.id,
                location_id: p.location_id,
                plan_date: p.plan_date.format("%Y-%m-%d").to_string(),
                title: p.title,
                notes: p.notes,
            })
            .collect(),
    ))
}

pub async fn create(
    user: ManagerUser,
    loc: LocationContext,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreatePlanRequest>,
) -> Result<(StatusCode, Json<PlanResponse>), (StatusCode, String)> {
    loc.validate(&user.0, &state).await?;
    let date = NaiveDate::parse_from_str(&body.plan_date, "%Y-%m-%d")
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid plan_date".into()))?;
    let plan = state
        .production
        .create(
            user.0.id,
            loc.0,
            date,
            body.title.as_deref(),
            body.notes.as_deref(),
        )
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok((
        StatusCode::CREATED,
        Json(PlanResponse {
            id: plan.id,
            location_id: plan.location_id,
            plan_date: plan.plan_date.format("%Y-%m-%d").to_string(),
            title: plan.title,
            notes: plan.notes,
        }),
    ))
}

pub async fn items(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<larder_core::services::ProductionPlanItem>>, (StatusCode, String)> {
    state
        .production
        .list_items(id)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn add_item(
    _manager: ManagerUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddItemRequest>,
) -> Result<(StatusCode, Json<larder_core::services::ProductionPlanItem>), (StatusCode, String)> {
    let batches: Decimal = body
        .batches
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid batches".into()))?;
    let item = state
        .production
        .add_item(id, body.recipe_id, batches, body.servings_override)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(item)))
}

pub async fn remove_item(
    _manager: ManagerUser,
    State(state): State<Arc<AppState>>,
    Path((_plan_id, item_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .production
        .remove_item(item_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn pull_list_json(
    user: AuthUser,
    loc: LocationContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<larder_core::services::PullListLine>>, (StatusCode, String)> {
    loc.validate(&user, &state).await?;
    let lines = state
        .production
        .generate_pull_list(id, loc.0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(lines))
}

pub async fn pull_list_html(
    user: AuthUser,
    loc: LocationContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, (StatusCode, String)> {
    loc.validate(&user, &state).await?;
    let plan = state
        .production
        .get(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "plan not found".into()))?;
    let lines = state
        .production
        .generate_pull_list(id, loc.0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rows: String = lines
        .iter()
        .map(|l| {
            format!(
                "<tr><td><span class=\"box\"></span></td><td><strong>{}</strong><br><span class=\"meta\">{}</span></td><td class=\"qty\">{}</td></tr>",
                html_escape(&l.ingredient),
                html_escape(&l.recipes.join(", ")),
                html_escape(&l.quantity_display),
            )
        })
        .collect();

    let title = plan.title.as_deref().unwrap_or("Production pull list");
    let html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="UTF-8"><title>{title}</title>
<style>
body {{ font-family: Georgia, serif; margin: 1rem; color: #111; }}
h1 {{ border-bottom: 2px solid #111; padding-bottom: 0.35rem; }}
table {{ width: 100%; border-collapse: collapse; margin-top: 1rem; }}
td {{ padding: 0.5rem 0.35rem; border-bottom: 1px solid #ddd; vertical-align: top; }}
.qty {{ text-align: right; font-weight: bold; white-space: nowrap; }}
.meta {{ color: #666; font-size: 0.85rem; }}
.box {{ display:inline-block;width:0.85rem;height:0.85rem;border:1.5px solid #111; }}
.noprint {{ margin-bottom: 1rem; }}
@media print {{ .noprint {{ display:none; }} }}
</style></head><body>
<div class="noprint"><button onclick="window.print()">Print</button></div>
<h1>{title}</h1>
<p>{date}</p>
<table><tbody>{rows}</tbody></table>
</body></html>"#,
        title = html_escape(title),
        date = plan.plan_date,
        rows = rows,
    );
    Ok(Html(html))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
