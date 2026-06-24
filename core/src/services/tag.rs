use anyhow::Result;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::Tag;

pub struct TagService {
    pool: SqlitePool,
}

impl TagService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> Result<Vec<Tag>> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, name, color FROM tags ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, name, color)| Tag {
                id: Uuid::parse_str(&id).unwrap_or_default(),
                name,
                color,
            })
            .collect())
    }

    pub async fn get_or_create(&self, name: &str) -> Result<Tag> {
        let normalized = name.trim().to_lowercase();
        if normalized.is_empty() {
            anyhow::bail!("Tag name cannot be empty");
        }

        if let Some(existing) = sqlx::query_as::<_, (String, String, Option<String>)>(
            "SELECT id, name, color FROM tags WHERE lower(name) = ?",
        )
        .bind(&normalized)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(Tag {
                id: Uuid::parse_str(&existing.0).unwrap_or_default(),
                name: existing.1,
                color: existing.2,
            });
        }

        let id = Uuid::new_v4();
        let display_name = name.trim().to_string();
        sqlx::query("INSERT INTO tags (id, name) VALUES (?, ?)")
            .bind(id.to_string())
            .bind(&display_name)
            .execute(&self.pool)
            .await?;

        Ok(Tag {
            id,
            name: display_name,
            color: None,
        })
    }

    pub async fn add_to_recipe(&self, recipe_id: Uuid, name: &str) -> Result<Tag> {
        let tag = self.get_or_create(name).await?;
        sqlx::query(
            "INSERT OR IGNORE INTO recipe_tags (recipe_id, tag_id) VALUES (?, ?)",
        )
        .bind(recipe_id.to_string())
        .bind(tag.id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(tag)
    }

    pub async fn remove_from_recipe(&self, recipe_id: Uuid, tag_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM recipe_tags WHERE recipe_id = ? AND tag_id = ?")
            .bind(recipe_id.to_string())
            .bind(tag_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
