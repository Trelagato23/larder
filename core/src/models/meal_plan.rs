use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealPlan {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub meal_type: MealType,
    pub recipe_id: Option<Uuid>,
    pub title: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, strum::EnumIter)]
pub enum MealType {
    Breakfast,
    Lunch,
    Dinner,
    Snack,
}

impl std::fmt::Display for MealType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MealType::Breakfast => write!(f, "Breakfast"),
            MealType::Lunch => write!(f, "Lunch"),
            MealType::Dinner => write!(f, "Dinner"),
            MealType::Snack => write!(f, "Snack"),
        }
    }
}
