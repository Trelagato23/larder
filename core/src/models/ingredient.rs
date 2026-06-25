use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeIngredient {
    pub id: Uuid,
    pub recipe_id: Uuid,
    pub ingredient: String,
    pub quantity: Option<Decimal>,
    pub unit: Option<String>,
    pub note: Option<String>,
    pub display: String,
    pub category: Option<String>,
    pub cost_per_unit: Option<Decimal>,
    /// Flat cost for this line at recipe yield (used when set instead of qty × unit cost).
    pub line_cost: Option<Decimal>,
}

impl RecipeIngredient {
    pub fn formatted(&self) -> String {
        match (&self.quantity, &self.unit) {
            (Some(qty), Some(unit)) => format!("{} {} {}", qty, unit, self.ingredient),
            (Some(qty), None) => format!("{} {}", qty, self.ingredient),
            (None, Some(unit)) => format!("{} {}", unit, self.ingredient),
            (None, None) => self.ingredient.clone(),
        }
    }
}
