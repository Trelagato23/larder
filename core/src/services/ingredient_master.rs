use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::models::{MasterIngredient, name_key};

pub struct IngredientMasterService {
    pool: SqlitePool,
}

#[derive(FromRow)]
struct MasterRow {
    id: String,
    name: String,
    default_unit: Option<String>,
    cost_per_unit: Option<String>,
    pack_size: Option<String>,
    pack_unit: Option<String>,
    notes: Option<String>,
    g_per_cup: Option<String>,
    created_at: String,
    updated_at: String,
}

impl MasterRow {
    fn into_master(self) -> Result<MasterIngredient> {
        Ok(MasterIngredient {
            id: Uuid::parse_str(&self.id).context("master id")?,
            name: self.name,
            default_unit: self.default_unit,
            cost_per_unit: self.cost_per_unit.and_then(|p| p.parse().ok()),
            pack_size: self.pack_size.and_then(|p| p.parse().ok()),
            pack_unit: self.pack_unit,
            notes: self.notes,
            g_per_cup: self.g_per_cup.and_then(|p| p.parse().ok()),
            created_at: parse_dt(&self.created_at)?,
            updated_at: parse_dt(&self.updated_at)?,
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

impl IngredientMasterService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, location_id: Option<Uuid>) -> Result<Vec<MasterIngredient>> {
        let rows: Vec<MasterRow> = if let Some(loc_id) = location_id {
            sqlx::query_as(
                r#"
                SELECT i.id, i.name, i.default_unit,
                    COALESCE(loc.cost_per_unit, i.cost_per_unit) AS cost_per_unit,
                    i.pack_size, i.pack_unit, i.notes, i.g_per_cup, i.created_at, i.updated_at
                FROM ingredients i
                LEFT JOIN location_ingredient_prices loc
                    ON loc.ingredient_id = i.id AND loc.location_id = ?
                ORDER BY i.name COLLATE NOCASE
                "#,
            )
            .bind(loc_id.to_string())
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, name, default_unit, cost_per_unit, pack_size, pack_unit, notes, g_per_cup, created_at, updated_at
                 FROM ingredients ORDER BY name COLLATE NOCASE",
            )
            .fetch_all(&self.pool)
            .await?
        };
        rows.into_iter().map(MasterRow::into_master).collect()
    }

    pub async fn get(&self, id: Uuid, location_id: Option<Uuid>) -> Result<Option<MasterIngredient>> {
        let row: Option<MasterRow> = if let Some(loc_id) = location_id {
            sqlx::query_as(
                r#"
                SELECT i.id, i.name, i.default_unit,
                    COALESCE(loc.cost_per_unit, i.cost_per_unit) AS cost_per_unit,
                    i.pack_size, i.pack_unit, i.notes, i.g_per_cup, i.created_at, i.updated_at
                FROM ingredients i
                LEFT JOIN location_ingredient_prices loc
                    ON loc.ingredient_id = i.id AND loc.location_id = ?
                WHERE i.id = ?
                "#,
            )
            .bind(loc_id.to_string())
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, name, default_unit, cost_per_unit, pack_size, pack_unit, notes, g_per_cup, created_at, updated_at
                 FROM ingredients WHERE id = ?",
            )
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?
        };
        row.map(MasterRow::into_master).transpose()
    }

    pub async fn get_by_name_key(&self, key: &str) -> Result<Option<MasterIngredient>> {
        let row: Option<MasterRow> = sqlx::query_as(
            "SELECT id, name, default_unit, cost_per_unit, pack_size, pack_unit, notes, g_per_cup, created_at, updated_at
             FROM ingredients WHERE name_key = ?",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        row.map(MasterRow::into_master).transpose()
    }

    pub async fn create(
        &self,
        name: &str,
        default_unit: Option<&str>,
        cost_per_unit: Option<Decimal>,
        pack_size: Option<Decimal>,
        pack_unit: Option<&str>,
        notes: Option<&str>,
    ) -> Result<MasterIngredient> {
        let id = Uuid::new_v4();
        let key = name_key(name);
        if key.is_empty() {
            anyhow::bail!("ingredient name required");
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO ingredients (id, name, name_key, default_unit, cost_per_unit, pack_size, pack_unit, notes, g_per_cup, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(name.trim())
        .bind(&key)
        .bind(default_unit)
        .bind(cost_per_unit.map(|p| p.to_string()))
        .bind(pack_size.map(|p| p.to_string()))
        .bind(pack_unit)
        .bind(notes)
        .bind(Option::<String>::None) // g_per_cup — set via update
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .context("insert ingredient")?;

        self.get(id, None)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ingredient missing after insert"))
    }

    /// Create if missing; return existing on name match.
    pub async fn find_or_create(
        &self,
        name: &str,
        default_unit: Option<&str>,
        cost_per_unit: Option<Decimal>,
    ) -> Result<MasterIngredient> {
        let key = name_key(name);
        if let Some(existing) = self.get_by_name_key(&key).await? {
            return Ok(existing);
        }
        self.create(name, default_unit, cost_per_unit, None, None, None)
            .await
    }

    pub async fn update(
        &self,
        id: Uuid,
        name: Option<&str>,
        default_unit: Option<Option<&str>>,
        cost_per_unit: Option<Option<Decimal>>,
        pack_size: Option<Option<Decimal>>,
        pack_unit: Option<Option<&str>>,
        notes: Option<Option<&str>>,
        g_per_cup: Option<Option<Decimal>>,
    ) -> Result<MasterIngredient> {
        let mut current = self
            .get(id, None)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ingredient not found"))?;

        if let Some(n) = name {
            current.name = n.trim().to_string();
        }
        if let Some(u) = default_unit {
            current.default_unit = u.map(|s| s.to_string());
        }
        if let Some(c) = cost_per_unit {
            current.cost_per_unit = c;
        }
        if let Some(p) = pack_size {
            current.pack_size = p;
        }
        if let Some(u) = pack_unit {
            current.pack_unit = u.map(|s| s.to_string());
        }
        if let Some(n) = notes {
            current.notes = n.map(|s| s.to_string());
        }
        if let Some(d) = g_per_cup {
            current.g_per_cup = d;
        }

        let key = name_key(&current.name);
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE ingredients SET
                name = ?, name_key = ?, default_unit = ?, cost_per_unit = ?,
                pack_size = ?, pack_unit = ?, notes = ?, g_per_cup = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&current.name)
        .bind(&key)
        .bind(&current.default_unit)
        .bind(current.cost_per_unit.map(|p| p.to_string()))
        .bind(current.pack_size.map(|p| p.to_string()))
        .bind(&current.pack_unit)
        .bind(&current.notes)
        .bind(current.g_per_cup.map(|p| p.to_string()))
        .bind(&now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .context("update ingredient")?;

        // Roll-through is by JOIN at read time — no denormalized copy needed.
        // Optionally clear stale line cost_per_unit on linked rows so UI isn't confusing:
        if let Some(cpu) = current.cost_per_unit {
            sqlx::query(
                "UPDATE recipe_ingredients SET cost_per_unit = ? WHERE master_ingredient_id = ?",
            )
            .bind(cpu.to_string())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        }

        self.get(id, None)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ingredient missing after update"))
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE recipe_ingredients SET master_ingredient_id = NULL WHERE master_ingredient_id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM ingredients WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Recipes that use this master ingredient.
    pub async fn recipe_usage(&self, id: Uuid) -> Result<Vec<(Uuid, String)>> {
        #[derive(FromRow)]
        struct Row {
            recipe_id: String,
            name: String,
        }
        let rows: Vec<Row> = sqlx::query_as(
            r#"
            SELECT DISTINCT r.id as recipe_id, r.name
            FROM recipe_ingredients ri
            JOIN recipes r ON r.id = ri.recipe_id
            WHERE ri.master_ingredient_id = ?
            ORDER BY r.name COLLATE NOCASE
            "#,
        )
        .bind(id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| Uuid::parse_str(&r.recipe_id).ok().map(|id| (id, r.name)))
            .collect())
    }

    pub async fn usage_count(&self, id: Uuid) -> Result<i64> {
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT recipe_id) FROM recipe_ingredients WHERE master_ingredient_id = ?",
        )
        .bind(id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(n)
    }

    /// Link free-text recipe lines to master by name; create masters as needed.
    /// Returns (linked_or_created_lines, masters_created).
    pub async fn backfill_from_recipe_lines(&self) -> Result<(u64, u64)> {
        #[derive(FromRow)]
        struct Line {
            id: String,
            ingredient: String,
            unit: Option<String>,
            cost_per_unit: Option<String>,
            line_cost: Option<String>,
            quantity: Option<String>,
            master_ingredient_id: Option<String>,
        }

        let lines: Vec<Line> = sqlx::query_as(
            "SELECT id, ingredient, unit, cost_per_unit, line_cost, quantity, master_ingredient_id FROM recipe_ingredients",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut linked = 0u64;
        let mut created = 0u64;

        for line in lines {
            if line.master_ingredient_id.is_some() {
                continue;
            }
            let name = line.ingredient.trim();
            if name.is_empty() {
                continue;
            }
            let key = name_key(name);
            let existing = self.get_by_name_key(&key).await?;
            let master = if let Some(m) = existing {
                m
            } else {
                // Prefer unit cost; else derive rough unit cost from line_cost / qty
                let mut cpu = line.cost_per_unit.and_then(|p| p.parse().ok());
                if cpu.is_none() {
                    if let (Some(lc), Some(q)) = (
                        line.line_cost.as_ref().and_then(|p| p.parse::<Decimal>().ok()),
                        line.quantity.as_ref().and_then(|p| p.parse::<Decimal>().ok()),
                    ) {
                        if q > Decimal::ZERO {
                            cpu = Some(lc / q);
                        }
                    }
                }
                created += 1;
                self.create(name, line.unit.as_deref(), cpu, None, None, None)
                    .await?
            };

            sqlx::query(
                "UPDATE recipe_ingredients SET master_ingredient_id = ?, cost_per_unit = COALESCE(?, cost_per_unit) WHERE id = ?",
            )
            .bind(master.id.to_string())
            .bind(master.cost_per_unit.map(|p| p.to_string()))
            .bind(&line.id)
            .execute(&self.pool)
            .await?;
            linked += 1;
        }

        Ok((linked, created))
    }
}
