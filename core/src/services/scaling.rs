use rust_decimal::Decimal;

pub fn scale_ingredient(
    quantity: &Decimal,
    original_servings: u32,
    target_servings: u32,
) -> Decimal {
    if original_servings == 0 {
        return quantity.clone();
    }
    let scale = Decimal::from(target_servings) / Decimal::from(original_servings);
    quantity * scale
}

pub fn format_quantity(qty: &Decimal) -> String {
    let normalized = qty.normalize();
    let s = normalized.to_string();

    if s.contains('.') {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() == 2 && parts[1].chars().all(|c| c == '0') {
            return parts[0].to_string();
        }
    }

    s
}

pub fn scale_display_text(display: &str, original_servings: u32, target_servings: u32) -> String {
    if original_servings == target_servings || original_servings == 0 {
        return display.to_string();
    }

    let scale = Decimal::from(target_servings) / Decimal::from(original_servings);

    if let Some((qty_str, rest)) = extract_leading_number(display) {
        if let Ok(qty) = qty_str.parse::<Decimal>() {
            let scaled = qty * scale;
            return format!("{}{}", format_quantity(&scaled), rest);
        }
    }

    display.to_string()
}

fn extract_leading_number(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let mut end = 0;
    let mut has_dot = false;

    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            end = i + c.len_utf8();
        } else if c == '.' && !has_dot {
            has_dot = true;
            end = i + c.len_utf8();
        } else {
            break;
        }
    }

    if end > 0 {
        Some((s[..end].to_string(), &s[end..]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn scales_ingredient_quantity() {
        let qty = Decimal::from(2);
        let scaled = scale_ingredient(&qty, 4, 8);
        assert_eq!(scaled, Decimal::from(4));
    }

    #[test]
    fn scales_display_text() {
        let scaled = scale_display_text("2 cups flour", 4, 8);
        assert_eq!(scaled, "4 cups flour");
    }
}
