use anyhow::Result;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::ShoppingListItem;

#[derive(FromRow)]
struct ShoppingItemRow {
    id: String,
    user_id: String,
    item: String,
    quantity: Option<String>,
    unit: Option<String>,
    category: Option<String>,
    checked: i64,
    recipe_id: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

impl From<ShoppingItemRow> for ShoppingListItem {
    fn from(row: ShoppingItemRow) -> Self {
        Self {
            id: Uuid::parse_str(&row.id).unwrap_or_default(),
            user_id: Uuid::parse_str(&row.user_id).unwrap_or_default(),
            item: row.item,
            quantity: row.quantity.and_then(|q| q.parse().ok()),
            unit: row.unit,
            category: row.category,
            checked: row.checked != 0,
            recipe_id: row.recipe_id.and_then(|s| Uuid::parse_str(&s).ok()),
            created_at: row.created_at,
        }
    }
}

pub struct ShoppingListService {
    pool: SqlitePool,
}

impl ShoppingListService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_list(&self, user_id: Uuid) -> Result<Vec<ShoppingListItem>> {
        let rows: Vec<ShoppingItemRow> = sqlx::query_as(
            "SELECT * FROM shopping_list_items WHERE user_id = ? ORDER BY category, item",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(ShoppingListItem::from).collect())
    }

    pub async fn add_item(&self, item: &ShoppingListItem) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO shopping_list_items (id, user_id, item, quantity, unit, category, checked, recipe_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(id.to_string())
        .bind(item.user_id.to_string())
        .bind(&item.item)
        .bind(item.quantity.as_ref().map(|q| q.to_string()))
        .bind(&item.unit)
        .bind(&item.category)
        .bind(if item.checked { 1 } else { 0 })
        .bind(item.recipe_id.map(|r| r.to_string()))
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn toggle_checked(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE shopping_list_items SET checked = NOT checked WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn clear_checked(&self, user_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM shopping_list_items WHERE user_id = ? AND checked = 1")
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn clear_all(&self, user_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM shopping_list_items WHERE user_id = ?")
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn generate_from_recipes(&self, user_id: Uuid, recipe_ids: &[Uuid]) -> Result<usize> {
        let mut merged: HashMap<String, (String, Option<Decimal>, Option<String>, Option<String>)> =
            HashMap::new();

        for recipe_id in recipe_ids {
            let ingredients: Vec<(
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            )> = sqlx::query_as(
                "SELECT ingredient, display, quantity, unit, note, category FROM recipe_ingredients WHERE recipe_id = ?",
            )
            .bind(recipe_id.to_string())
            .fetch_all(&self.pool)
            .await?;

            for (ingredient, _display, quantity, unit, _note, category) in ingredients {
                let key = format!(
                    "{}|{}|{}",
                    ingredient.to_lowercase(),
                    unit.as_deref().unwrap_or(""),
                    category.as_deref().unwrap_or("")
                );
                let parsed_qty = quantity.as_deref().and_then(|q| q.parse::<Decimal>().ok());

                merged
                    .entry(key)
                    .and_modify(|(_, qty, _, _)| {
                        if let (Some(existing), Some(add)) = (qty.as_mut(), parsed_qty.as_ref()) {
                            *existing += add;
                        }
                    })
                    .or_insert((ingredient, parsed_qty, unit, category));
            }
        }

        let mut count = 0;
        for (_key, (ingredient, quantity, unit, category)) in merged {
            if self
                .upsert_ingredient(user_id, &ingredient, quantity, unit, category)
                .await?
            {
                count += 1;
            }
        }

        Ok(count)
    }

    async fn upsert_ingredient(
        &self,
        user_id: Uuid,
        ingredient: &str,
        quantity: Option<Decimal>,
        unit: Option<String>,
        category: Option<String>,
    ) -> Result<bool> {
        let existing: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT id, quantity FROM shopping_list_items
             WHERE user_id = ? AND checked = 0 AND lower(item) = lower(?) AND coalesce(unit, '') = coalesce(?, '')",
        )
        .bind(user_id.to_string())
        .bind(ingredient)
        .bind(&unit)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((id, existing_qty)) = existing {
            if let (Some(existing), Some(add)) = (
                existing_qty.and_then(|q| q.parse::<Decimal>().ok()),
                quantity,
            ) {
                let combined = existing + add;
                sqlx::query("UPDATE shopping_list_items SET quantity = ? WHERE id = ?")
                    .bind(combined.to_string())
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
            return Ok(false);
        }

        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO shopping_list_items (id, user_id, item, quantity, unit, category, checked, recipe_id) VALUES (?, ?, ?, ?, ?, ?, 0, NULL)",
        )
        .bind(id.to_string())
        .bind(user_id.to_string())
        .bind(ingredient)
        .bind(quantity.map(|q| q.to_string()))
        .bind(unit)
        .bind(category)
        .execute(&self.pool)
        .await?;

        Ok(true)
    }

    pub async fn generate_from_meal_plan(
        &self,
        user_id: Uuid,
        week_start: chrono::NaiveDate,
    ) -> Result<usize> {
        let week_end = week_start + chrono::Duration::days(6);

        let recipe_ids: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT recipe_id FROM meal_plans WHERE user_id = ? AND date BETWEEN ? AND ? AND recipe_id IS NOT NULL"
        )
        .bind(user_id.to_string())
        .bind(week_start.to_string())
        .bind(week_end.to_string())
        .fetch_all(&self.pool)
        .await?;

        let ids: Vec<Uuid> = recipe_ids
            .into_iter()
            .filter_map(|(s,)| Uuid::parse_str(&s).ok())
            .collect();

        if ids.is_empty() {
            return Ok(0);
        }

        self.generate_from_recipes(user_id, &ids).await
    }
}
