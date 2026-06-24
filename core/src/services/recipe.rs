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
    user_id: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
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
}

impl From<IngredientRow> for RecipeIngredient {
    fn from(row: IngredientRow) -> Self {
        Self {
            id: Uuid::parse_str(&row.id).unwrap_or_default(),
            recipe_id: Uuid::parse_str(&row.recipe_id).unwrap_or_default(),
            ingredient: row.ingredient,
            quantity: row.quantity.and_then(|q| q.parse().ok()),
            unit: row.unit,
            note: row.note,
            display: row.display,
            category: row.category,
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
            "INSERT INTO recipes (id, name, description, image_url, servings, prep_time_minutes, cook_time_minutes, total_time_minutes, source_url, rating, difficulty, user_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
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
        .bind(recipe.user_id.to_string())
        .execute(&self.pool)
        .await?;

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
        let rows: Vec<RecipeRow> = sqlx::query_as(
            "SELECT * FROM recipes WHERE user_id = ? ORDER BY updated_at DESC"
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Recipe::from).collect())
    }

    pub async fn search_recipes(&self, query: &str) -> Result<Vec<Recipe>> {
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

    pub async fn list_recipes_by_tag(&self, tag_name: &str) -> Result<Vec<Recipe>> {
        let pattern = format!("%{}%", tag_name.trim().to_lowercase());
        let rows: Vec<RecipeRow> = sqlx::query_as(
            "SELECT r.* FROM recipes r
             JOIN recipe_tags rt ON r.id = rt.recipe_id
             JOIN tags t ON t.id = rt.tag_id
             WHERE lower(t.name) LIKE ?
             ORDER BY r.name",
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
            "UPDATE recipes SET name = ?, description = ?, image_url = ?, servings = ?, prep_time_minutes = ?, cook_time_minutes = ?, total_time_minutes = ?, source_url = ?, rating = ?, difficulty = ?, updated_at = datetime('now') WHERE id = ?"
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
        .bind(recipe.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_recipe(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM recipes WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_ingredients(&self, recipe_id: Uuid) -> Result<Vec<RecipeIngredient>> {
        let rows: Vec<IngredientRow> = sqlx::query_as(
            "SELECT * FROM recipe_ingredients WHERE recipe_id = ? ORDER BY rowid"
        )
        .bind(recipe_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(RecipeIngredient::from).collect())
    }

    pub async fn add_ingredient(&self, ingredient: &RecipeIngredient) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO recipe_ingredients (id, recipe_id, ingredient, quantity, unit, note, display, category) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(id.to_string())
        .bind(ingredient.recipe_id.to_string())
        .bind(&ingredient.ingredient)
        .bind(ingredient.quantity.as_ref().map(|q| q.to_string()))
        .bind(&ingredient.unit)
        .bind(&ingredient.note)
        .bind(&ingredient.display)
        .bind(&ingredient.category)
        .execute(&self.pool)
        .await?;

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
        Ok(())
    }

    pub async fn clear_steps(&self, recipe_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM recipe_steps WHERE recipe_id = ?")
            .bind(recipe_id.to_string())
            .execute(&self.pool)
            .await?;
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
}
