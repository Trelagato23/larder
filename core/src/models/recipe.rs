use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub servings: u32,
    pub prep_time_minutes: Option<u32>,
    pub cook_time_minutes: Option<u32>,
    pub total_time_minutes: Option<u32>,
    pub source_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub user_id: Uuid,
    pub rating: Option<u8>,
    pub difficulty: Option<Difficulty>,
    /// Sell price for food-cost calculations.
    pub menu_price: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeStep {
    pub id: Uuid,
    pub recipe_id: Uuid,
    pub position: u32,
    pub instruction: String,
    pub timer_seconds: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Recipe {
    pub fn total_time(&self) -> Option<u32> {
        self.total_time_minutes
            .or_else(|| match (self.prep_time_minutes, self.cook_time_minutes) {
                (Some(prep), Some(cook)) => Some(prep + cook),
                (Some(prep), None) => Some(prep),
                (None, Some(cook)) => Some(cook),
                (None, None) => None,
            })
    }
}
