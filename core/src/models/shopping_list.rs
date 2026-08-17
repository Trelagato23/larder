use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingListItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub item: String,
    pub quantity: Option<rust_decimal::Decimal>,
    pub unit: Option<String>,
    pub category: Option<String>,
    pub checked: bool,
    pub recipe_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
