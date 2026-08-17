use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use larder_core::services::{BundleService, ExportService};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::ManagerUser;
use crate::AppState;

#[derive(Deserialize)]
pub struct ExportQuery {
    #[serde(default = "default_format")]
    pub format: String,
    /// Only recipes with this tag (e.g. work, coop)
    pub tag: Option<String>,
    /// Only recipes in this cookbook (uuid)
    pub cookbook: Option<String>,
}

fn default_format() -> String {
    "json".to_string()
}

fn ext_for_format(format: &str) -> (&'static str, &'static str) {
    match format {
        "markdown" | "md" => ("text/markdown", "larder-recipes.md"),
        "csv-recipes" | "csv" => ("text/csv", "larder-recipes.csv"),
        "csv-ingredients" => ("text/csv", "larder-ingredients.csv"),
        "simple" => ("application/json", "larder-recipes-simple.json"),
        _ => ("application/json", "larder-bundle.json"),
    }
}

async fn resolve_filter_ids(
    state: &AppState,
    tag: Option<&str>,
    cookbook: Option<&str>,
) -> Result<Option<Vec<Uuid>>, (StatusCode, String)> {
    if tag.is_none() && cookbook.is_none() {
        return Ok(None);
    }
    let mut ids: Option<std::collections::HashSet<Uuid>> = None;
    if let Some(t) = tag.filter(|s| !s.trim().is_empty()) {
        let tagged = state
            .recipes
            .list_recipes_by_tag(t.trim())
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        ids = Some(tagged.into_iter().map(|r| r.id).collect());
    }
    if let Some(c) = cookbook.filter(|s| !s.trim().is_empty()) {
        let cid = Uuid::parse_str(c.trim())
            .map_err(|_| (StatusCode::BAD_REQUEST, "invalid cookbook id".into()))?;
        let entries = state
            .cookbooks
            .get_recipes(cid)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let set: std::collections::HashSet<Uuid> =
            entries.into_iter().map(|e| e.recipe_id).collect();
        ids = Some(match ids {
            Some(prev) => prev.intersection(&set).copied().collect(),
            None => set,
        });
    }
    Ok(ids.map(|s| s.into_iter().collect()))
}

fn filter_recipes<T>(all: Vec<T>, ids: Option<&Vec<Uuid>>, id_of: impl Fn(&T) -> Uuid) -> Vec<T> {
    match ids {
        Some(allow) => {
            let set: std::collections::HashSet<Uuid> = allow.iter().copied().collect();
            all.into_iter().filter(|r| set.contains(&id_of(r))).collect()
        }
        None => all,
    }
}

pub async fn handler(
    _user: ManagerUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ExportQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = Uuid::nil();
    let format = params.format.as_str();
    let filter_ids = resolve_filter_ids(
        &state,
        params.tag.as_deref(),
        params.cookbook.as_deref(),
    )
    .await?;
    let (tag_for_bundle, id_allow) = if params.cookbook.as_ref().is_some_and(|s| !s.trim().is_empty())
    {
        (None, filter_ids.as_deref())
    } else if params.tag.as_ref().is_some_and(|s| !s.trim().is_empty()) {
        (params.tag.as_deref(), None)
    } else {
        (None, None)
    };

    let (content_type, filename) = ext_for_format(format);
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    match format {
        "simple" => {
            let all = filter_recipes(
                state
                    .recipes
                    .list_recipes(user_id)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
                filter_ids.as_ref(),
                |r| r.id,
            );
            let mut ingredients_map = Vec::new();
            let mut steps_map = Vec::new();
            for recipe in &all {
                ingredients_map.push((
                    recipe.id,
                    state
                        .recipes
                        .get_ingredients(recipe.id, None)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
                ));
                steps_map.push((
                    recipe.id,
                    state
                        .recipes
                        .get_steps(recipe.id)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
                ));
            }
            let body = ExportService::to_json(&all, &ingredients_map, &steps_map)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            Ok((headers, body).into_response())
        }
        "markdown" | "md" => {
            let all = filter_recipes(
                state
                    .recipes
                    .list_recipes(user_id)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
                filter_ids.as_ref(),
                |r| r.id,
            );
            let mut ingredients_map = Vec::new();
            let mut steps_map = Vec::new();
            let mut tags_map = Vec::new();
            for recipe in &all {
                ingredients_map.push((
                    recipe.id,
                    state
                        .recipes
                        .get_ingredients(recipe.id, None)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
                ));
                steps_map.push((
                    recipe.id,
                    state
                        .recipes
                        .get_steps(recipe.id)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
                ));
                let tags = state
                    .recipes
                    .get_tags(recipe.id)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                tags_map.push((
                    recipe.id,
                    tags.into_iter().map(|t| t.name).collect(),
                ));
            }
            let body = ExportService::to_markdown(&all, &ingredients_map, &steps_map, &tags_map)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            Ok((headers, body).into_response())
        }
        "csv-recipes" | "csv" => {
            let bundle = ExportService::export_bundle_filtered(
                &state.recipes,
                &state.ingredients,
                &state.locations,
                user_id,
                tag_for_bundle,
                id_allow,
            )
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let body = BundleService::to_csv_recipes(&bundle);
            Ok((headers, body).into_response())
        }
        "csv-ingredients" => {
            let bundle = ExportService::export_bundle_filtered(
                &state.recipes,
                &state.ingredients,
                &state.locations,
                user_id,
                tag_for_bundle,
                id_allow,
            )
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let body = BundleService::to_csv_ingredients(&bundle);
            Ok((headers, body).into_response())
        }
        "json" => {
            let bundle = if tag_for_bundle.is_some() || id_allow.is_some() {
                ExportService::export_bundle_filtered(
                    &state.recipes,
                    &state.ingredients,
                    &state.locations,
                    user_id,
                    tag_for_bundle,
                    id_allow,
                )
                .await
            } else {
                ExportService::export_bundle(
                    &state.recipes,
                    &state.ingredients,
                    &state.locations,
                    user_id,
                )
                .await
            }
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let body = ExportService::bundle_to_json(&bundle)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            Ok((headers, body).into_response())
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            "Unknown format. Use json, simple, markdown, csv-recipes, or csv-ingredients.".into(),
        )),
    }
}

pub async fn backup(
    _user: ManagerUser,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:larder.db".to_string());
    let path = database_url
        .strip_prefix("sqlite:")
        .filter(|p| !p.is_empty() && *p != ":memory:")
        .ok_or((
            StatusCode::BAD_REQUEST,
            "Backup requires a file-backed SQLite database".into(),
        ))?;

    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let stamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("larder-backup-{}.db", stamp);

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/octet-stream".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    Ok((headers, bytes).into_response())
}

pub async fn count(
    _user: ManagerUser,
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let user_id = Uuid::nil();
    let count = state
        .recipes
        .list_recipes(user_id)
        .await
        .map(|r| r.len())
        .unwrap_or(0);
    Json(serde_json::json!({ "recipes": count }))
}
