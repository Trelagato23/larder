use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::models::{Cookbook, CookbookRecipe};

pub struct CookbookService {
    pool: sqlx::SqlitePool,
}

impl CookbookService {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_cookbook(&self, name: &str, description: &str, user_id: Uuid, public: bool) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO cookbooks (id, name, description, user_id, public) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(id.to_string())
        .bind(name)
        .bind(description)
        .bind(user_id.to_string())
        .bind(if public { 1 } else { 0 })
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn list_cookbooks(&self, user_id: Uuid) -> Result<Vec<Cookbook>> {
        let rows: Vec<(String, String, Option<String>, String, i64, chrono::DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, name, description, user_id, public, created_at FROM cookbooks WHERE user_id = ? ORDER BY name"
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(id, name, desc, uid, pub_val, created)| Cookbook {
            id: Uuid::parse_str(&id).unwrap_or_default(),
            name,
            description: desc,
            user_id: Uuid::parse_str(&uid).unwrap_or_default(),
            public: pub_val != 0,
            created_at: created,
        }).collect())
    }

    pub async fn add_recipe(&self, cookbook_id: Uuid, recipe_id: Uuid, position: u32) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO cookbook_recipes (cookbook_id, recipe_id, position) VALUES (?, ?, ?)"
        )
        .bind(cookbook_id.to_string())
        .bind(recipe_id.to_string())
        .bind(position as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_recipe(&self, cookbook_id: Uuid, recipe_id: Uuid) -> Result<()> {
        sqlx::query(
            "DELETE FROM cookbook_recipes WHERE cookbook_id = ? AND recipe_id = ?"
        )
        .bind(cookbook_id.to_string())
        .bind(recipe_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_recipes(&self, cookbook_id: Uuid) -> Result<Vec<CookbookRecipe>> {
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT cookbook_id, recipe_id, position FROM cookbook_recipes WHERE cookbook_id = ? ORDER BY position"
        )
        .bind(cookbook_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(cid, rid, pos)| CookbookRecipe {
            cookbook_id: Uuid::parse_str(&cid).unwrap_or_default(),
            recipe_id: Uuid::parse_str(&rid).unwrap_or_default(),
            position: pos as u32,
        }).collect())
    }

    pub async fn delete_cookbook(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM cookbooks WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
