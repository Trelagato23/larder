use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::models::RecipeIngredient;
use crate::services::{
    scaling::combined_scale_factor,
    uom::{normalize_unit, to_pull_display_with_density},
    RecipeService,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionPlan {
    pub id: Uuid,
    pub location_id: Option<Uuid>,
    pub plan_date: NaiveDate,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionPlanItem {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub recipe_id: Uuid,
    pub recipe_name: Option<String>,
    pub batches: Decimal,
    pub servings_override: Option<u32>,
    pub position: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullListLine {
    pub ingredient: String,
    pub master_name: Option<String>,
    pub quantity_display: String,
    pub unit: Option<String>,
    pub recipes: Vec<String>,
}

struct Agg {
    ingredient: String,
    master_name: Option<String>,
    density_g_per_cup: Option<Decimal>,
    total: Decimal,
    unit: Option<String>,
    recipes: Vec<String>,
}

pub struct ProductionService {
    pool: SqlitePool,
}

#[derive(FromRow)]
struct PlanRow {
    id: String,
    location_id: Option<String>,
    plan_date: String,
    title: Option<String>,
    notes: Option<String>,
    user_id: String,
    created_at: String,
}

impl PlanRow {
    fn into_plan(self) -> Result<ProductionPlan> {
        Ok(ProductionPlan {
            id: Uuid::parse_str(&self.id).context("plan id")?,
            location_id: self
                .location_id
                .as_ref()
                .and_then(|s| Uuid::parse_str(s).ok()),
            plan_date: NaiveDate::parse_from_str(&self.plan_date, "%Y-%m-%d")
                .or_else(|_| NaiveDate::parse_from_str(&self.plan_date[..10.min(self.plan_date.len())], "%Y-%m-%d"))
                .context("plan_date")?,
            title: self.title,
            notes: self.notes,
            user_id: Uuid::parse_str(&self.user_id).context("user_id")?,
            created_at: parse_dt(&self.created_at)?,
        })
    }
}

fn parse_dt(s: &str) -> Result<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    let naive = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
        .with_context(|| format!("parse datetime: {s}"))?;
    Ok(DateTime::from_naive_utc_and_offset(naive, Utc))
}

impl ProductionService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_by_date(&self, date: NaiveDate) -> Result<Vec<ProductionPlan>> {
        let rows: Vec<PlanRow> = sqlx::query_as(
            "SELECT id, location_id, plan_date, title, notes, user_id, created_at FROM production_plans WHERE plan_date = ? ORDER BY created_at",
        )
        .bind(date.format("%Y-%m-%d").to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(PlanRow::into_plan).collect()
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<ProductionPlan>> {
        let row: Option<PlanRow> = sqlx::query_as(
            "SELECT id, location_id, plan_date, title, notes, user_id, created_at FROM production_plans WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(PlanRow::into_plan).transpose()
    }

    pub async fn create(
        &self,
        user_id: Uuid,
        location_id: Option<Uuid>,
        plan_date: NaiveDate,
        title: Option<&str>,
        notes: Option<&str>,
    ) -> Result<ProductionPlan> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO production_plans (id, location_id, plan_date, title, notes, user_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(location_id.map(|l| l.to_string()))
        .bind(plan_date.format("%Y-%m-%d").to_string())
        .bind(title)
        .bind(notes)
        .bind(user_id.to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("plan missing after insert"))
    }

    pub async fn list_items(&self, plan_id: Uuid) -> Result<Vec<ProductionPlanItem>> {
        #[derive(FromRow)]
        struct Row {
            id: String,
            plan_id: String,
            recipe_id: String,
            recipe_name: Option<String>,
            batches: String,
            servings_override: Option<i64>,
            position: i64,
        }
        let rows: Vec<Row> = sqlx::query_as(
            r#"
            SELECT pi.id, pi.plan_id, pi.recipe_id, r.name AS recipe_name,
                   pi.batches, pi.servings_override, pi.position
            FROM production_plan_items pi
            JOIN recipes r ON r.id = pi.recipe_id
            WHERE pi.plan_id = ?
            ORDER BY pi.position, pi.rowid
            "#,
        )
        .bind(plan_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                Some(ProductionPlanItem {
                    id: Uuid::parse_str(&r.id).ok()?,
                    plan_id: Uuid::parse_str(&r.plan_id).ok()?,
                    recipe_id: Uuid::parse_str(&r.recipe_id).ok()?,
                    recipe_name: r.recipe_name,
                    batches: r.batches.parse().ok()?,
                    servings_override: r.servings_override.map(|v| v as u32),
                    position: r.position as u32,
                })
            })
            .collect())
    }

    pub async fn add_item(
        &self,
        plan_id: Uuid,
        recipe_id: Uuid,
        batches: Decimal,
        servings_override: Option<u32>,
    ) -> Result<ProductionPlanItem> {
        let id = Uuid::new_v4();
        let (max_pos,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(position), 0) FROM production_plan_items WHERE plan_id = ?",
        )
        .bind(plan_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO production_plan_items (id, plan_id, recipe_id, batches, servings_override, position) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(plan_id.to_string())
        .bind(recipe_id.to_string())
        .bind(batches.to_string())
        .bind(servings_override.map(|v| v as i64))
        .bind(max_pos + 1)
        .execute(&self.pool)
        .await?;

        self.list_items(plan_id)
            .await?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| anyhow::anyhow!("item missing after insert"))
    }

    pub async fn remove_item(&self, item_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM production_plan_items WHERE id = ?")
            .bind(item_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn generate_pull_list(
        &self,
        plan_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<Vec<PullListLine>> {
        let items = self.list_items(plan_id).await?;
        let recipes = RecipeService::new(self.pool.clone());
        let mut map: BTreeMap<String, Agg> = BTreeMap::new();

        for item in items {
            let recipe = recipes
                .get_recipe(item.recipe_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("recipe not found"))?;
            let ings = recipes
                .get_ingredients(item.recipe_id, location_id)
                .await?;
            let target_servings = item.servings_override.unwrap_or(recipe.servings);
            let factor = combined_scale_factor(recipe.servings, target_servings, item.batches);
            let recipe_waste = recipe.waste_percent.unwrap_or(Decimal::ZERO);
            let recipe_name = item.recipe_name.as_deref().unwrap_or(&recipe.name);

            for ing in ings {
                aggregate_ingredient(&mut map, &ing, factor, recipe_waste, recipe_name);
            }
        }

        Ok(map
            .into_values()
            .map(|a| PullListLine {
                ingredient: a.ingredient.clone(),
                master_name: a.master_name.clone(),
                quantity_display: to_pull_display_with_density(
                    a.total,
                    a.unit.as_deref(),
                    a.density_g_per_cup,
                    Some(&a.ingredient),
                ),
                unit: a.unit,
                recipes: a.recipes,
            })
            .collect())
    }
}

fn aggregate_ingredient(
    map: &mut BTreeMap<String, Agg>,
    ing: &RecipeIngredient,
    factor: Decimal,
    recipe_waste: Decimal,
    recipe_name: &str,
) {
    let Some(q) = ing.quantity else { return };
    let mut effective = q * factor;
    if let Some(py) = ing.prep_yield_percent {
        if py > Decimal::ZERO && py <= Decimal::from(100) {
            effective = effective / (py / Decimal::from(100));
        }
    }
    if recipe_waste > Decimal::ZERO {
        effective = effective * (Decimal::ONE + recipe_waste / Decimal::from(100));
    }

    let unit = ing.unit.as_deref().map(normalize_unit);
    let key = ing
        .master_ingredient_id
        .map(|id| format!("master:{id}"))
        .unwrap_or_else(|| {
            format!(
                "{}|{}",
                ing.ingredient.to_lowercase(),
                unit.as_deref().unwrap_or("")
            )
        });

    let entry = map.entry(key).or_insert_with(|| Agg {
        ingredient: ing.ingredient.clone(),
        master_name: ing.master_name.clone(),
        density_g_per_cup: ing.master_g_per_cup,
        total: Decimal::ZERO,
        unit: unit.clone(),
        recipes: Vec::new(),
    });
    if entry.density_g_per_cup.is_none() {
        entry.density_g_per_cup = ing.master_g_per_cup;
    }
    entry.total += effective;
    if !entry.recipes.iter().any(|r| r == recipe_name) {
        entry.recipes.push(recipe_name.to_string());
    }
}
