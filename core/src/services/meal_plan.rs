use anyhow::Result;
use chrono::NaiveDate;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::models::{MealPlan, MealType};

fn meal_type_to_str(meal_type: MealType) -> &'static str {
    match meal_type {
        MealType::Breakfast => "breakfast",
        MealType::Lunch => "lunch",
        MealType::Dinner => "dinner",
        MealType::Snack => "snack",
    }
}

#[derive(FromRow)]
struct MealPlanRow {
    id: String,
    user_id: String,
    date: String,
    meal_type: String,
    recipe_id: Option<String>,
    title: Option<String>,
    notes: Option<String>,
}

impl From<MealPlanRow> for MealPlan {
    fn from(row: MealPlanRow) -> Self {
        Self {
            id: Uuid::parse_str(&row.id).unwrap_or_default(),
            user_id: Uuid::parse_str(&row.user_id).unwrap_or_default(),
            date: NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").unwrap_or_default(),
            meal_type: match row.meal_type.as_str() {
                "breakfast" => MealType::Breakfast,
                "lunch" => MealType::Lunch,
                "dinner" => MealType::Dinner,
                _ => MealType::Snack,
            },
            recipe_id: row.recipe_id.and_then(|s| Uuid::parse_str(&s).ok()),
            title: row.title,
            notes: row.notes,
        }
    }
}

pub struct MealPlanService {
    pool: SqlitePool,
}

impl MealPlanService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_week(&self, user_id: Uuid, week_start: NaiveDate) -> Result<Vec<MealPlan>> {
        let week_end = week_start + chrono::Duration::days(6);
        let rows: Vec<MealPlanRow> = sqlx::query_as(
            "SELECT * FROM meal_plans WHERE user_id = ? AND date BETWEEN ? AND ? ORDER BY date, meal_type"
        )
        .bind(user_id.to_string())
        .bind(week_start.to_string())
        .bind(week_end.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(MealPlan::from).collect())
    }

    pub async fn get_day(&self, user_id: Uuid, date: NaiveDate) -> Result<Vec<MealPlan>> {
        let rows: Vec<MealPlanRow> = sqlx::query_as(
            "SELECT * FROM meal_plans WHERE user_id = ? AND date = ? ORDER BY meal_type",
        )
        .bind(user_id.to_string())
        .bind(date.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(MealPlan::from).collect())
    }

    pub async fn add_meal(&self, plan: &MealPlan) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let meal_type_str = meal_type_to_str(plan.meal_type);

        sqlx::query(
            "INSERT INTO meal_plans (id, user_id, date, meal_type, recipe_id, title, notes) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(id.to_string())
        .bind(plan.user_id.to_string())
        .bind(plan.date.to_string())
        .bind(meal_type_str)
        .bind(plan.recipe_id.map(|r| r.to_string()))
        .bind(&plan.title)
        .bind(&plan.notes)
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn remove_meal(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM meal_plans WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn clear_slot(
        &self,
        user_id: Uuid,
        date: NaiveDate,
        meal_type: MealType,
    ) -> Result<()> {
        let meal_type_str = meal_type_to_str(meal_type);
        sqlx::query(
            "DELETE FROM meal_plans WHERE user_id = ? AND date = ? AND meal_type = ?",
        )
        .bind(user_id.to_string())
        .bind(date.to_string())
        .bind(meal_type_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn set_recipe(
        &self,
        user_id: Uuid,
        date: NaiveDate,
        meal_type: MealType,
        recipe_id: Uuid,
    ) -> Result<Uuid> {
        self.clear_slot(user_id, date, meal_type).await?;

        let plan = MealPlan {
            id: Uuid::new_v4(),
            user_id,
            date,
            meal_type,
            recipe_id: Some(recipe_id),
            title: None,
            notes: None,
        };

        self.add_meal(&plan).await
    }

    pub async fn get_recipes_for_week(
        &self,
        user_id: Uuid,
        week_start: NaiveDate,
    ) -> Result<Vec<Uuid>> {
        let week_end = week_start + chrono::Duration::days(6);
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT recipe_id FROM meal_plans WHERE user_id = ? AND date BETWEEN ? AND ? AND recipe_id IS NOT NULL"
        )
        .bind(user_id.to_string())
        .bind(week_start.to_string())
        .bind(week_end.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(s,)| Uuid::parse_str(&s).ok())
            .collect())
    }
}
