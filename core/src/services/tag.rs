use anyhow::Result;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::Tag;

/// Cold case — grab-and-go line. Mutually exclusive with hot-bar.
const GRAB_GO_CANON: &str = "grab-go";
/// Hot line. Mutually exclusive with grab-go.
const HOT_BAR_CANON: &str = "hot-bar";

/// Tag names treated as cold grab-and-go (for mutual exclusion).
const GRAB_GO_FAMILY: &[&str] = &["grab-go", "grab-and-go"];
/// Tag names treated as hot bar (for mutual exclusion / aliases).
const HOT_BAR_FAMILY: &[&str] = &["hot-bar", "grab-go-hot-bar"];

pub struct TagService {
    pool: SqlitePool,
}

impl TagService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Canonical kitchen tag names (aliases collapse here).
    pub fn normalize_name(name: &str) -> String {
        let n = name.trim().to_lowercase();
        match n.as_str() {
            "grab-and-go" => GRAB_GO_CANON.to_string(),
            // Hybrid ChefTec tag = hot line, not cold case
            "grab-go-hot-bar" => HOT_BAR_CANON.to_string(),
            "hb-menus" => HOT_BAR_CANON.to_string(),
            "dressing" => "dressings".to_string(),
            "bakery-grab-go" => "bakery".to_string(),
            "grab-go-sandwiches" => "sandwiches".to_string(),
            "grab-go-pizza" => "pizza".to_string(),
            "cheese-grab-go" => GRAB_GO_CANON.to_string(),
            other => other.to_string(),
        }
    }

    fn is_grab_go(name: &str) -> bool {
        GRAB_GO_FAMILY.contains(&name) || name == GRAB_GO_CANON
    }

    fn is_hot_bar(name: &str) -> bool {
        HOT_BAR_FAMILY.contains(&name) || name == HOT_BAR_CANON
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
        let normalized = Self::normalize_name(name);
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
        // Store canonical lowercase so chips match QUICK_DEPT keys.
        sqlx::query("INSERT INTO tags (id, name) VALUES (?, ?)")
            .bind(id.to_string())
            .bind(&normalized)
            .execute(&self.pool)
            .await?;

        Ok(Tag {
            id,
            name: normalized,
            color: None,
        })
    }

    /// Remove every tag whose lower(name) is in `names` from a recipe.
    async fn remove_names_from_recipe(&self, recipe_id: Uuid, names: &[&str]) -> Result<()> {
        for name in names {
            sqlx::query(
                r#"
                DELETE FROM recipe_tags
                WHERE recipe_id = ?
                  AND tag_id IN (SELECT id FROM tags WHERE lower(name) = ?)
                "#,
            )
            .bind(recipe_id.to_string())
            .bind(name)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn add_to_recipe(&self, recipe_id: Uuid, name: &str) -> Result<Tag> {
        let tag = self.get_or_create(name).await?;
        let key = tag.name.to_lowercase();

        // Grab & Go (cold) and Hot Bar (hot) cannot both be on a recipe.
        if Self::is_hot_bar(&key) {
            self.remove_names_from_recipe(recipe_id, GRAB_GO_FAMILY)
                .await?;
            // Drop hybrid leftover if present under old id
            self.remove_names_from_recipe(recipe_id, &["grab-go-hot-bar"])
                .await?;
        } else if Self::is_grab_go(&key) {
            self.remove_names_from_recipe(recipe_id, HOT_BAR_FAMILY)
                .await?;
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_aliases() {
        assert_eq!(TagService::normalize_name("Grab-and-Go"), "grab-go");
        assert_eq!(TagService::normalize_name("grab-go-hot-bar"), "hot-bar");
        assert_eq!(TagService::normalize_name("  HOT-BAR "), "hot-bar");
        assert_eq!(TagService::normalize_name("soups"), "soups");
    }

    #[test]
    fn stations_are_exclusive_families() {
        assert!(TagService::is_grab_go("grab-go"));
        assert!(TagService::is_grab_go("grab-and-go"));
        assert!(!TagService::is_grab_go("hot-bar"));
        assert!(TagService::is_hot_bar("hot-bar"));
        assert!(TagService::is_hot_bar("grab-go-hot-bar"));
        assert!(!TagService::is_hot_bar("grab-go"));
    }
}
