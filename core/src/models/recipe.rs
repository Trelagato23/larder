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
    /// Batch output quantity (e.g. 24 rolls).
    pub yield_quantity: Option<Decimal>,
    pub yield_unit: Option<String>,
    /// Recipe-level waste/spoilage % added to pulls.
    pub waste_percent: Option<Decimal>,
    /// Display name of who wrote / owns the recipe.
    pub author: Option<String>,
    /// Estimated kcal per serving (whole number).
    pub estimated_calories: Option<u32>,
    /// Comma-separated allergen / dietary labels (e.g. "gluten, dairy, egg").
    pub allergens: Option<String>,
    /// Last time kitchen/office opened this recipe (detail view). NULL = never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened_at: Option<DateTime<Utc>>,
    /// How many times the detail view was opened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_count: Option<u32>,
}

/// Suggested vocabulary for manager editors; free text is also allowed.
pub const SUGGESTED_ALLERGENS: &[&str] = &[
    "gluten", "dairy", "egg", "nuts", "soy", "sesame", "shellfish", "fish",
];

/// Split a stored allergens string into normalized lowercase labels.
pub fn parse_allergen_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Normalize user input to a stored comma-separated string, or None if empty.
pub fn normalize_allergens(raw: Option<&str>) -> Option<String> {
    let list = raw.map(parse_allergen_list).unwrap_or_default();
    if list.is_empty() {
        None
    } else {
        Some(list.join(", "))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeStep {
    pub id: Uuid,
    pub recipe_id: Uuid,
    pub position: u32,
    pub instruction: String,
    pub timer_seconds: Option<u32>,
}

/// Floor note on a recipe: subtle tip or flagged alert.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NoteSeverity {
    Subtle,
    Flagged,
}

impl NoteSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            NoteSeverity::Subtle => "subtle",
            NoteSeverity::Flagged => "flagged",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "subtle" | "note" | "info" => Some(NoteSeverity::Subtle),
            "flagged" | "flag" | "alert" | "important" => Some(NoteSeverity::Flagged),
            _ => None,
        }
    }
}

/// Who posted the note (kitchen team vs supervisor vs manager).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NoteAuthorRole {
    Team,
    Supervisor,
    Manager,
}

impl NoteAuthorRole {
    pub fn as_str(self) -> &'static str {
        match self {
            NoteAuthorRole::Team => "team",
            NoteAuthorRole::Supervisor => "supervisor",
            NoteAuthorRole::Manager => "manager",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "team" | "kitchen" | "member" => Some(NoteAuthorRole::Team),
            "supervisor" | "lead" => Some(NoteAuthorRole::Supervisor),
            "manager" => Some(NoteAuthorRole::Manager),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeNote {
    pub id: Uuid,
    pub recipe_id: Uuid,
    pub body: String,
    pub severity: NoteSeverity,
    pub author_role: NoteAuthorRole,
    pub author_name: String,
    pub author_user_id: Option<Uuid>,
    /// Typed manager sign-off (name).
    pub signature: Option<String>,
    pub signed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Recipe {
    pub fn allergen_list(&self) -> Vec<String> {
        self.allergens
            .as_deref()
            .map(parse_allergen_list)
            .unwrap_or_default()
    }

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
