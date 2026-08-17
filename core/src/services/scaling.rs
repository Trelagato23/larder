use rust_decimal::Decimal;

pub fn scale_ingredient(
    quantity: &Decimal,
    original_servings: u32,
    target_servings: u32,
) -> Decimal {
    scale_ingredient_by_factor(quantity, combined_scale_factor(original_servings, target_servings, Decimal::ONE))
}

/// Combined multiplier: serving adjustment × batch count.
pub fn combined_scale_factor(
    original_servings: u32,
    target_servings: u32,
    batches: Decimal,
) -> Decimal {
    if original_servings == 0 {
        return batches;
    }
    (Decimal::from(target_servings) / Decimal::from(original_servings)) * batches
}

pub fn scale_ingredient_by_factor(quantity: &Decimal, factor: Decimal) -> Decimal {
    quantity * factor
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
    scale_display_by_factor(
        display,
        combined_scale_factor(original_servings, target_servings, Decimal::ONE),
    )
}

pub fn scale_display_by_factor(display: &str, factor: Decimal) -> String {
    if factor == Decimal::ONE {
        return display.to_string();
    }

    if let Some((qty_str, rest)) = extract_leading_number(display) {
        if let Ok(qty) = qty_str.parse::<Decimal>() {
            let class = detect_unit_class(rest);
            let scaled = round_culinary(qty * factor, class);
            return format!("{}{}", format_culinary_quantity(scaled, class), rest);
        }
    }

    display.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitClass {
    /// cups / tsp / tbsp — round to nearest ¼
    Volume,
    /// grams — whole numbers
    Grams,
    /// kilograms — nearest 0.1
    Kilograms,
    /// everything else — 3 decimal places
    Generic,
}

fn detect_unit_class(rest: &str) -> UnitClass {
    let word = rest
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == ',')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match word.as_str() {
        "cup" | "cups" | "tsp" | "tsps" | "teaspoon" | "teaspoons" | "tbsp" | "tbsps"
        | "tablespoon" | "tablespoons" | "ml" | "milliliter" | "milliliters" | "millilitre"
        | "millilitres" => UnitClass::Volume,
        "g" | "gram" | "grams" => UnitClass::Grams,
        "kg" | "kilogram" | "kilograms" => UnitClass::Kilograms,
        _ => UnitClass::Generic,
    }
}

fn round_culinary(qty: Decimal, class: UnitClass) -> Decimal {
    match class {
        UnitClass::Volume => (qty * Decimal::from(4)).round() / Decimal::from(4),
        UnitClass::Grams => qty.round_dp(0),
        UnitClass::Kilograms => (qty * Decimal::from(10)).round() / Decimal::from(10),
        UnitClass::Generic => qty.round_dp(3),
    }
}

fn format_culinary_quantity(qty: Decimal, class: UnitClass) -> String {
    if class == UnitClass::Volume {
        if let Some(pretty) = format_mixed_fraction(qty) {
            return pretty;
        }
    }
    format_quantity(&qty)
}

/// Format a quantity as a mixed number with unicode fractions when it lands on
/// a common culinary step (quarters / thirds / eighths).
pub fn format_mixed_fraction(qty: Decimal) -> Option<String> {
    let negative = qty < Decimal::ZERO;
    let qty = if negative { -qty } else { qty };
    let whole = qty.trunc();
    let frac = qty - whole;

    let frac_char = if frac.is_zero() {
        None
    } else if frac == Decimal::from(1) / Decimal::from(2) {
        Some('½')
    } else if frac == Decimal::from(1) / Decimal::from(4) {
        Some('¼')
    } else if frac == Decimal::from(3) / Decimal::from(4) {
        Some('¾')
    } else if frac == Decimal::from(1) / Decimal::from(3) {
        Some('⅓')
    } else if frac == Decimal::from(2) / Decimal::from(3) {
        Some('⅔')
    } else if frac == Decimal::from(1) / Decimal::from(8) {
        Some('⅛')
    } else if frac == Decimal::from(3) / Decimal::from(8) {
        Some('⅜')
    } else if frac == Decimal::from(5) / Decimal::from(8) {
        Some('⅝')
    } else if frac == Decimal::from(7) / Decimal::from(8) {
        Some('⅞')
    } else {
        return None;
    };

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if !whole.is_zero() || frac_char.is_none() {
        out.push_str(&format_quantity(&whole));
    }
    if let Some(c) = frac_char {
        out.push(c);
    }
    Some(out)
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

    // Lone unicode fraction, e.g. "½ cup flour".
    if end == 0 {
        let c = s.chars().next()?;
        if let Some((num, den)) = unicode_fraction(c) {
            let value = Decimal::from(num) / Decimal::from(den);
            return Some((format_quantity(&value), &s[c.len_utf8()..]));
        }
        return None;
    }

    let head = &s[..end];
    let rest = &s[end..];

    // Whole number directly followed by a unicode fraction, e.g. "1½ cups".
    if let Some(c) = rest.chars().next() {
        if let Some((num, den)) = unicode_fraction(c) {
            if let Ok(whole) = head.parse::<i64>() {
                let value = Decimal::from(whole) + Decimal::from(num) / Decimal::from(den);
                return Some((format_quantity(&value), &rest[c.len_utf8()..]));
            }
        }
    }

    if !has_dot {
        // Simple fraction, e.g. "1/2 cup flour".
        if let Some(after_slash) = rest.strip_prefix('/') {
            let den_len = digit_len(after_slash);
            if den_len > 0 {
                let num: i64 = head.parse().ok()?;
                let den: i64 = after_slash[..den_len].parse().ok()?;
                if den != 0 {
                    let value = Decimal::from(num) / Decimal::from(den);
                    return Some((format_quantity(&value), &after_slash[den_len..]));
                }
            }
        }

        // Mixed number, e.g. "1 1/2 cups sugar".
        let trimmed = rest.trim_start();
        if trimmed.len() < rest.len() {
            let num_len = digit_len(trimmed);
            if num_len > 0 {
                if let Some(after_slash) = trimmed[num_len..].strip_prefix('/') {
                    let den_len = digit_len(after_slash);
                    if den_len > 0 {
                        let whole: i64 = head.parse().ok()?;
                        let num: i64 = trimmed[..num_len].parse().ok()?;
                        let den: i64 = after_slash[..den_len].parse().ok()?;
                        if den != 0 {
                            let value =
                                Decimal::from(whole) + Decimal::from(num) / Decimal::from(den);
                            return Some((format_quantity(&value), &after_slash[den_len..]));
                        }
                    }
                }
            }
        }
    }

    Some((head.to_string(), rest))
}

fn digit_len(s: &str) -> usize {
    s.chars()
        .take_while(|c| c.is_ascii_digit())
        .map(|c| c.len_utf8())
        .sum()
}

/// Numerator and denominator for common unicode fraction characters.
fn unicode_fraction(c: char) -> Option<(i64, i64)> {
    match c {
        '½' => Some((1, 2)),
        '¼' => Some((1, 4)),
        '¾' => Some((3, 4)),
        '⅓' => Some((1, 3)),
        '⅔' => Some((2, 3)),
        '⅛' => Some((1, 8)),
        '⅜' => Some((3, 8)),
        '⅝' => Some((5, 8)),
        '⅞' => Some((7, 8)),
        '⅕' => Some((1, 5)),
        '⅖' => Some((2, 5)),
        '⅗' => Some((3, 5)),
        '⅘' => Some((4, 5)),
        '⅙' => Some((1, 6)),
        '⅚' => Some((5, 6)),
        _ => None,
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

    #[test]
    fn scales_simple_fraction() {
        let scaled = scale_display_text("1/2 cup flour", 4, 8);
        assert_eq!(scaled, "1 cup flour");
    }

    #[test]
    fn scales_simple_fraction_down() {
        // 3/4 × 0.5 = 0.375 → nearest quarter = 0.5 → ½
        let scaled = scale_display_text("3/4 cup sugar", 4, 2);
        assert_eq!(scaled, "½ cup sugar");
    }

    #[test]
    fn scales_mixed_number() {
        let scaled = scale_display_text("1 1/2 cups sugar", 4, 8);
        assert_eq!(scaled, "3 cups sugar");
    }

    #[test]
    fn scales_mixed_number_down() {
        let scaled = scale_display_text("1 1/2 cups sugar", 4, 2);
        assert_eq!(scaled, "¾ cups sugar");
    }

    #[test]
    fn scales_unicode_fraction() {
        let scaled = scale_display_text("½ cup flour", 4, 8);
        assert_eq!(scaled, "1 cup flour");
    }

    #[test]
    fn scales_unicode_fraction_after_whole_number() {
        let scaled = scale_display_text("1½ cups sugar", 4, 8);
        assert_eq!(scaled, "3 cups sugar");
    }

    #[test]
    fn scales_various_unicode_fractions() {
        assert_eq!(scale_display_text("¼ tsp salt", 4, 8), "½ tsp salt");
        assert_eq!(scale_display_text("¾ cup milk", 4, 8), "1½ cup milk");
        assert_eq!(scale_display_text("⅛ tsp pepper", 4, 16), "½ tsp pepper");
        assert_eq!(scale_display_text("⅓ cup oil", 1, 3), "1 cup oil");
        assert_eq!(scale_display_text("⅔ cup water", 1, 3), "2 cup water");
    }

    #[test]
    fn keeps_decimal_behavior() {
        assert_eq!(scale_display_text("2.5 cups flour", 4, 8), "5 cups flour");
        assert_eq!(scale_display_text("0.5 tsp salt", 4, 2), "¼ tsp salt");
    }

    #[test]
    fn culinary_rounds_volume_to_quarters() {
        // 2 × 1.33 ≈ 2.66 → nearest quarter 2.75 → 2¾
        let factor = Decimal::new(133, 2); // 1.33
        assert_eq!(
            scale_display_by_factor("2 cups flour", factor),
            "2¾ cups flour"
        );
    }

    #[test]
    fn culinary_rounds_grams_to_whole() {
        let factor = Decimal::new(133, 2);
        assert_eq!(scale_display_by_factor("100 g flour", factor), "133 g flour");
        assert_eq!(
            scale_display_by_factor("10.4 g salt", Decimal::ONE + Decimal::new(1, 1)),
            "11 g salt"
        );
    }

    #[test]
    fn culinary_rounds_kg_to_tenth() {
        assert_eq!(
            scale_display_by_factor("1.24 kg flour", Decimal::from(2)),
            "2.5 kg flour"
        );
    }

    #[test]
    fn keeps_whole_number_behavior() {
        assert_eq!(scale_display_text("2 eggs", 4, 2), "1 eggs");
        assert_eq!(scale_display_text("10 g butter", 4, 8), "20 g butter");
    }

    #[test]
    fn leaves_unparseable_display_unchanged() {
        assert_eq!(scale_display_text("salt to taste", 4, 8), "salt to taste");
        assert_eq!(scale_display_text("a pinch of salt", 4, 8), "a pinch of salt");
    }

    #[test]
    fn leaves_number_not_followed_by_fraction_unchanged() {
        // "1 2 eggs" is not a mixed number; only the leading 1 scales.
        assert_eq!(scale_display_text("1 2 eggs", 4, 8), "2 2 eggs");
    }
}
