use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};
use tracing::info;
use uuid::Uuid;

use crate::models::{Difficulty, Recipe, RecipeIngredient, RecipeStep, Tag};

#[derive(FromRow)]
struct RecipeRow {
    id: String,
    name: String,
    description: Option<String>,
    image_url: Option<String>,
    servings: i64,
    prep_time_minutes: Option<i64>,
    cook_time_minutes: Option<i64>,
    total_time_minutes: Option<i64>,
    source_url: Option<String>,
    rating: Option<i64>,
    difficulty: Option<String>,
    menu_price: Option<String>,
    user_id: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    yield_quantity: Option<String>,
    yield_unit: Option<String>,
    waste_percent: Option<String>,
    author: Option<String>,
    estimated_calories: Option<i64>,
    allergens: Option<String>,
    last_opened_at: Option<DateTime<Utc>>,
    open_count: Option<i64>,
}

impl From<RecipeRow> for Recipe {
    fn from(row: RecipeRow) -> Self {
        Self {
            id: Uuid::parse_str(&row.id).unwrap_or_default(),
            name: row.name,
            description: row.description,
            image_url: row.image_url,
            servings: row.servings as u32,
            prep_time_minutes: row.prep_time_minutes.map(|v| v as u32),
            cook_time_minutes: row.cook_time_minutes.map(|v| v as u32),
            total_time_minutes: row.total_time_minutes.map(|v| v as u32),
            source_url: row.source_url,
            created_at: row.created_at,
            updated_at: row.updated_at,
            user_id: Uuid::parse_str(&row.user_id).unwrap_or_default(),
            rating: row.rating.map(|v| v as u8),
            difficulty: row.difficulty.and_then(|d| match d.as_str() {
                "easy" => Some(Difficulty::Easy),
                "medium" => Some(Difficulty::Medium),
                "hard" => Some(Difficulty::Hard),
                _ => None,
            }),
            menu_price: row.menu_price.and_then(|p| p.parse().ok()),
            yield_quantity: row.yield_quantity.and_then(|p| p.parse().ok()),
            yield_unit: row.yield_unit,
            waste_percent: row.waste_percent.and_then(|p| p.parse().ok()),
            author: row.author,
            estimated_calories: row.estimated_calories.map(|v| v as u32),
            allergens: row.allergens,
            last_opened_at: row.last_opened_at,
            open_count: row.open_count.map(|c| c.max(0) as u32),
        }
    }
}

#[derive(FromRow)]
struct IngredientRow {
    id: String,
    recipe_id: String,
    ingredient: String,
    quantity: Option<String>,
    unit: Option<String>,
    note: Option<String>,
    display: String,
    category: Option<String>,
    cost_per_unit: Option<String>,
    line_cost: Option<String>,
    master_ingredient_id: Option<String>,
    master_cost_per_unit: Option<String>,
    master_name: Option<String>,
    master_g_per_cup: Option<String>,
    prep_yield_percent: Option<String>,
}

impl From<IngredientRow> for RecipeIngredient {
    fn from(row: IngredientRow) -> Self {
        let master_id = row
            .master_ingredient_id
            .as_ref()
            .and_then(|s| Uuid::parse_str(s).ok());
        // Roll-through: master unit cost wins when linked
        let cost_per_unit = row
            .master_cost_per_unit
            .and_then(|p| p.parse().ok())
            .or_else(|| row.cost_per_unit.and_then(|p| p.parse().ok()));
        Self {
            id: Uuid::parse_str(&row.id).unwrap_or_default(),
            recipe_id: Uuid::parse_str(&row.recipe_id).unwrap_or_default(),
            ingredient: row.ingredient,
            quantity: row.quantity.and_then(|q| q.parse().ok()),
            unit: row.unit,
            note: row.note,
            display: row.display,
            category: row.category,
            cost_per_unit,
            line_cost: row.line_cost.and_then(|p| p.parse().ok()),
            master_ingredient_id: master_id,
            master_name: row.master_name,
            master_g_per_cup: row.master_g_per_cup.and_then(|p| p.parse().ok()),
            prep_yield_percent: row.prep_yield_percent.and_then(|p| p.parse().ok()),
        }
    }
}

#[derive(FromRow)]
struct StepRow {
    id: String,
    recipe_id: String,
    position: i64,
    instruction: String,
    timer_seconds: Option<i64>,
}

impl From<StepRow> for RecipeStep {
    fn from(row: StepRow) -> Self {
        Self {
            id: Uuid::parse_str(&row.id).unwrap_or_default(),
            recipe_id: Uuid::parse_str(&row.recipe_id).unwrap_or_default(),
            position: row.position as u32,
            instruction: row.instruction,
            timer_seconds: row.timer_seconds.map(|v| v as u32),
        }
    }
}

pub struct RecipeService {
    pool: SqlitePool,
}

impl RecipeService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_recipe(&self, recipe: &Recipe) -> Result<Uuid> {
        info!("Creating recipe: {}", recipe.name);

        let id = Uuid::new_v4();
        let difficulty_str = recipe.difficulty.map(|d| match d {
            Difficulty::Easy => "easy",
            Difficulty::Medium => "medium",
            Difficulty::Hard => "hard",
        });

        sqlx::query(
            "INSERT INTO recipes (id, name, description, image_url, servings, prep_time_minutes, cook_time_minutes, total_time_minutes, source_url, rating, difficulty, menu_price, yield_quantity, yield_unit, waste_percent, author, estimated_calories, allergens, user_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(id.to_string())
        .bind(&recipe.name)
        .bind(&recipe.description)
        .bind(&recipe.image_url)
        .bind(recipe.servings as i64)
        .bind(recipe.prep_time_minutes.map(|v| v as i64))
        .bind(recipe.cook_time_minutes.map(|v| v as i64))
        .bind(recipe.total_time_minutes.map(|v| v as i64))
        .bind(&recipe.source_url)
        .bind(recipe.rating.map(|v| v as i64))
        .bind(difficulty_str)
        .bind(recipe.menu_price.as_ref().map(|p| p.to_string()))
        .bind(recipe.yield_quantity.as_ref().map(|p| p.to_string()))
        .bind(&recipe.yield_unit)
        .bind(recipe.waste_percent.as_ref().map(|p| p.to_string()))
        .bind(&recipe.author)
        .bind(recipe.estimated_calories.map(|v| v as i64))
        .bind(&recipe.allergens)
        .bind(recipe.user_id.to_string())
        .execute(&self.pool)
        .await?;

        let _ = self.upsert_fts_for_recipe(id).await;
        Ok(id)
    }

    pub async fn get_recipe(&self, id: Uuid) -> Result<Option<Recipe>> {
        let row: Option<RecipeRow> = sqlx::query_as(
            "SELECT * FROM recipes WHERE id = ?"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Recipe::from))
    }

    pub async fn list_recipes(&self, user_id: Uuid) -> Result<Vec<Recipe>> {
        // Frecency-ish: recently opened first; never-opened sinks; then opens, then name.
        let rows: Vec<RecipeRow> = sqlx::query_as(
            r#"
            SELECT * FROM recipes
            WHERE user_id = ?
            ORDER BY
                (last_opened_at IS NULL) ASC,
                last_opened_at DESC,
                COALESCE(open_count, 0) DESC,
                name COLLATE NOCASE ASC
            "#,
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Recipe::from).collect())
    }

    /// Record a kitchen/office open of the recipe detail view.
    pub async fn record_open(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE recipes
            SET last_opened_at = datetime('now'),
                open_count = COALESCE(open_count, 0) + 1
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn search_recipes(&self, query: &str) -> Result<Vec<Recipe>> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }

        if let Some(fts_q) = build_fts_query(q) {
            match self.search_recipes_fts(&fts_q).await {
                Ok(rows) => return Ok(rows),
                Err(e) => {
                    tracing::debug!("FTS search falling back to LIKE: {e}");
                }
            }
        }

        self.search_recipes_like(q).await
    }

    async fn search_recipes_fts(&self, fts_query: &str) -> Result<Vec<Recipe>> {
        let rows: Vec<RecipeRow> = sqlx::query_as(
            "SELECT r.*
             FROM recipe_fts f
             JOIN recipes r ON r.id = f.recipe_id
             WHERE recipe_fts MATCH ?
             ORDER BY bm25(recipe_fts), r.name",
        )
        .bind(fts_query)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Recipe::from).collect())
    }

    async fn search_recipes_like(&self, query: &str) -> Result<Vec<Recipe>> {
        let pattern = format!("%{}%", query);
        let rows: Vec<RecipeRow> = sqlx::query_as(
            "SELECT DISTINCT r.* FROM recipes r
             LEFT JOIN recipe_ingredients ri ON r.id = ri.recipe_id
             WHERE r.name LIKE ? OR r.description LIKE ?
                OR ri.ingredient LIKE ? OR ri.display LIKE ?
             ORDER BY r.name",
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Recipe::from).collect())
    }

    /// Drop and rebuild the entire FTS index from recipes + ingredients.
    pub async fn rebuild_fts(&self) -> Result<()> {
        // No-op if virtual table is missing (older DBs mid-migration).
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'recipe_fts'")
                .fetch_optional(&self.pool)
                .await?;
        if exists.is_none() {
            return Ok(());
        }

        sqlx::query("DELETE FROM recipe_fts")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            INSERT INTO recipe_fts(recipe_id, name, description, ingredients)
            SELECT
                r.id,
                r.name,
                coalesce(r.description, ''),
                coalesce((
                    SELECT group_concat(ri.display || ' ' || ri.ingredient, ' ')
                    FROM recipe_ingredients ri
                    WHERE ri.recipe_id = r.id
                ), '')
            FROM recipes r
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn upsert_fts_for_recipe(&self, recipe_id: Uuid) -> Result<()> {
        let id = recipe_id.to_string();
        let row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT name, description FROM recipes WHERE id = ?")
                .bind(&id)
                .fetch_optional(&self.pool)
                .await?;
        let Some((name, description)) = row else {
            sqlx::query("DELETE FROM recipe_fts WHERE recipe_id = ?")
                .bind(&id)
                .execute(&self.pool)
                .await
                .ok();
            return Ok(());
        };
        let ingredients: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT group_concat(display || ' ' || ingredient, ' ')
             FROM recipe_ingredients WHERE recipe_id = ?",
        )
        .bind(&id)
        .fetch_optional(&self.pool)
        .await?;
        let ingredients = ingredients
            .and_then(|(s,)| s)
            .unwrap_or_default();

        sqlx::query("DELETE FROM recipe_fts WHERE recipe_id = ?")
            .bind(&id)
            .execute(&self.pool)
            .await
            .ok();
        let _ = sqlx::query(
            "INSERT INTO recipe_fts(recipe_id, name, description, ingredients) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&name)
        .bind(description.unwrap_or_default())
        .bind(ingredients)
        .execute(&self.pool)
        .await;

        Ok(())
    }

    pub async fn list_recipes_by_tag(&self, tag_name: &str) -> Result<Vec<Recipe>> {
        let pattern = format!("%{}%", tag_name.trim().to_lowercase());
        let rows: Vec<RecipeRow> = sqlx::query_as(
            "SELECT r.* FROM recipes r
             JOIN recipe_tags rt ON r.id = rt.recipe_id
             JOIN tags t ON t.id = rt.tag_id
             WHERE lower(t.name) LIKE ?
             ORDER BY
                (r.last_opened_at IS NULL) ASC,
                r.last_opened_at DESC,
                COALESCE(r.open_count, 0) DESC,
                r.name COLLATE NOCASE ASC",
        )
        .bind(pattern)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Recipe::from).collect())
    }

    pub async fn update_recipe(&self, recipe: &Recipe) -> Result<()> {
        let difficulty_str = recipe.difficulty.map(|d| match d {
            Difficulty::Easy => "easy",
            Difficulty::Medium => "medium",
            Difficulty::Hard => "hard",
        });

        sqlx::query(
            "UPDATE recipes SET name = ?, description = ?, image_url = ?, servings = ?, prep_time_minutes = ?, cook_time_minutes = ?, total_time_minutes = ?, source_url = ?, rating = ?, difficulty = ?, menu_price = ?, yield_quantity = ?, yield_unit = ?, waste_percent = ?, author = ?, estimated_calories = ?, allergens = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(&recipe.name)
        .bind(&recipe.description)
        .bind(&recipe.image_url)
        .bind(recipe.servings as i64)
        .bind(recipe.prep_time_minutes.map(|v| v as i64))
        .bind(recipe.cook_time_minutes.map(|v| v as i64))
        .bind(recipe.total_time_minutes.map(|v| v as i64))
        .bind(&recipe.source_url)
        .bind(recipe.rating.map(|v| v as i64))
        .bind(difficulty_str)
        .bind(recipe.menu_price.as_ref().map(|p| p.to_string()))
        .bind(recipe.yield_quantity.as_ref().map(|p| p.to_string()))
        .bind(&recipe.yield_unit)
        .bind(recipe.waste_percent.as_ref().map(|p| p.to_string()))
        .bind(&recipe.author)
        .bind(recipe.estimated_calories.map(|v| v as i64))
        .bind(&recipe.allergens)
        .bind(recipe.id.to_string())
        .execute(&self.pool)
        .await?;

        let _ = self.upsert_fts_for_recipe(recipe.id).await;
        Ok(())
    }

    pub async fn delete_recipe(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM recipes WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        let _ = sqlx::query("DELETE FROM recipe_fts WHERE recipe_id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await;
        Ok(())
    }

    pub async fn get_ingredients(
        &self,
        recipe_id: Uuid,
        location_id: Option<Uuid>,
    ) -> Result<Vec<RecipeIngredient>> {
        let rows: Vec<IngredientRow> = if let Some(loc_id) = location_id {
            sqlx::query_as(
                r#"
                SELECT
                    ri.id, ri.recipe_id, ri.ingredient, ri.quantity, ri.unit, ri.note,
                    ri.display, ri.category, ri.cost_per_unit, ri.line_cost,
                    ri.master_ingredient_id, ri.prep_yield_percent,
                    COALESCE(loc.cost_per_unit, m.cost_per_unit) AS master_cost_per_unit,
                    m.name AS master_name,
                    m.g_per_cup AS master_g_per_cup
                FROM recipe_ingredients ri
                LEFT JOIN ingredients m ON m.id = ri.master_ingredient_id
                LEFT JOIN location_ingredient_prices loc
                    ON loc.ingredient_id = m.id AND loc.location_id = ?
                WHERE ri.recipe_id = ?
                ORDER BY ri.rowid
                "#,
            )
            .bind(loc_id.to_string())
            .bind(recipe_id.to_string())
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    ri.id, ri.recipe_id, ri.ingredient, ri.quantity, ri.unit, ri.note,
                    ri.display, ri.category, ri.cost_per_unit, ri.line_cost,
                    ri.master_ingredient_id, ri.prep_yield_percent,
                    m.cost_per_unit AS master_cost_per_unit,
                    m.name AS master_name,
                    m.g_per_cup AS master_g_per_cup
                FROM recipe_ingredients ri
                LEFT JOIN ingredients m ON m.id = ri.master_ingredient_id
                WHERE ri.recipe_id = ?
                ORDER BY ri.rowid
                "#,
            )
            .bind(recipe_id.to_string())
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.into_iter().map(RecipeIngredient::from).collect())
    }

    pub async fn add_ingredient(&self, ingredient: &RecipeIngredient) -> Result<Uuid> {
        let id = if ingredient.id.is_nil() {
            Uuid::new_v4()
        } else {
            ingredient.id
        };
        sqlx::query(
            r#"
            INSERT INTO recipe_ingredients (
                id, recipe_id, ingredient, quantity, unit, note, display, category,
                cost_per_unit, line_cost, master_ingredient_id, prep_yield_percent
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(ingredient.recipe_id.to_string())
        .bind(&ingredient.ingredient)
        .bind(ingredient.quantity.as_ref().map(|q| q.to_string()))
        .bind(&ingredient.unit)
        .bind(&ingredient.note)
        .bind(&ingredient.display)
        .bind(&ingredient.category)
        .bind(ingredient.cost_per_unit.as_ref().map(|p| p.to_string()))
        .bind(ingredient.line_cost.as_ref().map(|p| p.to_string()))
        .bind(
            ingredient
                .master_ingredient_id
                .as_ref()
                .map(|id| id.to_string()),
        )
        .bind(
            ingredient
                .prep_yield_percent
                .as_ref()
                .map(|p| p.to_string()),
        )
        .execute(&self.pool)
        .await?;

        let _ = self.upsert_fts_for_recipe(ingredient.recipe_id).await;
        Ok(id)
    }

    pub async fn get_steps(&self, recipe_id: Uuid) -> Result<Vec<RecipeStep>> {
        let rows: Vec<StepRow> = sqlx::query_as(
            "SELECT * FROM recipe_steps WHERE recipe_id = ? ORDER BY position"
        )
        .bind(recipe_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(RecipeStep::from).collect())
    }

    pub async fn add_step(&self, step: &RecipeStep) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO recipe_steps (id, recipe_id, position, instruction, timer_seconds) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(id.to_string())
        .bind(step.recipe_id.to_string())
        .bind(step.position as i64)
        .bind(&step.instruction)
        .bind(step.timer_seconds.map(|v| v as i64))
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn add_tags(&self, recipe_id: Uuid, tags: Vec<Tag>) -> Result<()> {
        for tag in tags {
            sqlx::query(
                "INSERT OR IGNORE INTO tags (id, name, color) VALUES (?, ?, ?)"
            )
            .bind(tag.id.to_string())
            .bind(&tag.name)
            .bind(&tag.color)
            .execute(&self.pool)
            .await?;

            sqlx::query(
                "INSERT OR IGNORE INTO recipe_tags (recipe_id, tag_id) VALUES (?, ?)"
            )
            .bind(recipe_id.to_string())
            .bind(tag.id.to_string())
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn clear_ingredients(&self, recipe_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM recipe_ingredients WHERE recipe_id = ?")
            .bind(recipe_id.to_string())
            .execute(&self.pool)
            .await?;
        let _ = self.upsert_fts_for_recipe(recipe_id).await;
        Ok(())
    }

    pub async fn clear_steps(&self, recipe_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM recipe_steps WHERE recipe_id = ?")
            .bind(recipe_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn replace_ingredients(
        &self,
        recipe_id: Uuid,
        ingredients: &[RecipeIngredient],
    ) -> Result<()> {
        self.clear_ingredients(recipe_id).await?;
        for ing in ingredients {
            self.add_ingredient(ing).await?;
        }
        let _ = self.upsert_fts_for_recipe(recipe_id).await;
        Ok(())
    }

    pub async fn replace_steps(&self, recipe_id: Uuid, steps: &[RecipeStep]) -> Result<()> {
        self.clear_steps(recipe_id).await?;
        for step in steps {
            self.add_step(step).await?;
        }
        Ok(())
    }

    pub async fn get_tags(&self, recipe_id: Uuid) -> Result<Vec<Tag>> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT t.id, t.name, t.color FROM tags t JOIN recipe_tags rt ON t.id = rt.tag_id WHERE rt.recipe_id = ?"
        )
        .bind(recipe_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(id, name, color)| Tag {
            id: Uuid::parse_str(&id).unwrap_or_default(),
            name,
            color,
        }).collect())
    }

    /// All tag names keyed by recipe id (for list views / dept stripes).
    pub async fn tags_by_recipe_ids(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<String>>> {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        for &id in ids {
            let tags = self.get_tags(id).await?;
            if !tags.is_empty() {
                map.insert(id, tags.into_iter().map(|t| t.name).collect());
            }
        }
        Ok(map)
    }

    /// Recipes that **directly** relate via ingredient lines:
    /// - `uses`: this recipe's ingredient name matches another recipe name
    /// - `used_in`: other recipes list this recipe as an ingredient
    ///
    /// No soft "similar tags" matching — only name equality after light normalize.
    pub async fn related_recipes(&self, recipe_id: Uuid) -> Result<RelatedRecipes> {
        use std::collections::{HashMap, HashSet};

        let me = match self.get_recipe(recipe_id).await? {
            Some(r) => r,
            None => {
                return Ok(RelatedRecipes {
                    uses: vec![],
                    used_in: vec![],
                });
            }
        };
        let me_key = normalize_recipe_key(&me.name);
        if me_key.is_empty() {
            return Ok(RelatedRecipes {
                uses: vec![],
                used_in: vec![],
            });
        }

        let all_rows: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM recipes")
                .fetch_all(&self.pool)
                .await?;

        let mut by_key: HashMap<String, (Uuid, String)> = HashMap::new();
        for (id_s, name) in &all_rows {
            let id = Uuid::parse_str(id_s).unwrap_or_default();
            if id.is_nil() {
                continue;
            }
            let key = normalize_recipe_key(name);
            if key.is_empty() {
                continue;
            }
            // Prefer exact unique names; first wins if duplicates
            by_key.entry(key).or_insert((id, name.clone()));
        }

        let my_ings: Vec<(String, String)> = sqlx::query_as(
            "SELECT ingredient, coalesce(display, '') FROM recipe_ingredients WHERE recipe_id = ?",
        )
        .bind(recipe_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut uses = Vec::new();
        let mut seen_uses: HashSet<Uuid> = HashSet::new();
        for (ingredient, display) in &my_ings {
            for raw in [ingredient.as_str(), display.as_str()] {
                let key = normalize_recipe_key(raw);
                if key.is_empty() || key == me_key {
                    continue;
                }
                if let Some((id, name)) = by_key.get(&key) {
                    if *id != recipe_id && seen_uses.insert(*id) {
                        uses.push(RelatedRecipeLink {
                            id: *id,
                            name: name.clone(),
                            via: ingredient.clone(),
                        });
                    }
                }
            }
        }
        uses.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        // Reverse: ingredient lines that name this recipe
        let others: Vec<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT r.id, r.name, ri.ingredient
            FROM recipe_ingredients ri
            JOIN recipes r ON r.id = ri.recipe_id
            WHERE ri.recipe_id != ?
            "#,
        )
        .bind(recipe_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut used_in = Vec::new();
        let mut seen_in: HashSet<Uuid> = HashSet::new();
        for (id_s, name, ingredient) in others {
            let key = normalize_recipe_key(&ingredient);
            if key != me_key {
                continue;
            }
            let id = Uuid::parse_str(&id_s).unwrap_or_default();
            if id.is_nil() || !seen_in.insert(id) {
                continue;
            }
            used_in.push(RelatedRecipeLink {
                id,
                name,
                via: ingredient,
            });
        }
        used_in.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Ok(RelatedRecipes { uses, used_in })
    }
}

/// Link to a recipe that appears as (or uses) a direct ingredient component.
#[derive(Debug, Clone)]
pub struct RelatedRecipeLink {
    pub id: Uuid,
    pub name: String,
    /// Ingredient text that established the link.
    pub via: String,
}

#[derive(Debug, Clone, Default)]
pub struct RelatedRecipes {
    /// Sub-recipes this recipe's ingredient list points at.
    pub uses: Vec<RelatedRecipeLink>,
    /// Parent recipes that include this recipe as an ingredient.
    pub used_in: Vec<RelatedRecipeLink>,
}

/// Normalize for recipe↔ingredient name equality (direct links only).
fn normalize_recipe_key(s: &str) -> String {
    let mut s = s.to_lowercase();
    // Strip common ChefTec parentheticals: "(see recipe)", "(homemade)", "(for packing out)" only when see-recipe-like
    loop {
        let trimmed = s.trim();
        if let Some(start) = trimmed.rfind('(') {
            if trimmed.ends_with(')') {
                let inner = &trimmed[start + 1..trimmed.len() - 1];
                let drop = inner.contains("see recipe")
                    || inner == "homemade"
                    || inner.starts_with("see ")
                    || inner == "recipe";
                if drop {
                    s = trimmed[..start].to_string();
                    continue;
                }
            }
        }
        s = trimmed.to_string();
        break;
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Build an FTS5 MATCH query with prefix terms. Returns None if nothing searchable remains.
fn build_fts_query(raw: &str) -> Option<String> {
    let terms: Vec<String> = raw
        .split_whitespace()
        .filter_map(|term| {
            let cleaned: String = term
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if cleaned.is_empty() {
                None
            } else {
                Some(format!("{cleaned}*"))
            }
        })
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::{build_fts_query, normalize_recipe_key};

    #[test]
    fn fts_query_prefix_tokens() {
        assert_eq!(build_fts_query("chicken soup").as_deref(), Some("chicken* soup*"));
        assert_eq!(build_fts_query("  PB! ").as_deref(), Some("PB*"));
        assert_eq!(build_fts_query("!!!").as_deref(), None);
    }

    #[test]
    fn recipe_key_strips_see_recipe_notes() {
        assert_eq!(
            normalize_recipe_key("Maitre D'Hotel Butter (see recipe)"),
            "maitre d'hotel butter"
        );
        assert_eq!(normalize_recipe_key("  Pie   Dough  "), "pie dough");
        assert_eq!(normalize_recipe_key("Paneer (homemade)"), "paneer");
    }
}
