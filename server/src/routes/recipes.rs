use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use larder_core::models::{
    normalize_allergens, Difficulty, NoteAuthorRole, NoteSeverity, Recipe, RecipeIngredient,
    RecipeNote, RecipeStep, Role,
};
use larder_core::services::ingredient_bases;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{AuthUser, LocationContext, ManagerUser};
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
    pub menu_price: Option<String>,
    pub author: Option<String>,
    /// Estimated kcal per serving.
    pub estimated_calories: Option<u32>,
    /// Comma-separated allergen / dietary labels.
    pub allergens: Option<String>,
    /// Batch output (e.g. 24) — what one batch makes. Required for batch-size display.
    pub yield_quantity: Option<String>,
    pub yield_unit: Option<String>,
    #[serde(default)]
    pub ingredients: Option<Vec<IngredientInput>>,
    #[serde(default)]
    pub steps: Option<Vec<StepInput>>,
}

#[derive(Deserialize)]
pub struct IngredientInput {
    pub display: String,
    pub ingredient: Option<String>,
    pub quantity: Option<String>,
    pub unit: Option<String>,
    pub category: Option<String>,
    pub cost_per_unit: Option<String>,
    pub line_cost: Option<String>,
    pub master_ingredient_id: Option<String>,
}

#[derive(Deserialize)]
pub struct StepInput {
    pub instruction: String,
    pub timer_seconds: Option<u32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_tag: Option<String>,
    /// Floor service: "hot" (Hot Bar) or "cold" (Grab & Go).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yield_quantity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yield_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waste_percent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_calories: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allergens: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_opened_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_count: Option<u32>,
}

fn pick_primary_tag(names: &[String], description: Option<&str>) -> Option<String> {
    // Product type before service station so cards stay consistent
    // (e.g. soup+grab-go → Soups, not Grab & Go).
    // Stations next (hot before cold). Skip noise (#work, seasons).
    const PRIORITY: &[&str] = &[
        "bakery",
        "soups",
        "sandwiches",
        "dips-and-spreads",
        "dressings",
        "flourless",
        "pizza",
        "salads",
        "deli",
        "hot-bar",
        "grab-go",
        "vegan",
        "vegetarian",
        // meal slots last — weak signals
        "lunch",
        "breakfast",
        "dinner",
        "snack",
    ];
    // Normalize hybrid / alias ChefTec tags to canons.
    let canon: Vec<String> = names
        .iter()
        .map(|n| {
            let k = n.trim().to_ascii_lowercase();
            match k.as_str() {
                "grab-go-hot-bar" => "hot-bar".to_string(),
                "grab-and-go" => "grab-go".to_string(),
                "dressing" => "dressings".to_string(),
                "bakery-grab-go" => "bakery".to_string(),
                "grab-go-sandwiches" => "sandwiches".to_string(),
                "grab-go-pizza" => "pizza".to_string(),
                "cheese-grab-go" => "grab-go".to_string(),
                _ => k,
            }
        })
        .filter(|n| !is_noise_tag(n))
        .collect();

    for dept in PRIORITY {
        if canon.iter().any(|n| n == dept) {
            return Some(dept.to_string());
        }
    }
    if let Some(t) = canon.into_iter().find(|n| !is_noise_tag(n)) {
        return Some(t);
    }
    // No useful tags — infer station from ChefTec description line.
    station_from_description(description)
}

/// Parse `station: …` from description when tags are missing (product-ish).
fn station_from_description(description: Option<&str>) -> Option<String> {
    let station = description_station(description)?;
    // Dual "grab & go, hot bar" → hot-bar (hot wins; exclusive stations)
    if station.contains("hot bar") || station.contains("hot-bar") {
        return Some("hot-bar".to_string());
    }
    if station.contains("grab") {
        return Some("grab-go".to_string());
    }
    if station.contains("pizza") {
        return Some("pizza".to_string());
    }
    if station.contains("bakery") {
        return Some("bakery".to_string());
    }
    if station.contains("soup") {
        return Some("soups".to_string());
    }
    if station.contains("deli") {
        return Some("deli".to_string());
    }
    None
}

fn description_station(description: Option<&str>) -> Option<String> {
    let desc = description?.to_ascii_lowercase();
    desc.split('·')
        .map(str::trim)
        .find(|b| b.starts_with("station:"))
        .map(|b| b.trim_start_matches("station:").trim().to_string())
}

/// Hot = Hot Bar (served hot). Cold = Grab & Go (served cold). Never both.
fn pick_service_line(names: &[String], description: Option<&str>) -> Option<String> {
    let tags: Vec<String> = names
        .iter()
        .map(|n| n.trim().to_ascii_lowercase())
        .collect();
    let has = |t: &str| tags.iter().any(|x| x == t);

    // Food served hot always Hot Bar — even if also tagged grab-go by mistake.
    // (Pizza is not Hot Bar service — cold case / grab-go or its own line.)
    if has("soups") || has("hot-bar") || has("grab-go-hot-bar") {
        return Some("hot".into());
    }

    // ChefTec station: hot wins over dual "Grab & Go, Hot Bar"
    if let Some(station) = description_station(description) {
        if station.contains("hot bar") || station.contains("hot-bar") {
            return Some("hot".into());
        }
        if station.contains("grab") {
            return Some("cold".into());
        }
    }

    // Explicit cold station / cold-case products → Grab & Go
    // Pizza: not served as Hot Bar — treat as cold/grab when no hot-bar tag
    if has("grab-go")
        || has("grab-and-go")
        || has("dips-and-spreads")
        || has("dressings")
        || has("dressing")
        || has("salads")
        || has("sandwiches")
        || has("bakery")
        || has("flourless")
        || has("pizza")
    {
        return Some("cold".into());
    }

    None
}

/// Deploy / season / day-menu tags — never use as primary.
fn is_noise_tag(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    matches!(
        n.as_str(),
        "work"
            | "incomplete"
            | "hb-menus"
            | "hb1-sunday"
            | "hb2-monday"
            | "hb3-tuesday"
            | "hb4-wednesday"
            | "hb5-thursday"
            | "hb6-friday"
            | "hb7-saturday"
            | "demo"
            | "demo-hot-bar"
            | "grab-go-hot-bar"
            // seasons — extras only, not dept stripe
            | "fall"
            | "winter"
            | "spring"
            | "summer"
            | "seasonal-fall"
            | "seasonal-winter"
            | "seasonal-spring"
            | "seasonal-summer"
    ) || n.starts_with("hb")
        && (n.contains("sunday")
            || n.contains("monday")
            || n.contains("tuesday")
            || n.contains("wednesday")
            || n.contains("thursday")
            || n.contains("friday")
            || n.contains("saturday")
            || n.starts_with("hb1")
            || n.starts_with("hb2")
            || n.starts_with("hb3")
            || n.starts_with("hb4")
            || n.starts_with("hb5")
            || n.starts_with("hb6")
            || n.starts_with("hb7"))
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
            menu_price: r.menu_price.map(|p| p.to_string()),
            primary_tag: None,
            service_line: None,
            yield_quantity: r.yield_quantity.map(|p| p.to_string()),
            yield_unit: r.yield_unit,
            waste_percent: r.waste_percent.map(|p| p.to_string()),
            author: r.author,
            estimated_calories: r.estimated_calories,
            allergens: r.allergens,
            created_at: r.created_at,
            last_opened_at: r.last_opened_at,
            open_count: r.open_count,
        }
    }
}

impl RecipeResponse {
    fn visible_to(mut self, role: Role) -> Self {
        self.menu_price = role.filter_cost(self.menu_price);
        self
    }
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub tag: Option<String>,
}

fn parse_opt_decimal(s: Option<&str>) -> Option<rust_decimal::Decimal> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok())
}

fn parse_opt_text(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn parse_difficulty(d: Option<&str>) -> Option<Difficulty> {
    d.and_then(|d| match d {
        "easy" => Some(Difficulty::Easy),
        "medium" => Some(Difficulty::Medium),
        "hard" => Some(Difficulty::Hard),
        _ => None,
    })
}

async fn save_ingredients_steps(
    state: &AppState,
    recipe_id: Uuid,
    ingredients: Option<Vec<IngredientInput>>,
    steps: Option<Vec<StepInput>>,
) -> Result<(), (StatusCode, String)> {
    if let Some(ings) = ingredients {
        let mut parsed = Vec::new();
        for i in ings.into_iter().filter(|i| !i.display.trim().is_empty()) {
            let display = i.display.trim().to_string();
            let name = i
                .ingredient
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| display.clone());
            let qty = i.quantity.and_then(|q| q.parse().ok());
            let mut master_id = i
                .master_ingredient_id
                .as_ref()
                .and_then(|s| Uuid::parse_str(s).ok());
            let mut cost_per_unit = i.cost_per_unit.and_then(|p| p.parse().ok());
            let line_cost = i.line_cost.and_then(|p| p.parse().ok());

            // Auto-link / create master by ingredient name for roll-through
            if master_id.is_none() && !name.trim().is_empty() {
                let cpu = cost_per_unit.or_else(|| match (line_cost, qty) {
                    (Some(lc), Some(q)) if q > rust_decimal::Decimal::ZERO => Some(lc / q),
                    _ => None,
                });
                if let Ok(master) = state
                    .ingredients
                    .find_or_create(&name, i.unit.as_deref(), cpu)
                    .await
                {
                    master_id = Some(master.id);
                    if cost_per_unit.is_none() {
                        cost_per_unit = master.cost_per_unit;
                    }
                }
            }

            parsed.push(RecipeIngredient {
                id: Uuid::new_v4(),
                recipe_id,
                ingredient: name,
                quantity: qty,
                unit: i.unit,
                note: None,
                display,
                category: i.category,
                cost_per_unit,
                line_cost: if master_id.is_some() { None } else { line_cost },
                master_ingredient_id: master_id,
                master_name: None,
                master_g_per_cup: None,
                prep_yield_percent: None,
            });
        }
        state
            .recipes
            .replace_ingredients(recipe_id, &parsed)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    if let Some(stps) = steps {
        let parsed: Vec<RecipeStep> = stps
            .into_iter()
            .filter(|s| !s.instruction.trim().is_empty())
            .enumerate()
            .map(|(idx, s)| RecipeStep {
                id: Uuid::new_v4(),
                recipe_id,
                position: (idx + 1) as u32,
                instruction: s.instruction.trim().to_string(),
                timer_seconds: s.timer_seconds,
            })
            .collect();
        state
            .recipes
            .replace_steps(recipe_id, &parsed)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(())
}

pub async fn list(
    user: AuthUser,
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
    let ids: Vec<Uuid> = recipes.iter().map(|r| r.id).collect();
    let tag_map = state
        .recipes
        .tags_by_recipe_ids(&ids)
        .await
        .unwrap_or_default();
    Json(
        recipes
            .into_iter()
            .map(|r| {
                let mut resp = RecipeResponse::from(r.clone()).visible_to(user.role);
                let names = tag_map.get(&r.id).cloned().unwrap_or_default();
                let desc = r.description.as_deref();
                resp.primary_tag = pick_primary_tag(&names, desc);
                resp.service_line = pick_service_line(&names, desc);
                resp
            })
            .collect(),
    )
}

pub async fn create(
    _manager: ManagerUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRecipeRequest>,
) -> (axum::http::StatusCode, Json<RecipeResponse>) {
    let difficulty = parse_difficulty(body.difficulty.as_deref());

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
        menu_price: body.menu_price.and_then(|p| p.parse().ok()),
        yield_quantity: parse_opt_decimal(body.yield_quantity.as_deref()),
        yield_unit: parse_opt_text(body.yield_unit.as_deref()),
        waste_percent: None,
        author: body.author.filter(|s| !s.trim().is_empty()),
        estimated_calories: body.estimated_calories,
        allergens: normalize_allergens(body.allergens.as_deref()),
            last_opened_at: None,
            open_count: None,
    };

    let id = state
        .recipes
        .create_recipe(&recipe)
        .await
        .unwrap_or_default();
    let mut created = recipe;
    created.id = id;

    let _ = save_ingredients_steps(&state, id, body.ingredients, body.steps).await;

    (
        axum::http::StatusCode::CREATED,
        Json(RecipeResponse::from(created)),
    )
}

pub async fn show(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RecipeResponse>, (axum::http::StatusCode, String)> {
    // Kitchen open → floats up in list; never-opened sinks.
    let _ = state.recipes.record_open(id).await;

    let recipe = state
        .recipes
        .get_recipe(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Recipe not found".to_string(),
        ))?;

    Ok(Json(RecipeResponse::from(recipe).visible_to(user.role)))
}

pub async fn update(
    _manager: ManagerUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateRecipeRequest>,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    let difficulty = parse_difficulty(body.difficulty.as_deref());
    let prep = body.prep_time_minutes;
    let cook = body.cook_time_minutes;
    let total_time_minutes = match (prep, cook) {
        (Some(p), Some(c)) => Some(p + c),
        (Some(p), None) => Some(p),
        (None, Some(c)) => Some(c),
        (None, None) => None,
    };

    let existing = state
        .recipes
        .get_recipe(id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Recipe not found".to_string(),
        ))?;

    let recipe = Recipe {
        id,
        name: body.name,
        description: body.description,
        image_url: existing.image_url,
        servings: body.servings.unwrap_or(1),
        prep_time_minutes: prep,
        cook_time_minutes: cook,
        total_time_minutes,
        source_url: body.source_url,
        created_at: existing.created_at,
        updated_at: chrono::Utc::now(),
        user_id: existing.user_id,
        rating: body.rating,
        difficulty,
        menu_price: body.menu_price.and_then(|p| p.parse().ok()),
        yield_quantity: parse_opt_decimal(body.yield_quantity.as_deref()),
        yield_unit: parse_opt_text(body.yield_unit.as_deref()),
        waste_percent: existing.waste_percent,
        author: body
            .author
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        estimated_calories: body.estimated_calories,
        allergens: normalize_allergens(body.allergens.as_deref()),
            last_opened_at: None,
            open_count: None,
    };

    state
        .recipes
        .update_recipe(&recipe)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    save_ingredients_steps(&state, id, body.ingredients, body.steps).await?;

    Ok("updated")
}

pub async fn delete(
    _manager: ManagerUser,
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
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): Query<SearchQuery>,
) -> Json<Vec<RecipeResponse>> {
    let recipes = state
        .recipes
        .search_recipes(&params.q)
        .await
        .unwrap_or_default();
    Json(
        recipes
            .into_iter()
            .map(|r| RecipeResponse::from(r).visible_to(user.role))
            .collect(),
    )
}

#[derive(Serialize)]
pub struct IngredientResponse {
    pub display: String,
    pub ingredient: String,
    pub quantity: Option<String>,
    pub unit: Option<String>,
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_cost: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaled_cost: Option<String>,
    pub master_ingredient_id: Option<String>,
    pub master_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_g_per_cup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_ml: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mass_g: Option<String>,
}

pub async fn ingredients(
    user: AuthUser,
    loc: LocationContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<IngredientResponse>>, (StatusCode, String)> {
    loc.validate(&user, &state).await?;
    let ings = state
        .recipes
        .get_ingredients(id, loc.0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<IngredientResponse> = ings
        .into_iter()
        .map(|i| {
            let (volume_ml, mass_g) =
                ingredient_bases(i.quantity, i.unit.as_deref(), &i.ingredient, i.master_g_per_cup);
            IngredientResponse {
                display: i.display,
                ingredient: i.ingredient,
                quantity: i.quantity.map(|q| q.to_string()),
                unit: i.unit,
                note: i.note,
                cost_per_unit: user.role.filter_cost(i.cost_per_unit.map(|p| p.to_string())),
                line_cost: user.role.filter_cost(i.line_cost.map(|p| p.to_string())),
                scaled_cost: None,
                master_ingredient_id: i.master_ingredient_id.map(|id| id.to_string()),
                master_name: i.master_name,
                master_g_per_cup: i.master_g_per_cup.map(|q| q.normalize().to_string()),
                volume_ml: volume_ml.map(|q| q.normalize().to_string()),
                mass_g: mass_g.map(|q| q.normalize().to_string()),
            }
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
    _user: AuthUser,
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

#[derive(Serialize)]
pub struct RelatedLinkResponse {
    pub id: String,
    pub name: String,
    pub via: String,
}

#[derive(Serialize)]
pub struct RelatedRecipesResponse {
    /// Component recipes this recipe's ingredient list points at.
    pub uses: Vec<RelatedLinkResponse>,
    /// Parent recipes that include this one as an ingredient.
    pub used_in: Vec<RelatedLinkResponse>,
}

/// Direct recipe↔recipe links only (ingredient name equals another recipe name).
pub async fn related(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RelatedRecipesResponse>, (StatusCode, String)> {
    let rel = state
        .recipes
        .related_recipes(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RelatedRecipesResponse {
        uses: rel
            .uses
            .into_iter()
            .map(|r| RelatedLinkResponse {
                id: r.id.to_string(),
                name: r.name,
                via: r.via,
            })
            .collect(),
        used_in: rel
            .used_in
            .into_iter()
            .map(|r| RelatedLinkResponse {
                id: r.id.to_string(),
                name: r.name,
                via: r.via,
            })
            .collect(),
    }))
}

// ── Recipe notes / flags ──────────────────────────────────────────────

#[derive(Serialize)]
pub struct NoteResponse {
    pub id: String,
    pub recipe_id: String,
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

impl From<RecipeNote> for NoteResponse {
    fn from(n: RecipeNote) -> Self {
        Self {
            id: n.id.to_string(),
            recipe_id: n.recipe_id.to_string(),
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

#[derive(Deserialize)]
pub struct CreateNoteRequest {
    pub body: String,
    /// subtle | flagged
    pub severity: Option<String>,
    /// team | supervisor | manager (defaults from login role)
    pub author_role: Option<String>,
    /// Optional display name override
    pub author_name: Option<String>,
    /// Required on Posts: signed name for the note
    pub signature: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateNoteRequest {
    pub body: Option<String>,
    pub severity: Option<String>,
}

#[derive(Deserialize)]
pub struct SignNoteRequest {
    /// Typed manager name for sign-off
    pub signature: String,
}

fn default_author_role(role: Role) -> NoteAuthorRole {
    match role {
        Role::Manager => NoteAuthorRole::Manager,
        Role::Kitchen => NoteAuthorRole::Team,
    }
}

pub async fn list_notes(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<NoteResponse>>, (StatusCode, String)> {
    let notes = state
        .recipe_notes
        .list_for_recipe(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(notes.into_iter().map(NoteResponse::from).collect()))
}

pub async fn create_note(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), (StatusCode, String)> {
    // Ensure recipe exists
    let _ = state
        .recipes
        .get_recipe(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Recipe not found".into()))?;

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
    // Kitchen may only post as team
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
    let author_name = signature;

    let note = state
        .recipe_notes
        .create(
            id,
            &body.body,
            severity,
            author_role,
            author_name,
            Some(user.id),
        )
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(NoteResponse::from(note))))
}

pub async fn update_note(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((recipe_id, note_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateNoteRequest>,
) -> Result<Json<NoteResponse>, (StatusCode, String)> {
    let existing = state
        .recipe_notes
        .get(note_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Note not found".into()))?;
    if existing.recipe_id != recipe_id {
        return Err((StatusCode::NOT_FOUND, "Note not found".into()));
    }
    let is_author = existing.author_user_id == Some(user.id);
    if !user.role.can_edit_recipes() && !is_author {
        return Err((StatusCode::FORBIDDEN, "not allowed".into()));
    }
    let severity = body.severity.as_deref().and_then(NoteSeverity::parse);
    let note = state
        .recipe_notes
        .update_body_severity(note_id, body.body.as_deref(), severity)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(NoteResponse::from(note)))
}

pub async fn delete_note(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((recipe_id, note_id)): Path<(Uuid, Uuid)>,
) -> Result<&'static str, (StatusCode, String)> {
    let existing = state
        .recipe_notes
        .get(note_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Note not found".into()))?;
    if existing.recipe_id != recipe_id {
        return Err((StatusCode::NOT_FOUND, "Note not found".into()));
    }
    let is_author = existing.author_user_id == Some(user.id);
    if !user.role.can_edit_recipes() && !is_author {
        return Err((StatusCode::FORBIDDEN, "not allowed".into()));
    }
    let ok = state
        .recipe_notes
        .delete(note_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !ok {
        return Err((StatusCode::NOT_FOUND, "Note not found".into()));
    }
    Ok("deleted")
}

pub async fn sign_note(
    manager: ManagerUser,
    State(state): State<Arc<AppState>>,
    Path((recipe_id, note_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<SignNoteRequest>,
) -> Result<Json<NoteResponse>, (StatusCode, String)> {
    let existing = state
        .recipe_notes
        .get(note_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Note not found".into()))?;
    if existing.recipe_id != recipe_id {
        return Err((StatusCode::NOT_FOUND, "Note not found".into()));
    }
    let sig = body.signature.trim();
    let signature = if sig.is_empty() {
        manager.0.name.clone()
    } else {
        sig.to_string()
    };
    let note = state
        .recipe_notes
        .sign(note_id, &signature)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(NoteResponse::from(note)))
}
