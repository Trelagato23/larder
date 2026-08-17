use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shared ingredient (master) — cost lives here and rolls into recipes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterIngredient {
    pub id: Uuid,
    pub name: String,
    pub default_unit: Option<String>,
    pub cost_per_unit: Option<Decimal>,
    pub pack_size: Option<Decimal>,
    pub pack_unit: Option<String>,
    pub notes: Option<String>,
    /// Grams per cup for volume↔weight conversion (overrides name heuristics).
    pub g_per_cup: Option<Decimal>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeIngredient {
    pub id: Uuid,
    pub recipe_id: Uuid,
    /// Free-text name on the line (for cook display / legacy).
    pub ingredient: String,
    pub quantity: Option<Decimal>,
    pub unit: Option<String>,
    pub note: Option<String>,
    pub display: String,
    pub category: Option<String>,
    /// Effective unit cost (from master when linked, else line override).
    pub cost_per_unit: Option<Decimal>,
    /// Flat cost for this line at recipe yield when not using unit cost.
    pub line_cost: Option<Decimal>,
    /// Link to master ingredient for price roll-through.
    pub master_ingredient_id: Option<Uuid>,
    /// Usable yield % after trim (e.g. 85 for onion).
    pub prep_yield_percent: Option<Decimal>,
    /// Master name when linked (API convenience).
    #[serde(default)]
    pub master_name: Option<String>,
    /// Master density when linked (g per cup).
    #[serde(default)]
    pub master_g_per_cup: Option<Decimal>,
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

pub fn name_key(name: &str) -> String {
    name.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}
