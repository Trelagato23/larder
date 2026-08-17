use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::models::Location;

#[derive(Debug, Clone)]
pub struct LocationPriceExport {
    pub location_slug: String,
    pub ingredient_name: String,
    pub cost_per_unit: Option<Decimal>,
}

/// Elmwood Ave store
pub const ELMWOOD_LOCATION_ID: Uuid = Uuid::from_bytes([
    0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0x00, 0x01,
]);

/// Hertel Ave store
pub const HERTEL_LOCATION_ID: Uuid = Uuid::from_bytes([
    0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0x00, 0x02,
]);

pub struct LocationService {
    pool: SqlitePool,
}

#[derive(FromRow)]
struct LocationRow {
    id: String,
    slug: String,
    name: String,
    created_at: String,
}

impl LocationRow {
    fn into_location(self) -> Result<Location> {
        Ok(Location {
            id: Uuid::parse_str(&self.id).context("location id")?,
            slug: self.slug,
            name: self.name,
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

impl LocationService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> Result<Vec<Location>> {
        let rows: Vec<LocationRow> = sqlx::query_as(
            "SELECT id, slug, name, created_at FROM locations ORDER BY name COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(LocationRow::into_location).collect()
    }

    /// Locations assigned to user; if none assigned, return all (shared demo / legacy).
    pub async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Location>> {
        let rows: Vec<LocationRow> = sqlx::query_as(
            r#"
            SELECT l.id, l.slug, l.name, l.created_at
            FROM locations l
            INNER JOIN user_locations ul ON ul.location_id = l.id
            WHERE ul.user_id = ?
            ORDER BY l.name COLLATE NOCASE
            "#,
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return self.list_all().await;
        }
        rows.into_iter().map(LocationRow::into_location).collect()
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Location>> {
        let row: Option<LocationRow> = sqlx::query_as(
            "SELECT id, slug, name, created_at FROM locations WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(LocationRow::into_location).transpose()
    }

    pub async fn user_can_access(&self, user_id: Uuid, location_id: Uuid) -> Result<bool> {
        let assigned: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_locations WHERE user_id = ?",
        )
        .bind(user_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        if assigned.0 == 0 {
            return self.get(location_id).await.map(|l| l.is_some());
        }

        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_locations WHERE user_id = ? AND location_id = ?",
        )
        .bind(user_id.to_string())
        .bind(location_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(n > 0)
    }

    pub async fn ensure_seed_locations(&self) -> Result<()> {
        for (id, slug, name) in [
            (ELMWOOD_LOCATION_ID, "elmwood", "Elmwood"),
            (HERTEL_LOCATION_ID, "hertel", "Hertel"),
        ] {
            sqlx::query(
                "INSERT OR IGNORE INTO locations (id, slug, name) VALUES (?, ?, ?)",
            )
            .bind(id.to_string())
            .bind(slug)
            .bind(name)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn ensure_user_location(&self, user_id: Uuid, location_id: Uuid) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO user_locations (user_id, location_id) VALUES (?, ?)",
        )
        .bind(user_id.to_string())
        .bind(location_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<Location>> {
        let row: Option<LocationRow> = sqlx::query_as(
            "SELECT id, slug, name, created_at FROM locations WHERE slug = ?",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;
        row.map(LocationRow::into_location).transpose()
    }

    pub async fn list_prices_for_export(&self) -> Result<Vec<LocationPriceExport>> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT l.slug, i.name, lip.cost_per_unit
            FROM location_ingredient_prices lip
            JOIN locations l ON l.id = lip.location_id
            JOIN ingredients i ON i.id = lip.ingredient_id
            ORDER BY l.slug, i.name COLLATE NOCASE
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(location_slug, ingredient_name, cost)| LocationPriceExport {
                location_slug,
                ingredient_name,
                cost_per_unit: cost.and_then(|p| p.parse().ok()),
            })
            .collect())
    }

    pub async fn set_ingredient_price(
        &self,
        location_id: Uuid,
        ingredient_id: Uuid,
        cost_per_unit: Option<Decimal>,
    ) -> Result<()> {
        if cost_per_unit.is_none() {
            sqlx::query(
                "DELETE FROM location_ingredient_prices WHERE location_id = ? AND ingredient_id = ?",
            )
            .bind(location_id.to_string())
            .bind(ingredient_id.to_string())
            .execute(&self.pool)
            .await?;
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO location_ingredient_prices (location_id, ingredient_id, cost_per_unit, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(location_id, ingredient_id) DO UPDATE SET
                cost_per_unit = excluded.cost_per_unit,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(location_id.to_string())
        .bind(ingredient_id.to_string())
        .bind(cost_per_unit.map(|p| p.to_string()))
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_location_price(
        &self,
        location_id: Uuid,
        ingredient_id: Uuid,
    ) -> Result<Option<Decimal>> {
        let row: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT cost_per_unit FROM location_ingredient_prices WHERE location_id = ? AND ingredient_id = ?",
        )
        .bind(location_id.to_string())
        .bind(ingredient_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|(p,)| p.and_then(|s| s.parse().ok())))
    }
}
