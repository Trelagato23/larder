use rust_decimal::Decimal;

use crate::models::RecipeIngredient;

/// Cost for one ingredient line at the given scale factor.
pub fn ingredient_line_cost(ing: &RecipeIngredient, scale: Decimal) -> Option<Decimal> {
    if let Some(line) = ing.line_cost {
        return Some(line * scale);
    }
    if let (Some(qty), Some(cpu)) = (&ing.quantity, &ing.cost_per_unit) {
        return Some(qty * cpu * scale);
    }
    None
}

/// Sum of known ingredient costs at scale.
pub fn recipe_ingredient_cost(ingredients: &[RecipeIngredient], scale: Decimal) -> Decimal {
    ingredients
        .iter()
        .filter_map(|i| ingredient_line_cost(i, scale))
        .fold(Decimal::ZERO, |a, b| a + b)
}

/// Food cost percentage when menu price is set (0–100+).
pub fn food_cost_percent(total_cost: Decimal, menu_price: Decimal) -> Option<Decimal> {
    if menu_price <= Decimal::ZERO {
        return None;
    }
    Some((total_cost / menu_price) * Decimal::from(100))
}

pub fn format_money(amount: Decimal) -> String {
    format!("${:.2}", amount.round_dp(2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sample_ing(qty: &str, cpu: &str) -> RecipeIngredient {
        RecipeIngredient {
            id: Uuid::nil(),
            recipe_id: Uuid::nil(),
            ingredient: "flour".to_string(),
            quantity: Some(qty.parse().unwrap()),
            unit: Some("cup".to_string()),
            note: None,
            display: format!("{} cup flour", qty),
            category: None,
            cost_per_unit: Some(cpu.parse().unwrap()),
            line_cost: None,
        }
    }

    #[test]
    fn scales_cost_with_batches() {
        let ing = sample_ing("2", "1.50");
        let cost = ingredient_line_cost(&ing, Decimal::from(2)).unwrap();
        assert_eq!(cost, Decimal::from(6));
    }
}
