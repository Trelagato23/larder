use rust_decimal::Decimal;

use super::scaling::{format_mixed_fraction, format_quantity};
use super::uom::{heuristic_density_g_per_cup, liquid_unit_g_per_cup, normalize_unit};

/// Kitchen volume in teaspoons so 16 tbsp == 1 cup exactly.
const TSP_PER_TBSP: i32 = 3;
const TSP_PER_FLOZ: i32 = 6;
const TSP_PER_CUP: i32 = 48;
const TSP_PER_PT: i32 = 96;
const TSP_PER_QT: i32 = 192;
const TSP_PER_GAL: i32 = 768;
const ML_PER_TSP: i32 = 5;

fn oz_g() -> Decimal {
    Decimal::new(283495, 4)
}

fn lb_g() -> Decimal {
    Decimal::new(453592, 3)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureMode {
    Default,
    Standard,
    Weights,
}

impl MeasureMode {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "standard" => Self::Standard,
            "weights" | "weight" => Self::Weights,
            _ => Self::Default,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Standard => "standard",
            Self::Weights => "weights",
        }
    }
}

/// Quantity text for one ingredient line after a batch factor.
pub fn format_ingredient_qty(
    qty: Option<Decimal>,
    unit: Option<&str>,
    ingredient: &str,
    density_g_per_cup: Option<Decimal>,
    factor: Decimal,
    mode: MeasureMode,
) -> String {
    let Some(q) = qty else {
        return unit.unwrap_or("").trim().to_string();
    };
    let scaled = q * factor;
    let unit = unit.unwrap_or("");
    match mode {
        MeasureMode::Default => format_default(scaled, unit),
        MeasureMode::Standard => format_standard(scaled, unit),
        MeasureMode::Weights => format_weights(scaled, unit, ingredient, density_g_per_cup)
            .unwrap_or_else(|| format_default(scaled, unit)),
    }
}

/// Factor-1 milliliters (kitchen 5 ml/tsp) and grams, when convertible.
pub fn ingredient_bases(
    qty: Option<Decimal>,
    unit: Option<&str>,
    ingredient: &str,
    density_g_per_cup: Option<Decimal>,
) -> (Option<Decimal>, Option<Decimal>) {
    let Some(q) = qty else {
        return (None, None);
    };
    let unit = unit.unwrap_or("");
    let volume_ml = kitchen_tsp(q, unit).map(|tsp| tsp * Decimal::from(ML_PER_TSP));
    let mass_g = to_mass_g(q, unit, ingredient, density_g_per_cup);
    (volume_ml, mass_g)
}

fn format_default(qty: Decimal, unit: &str) -> String {
    join_qty_unit(&format_quantity(&qty), unit)
}

fn format_standard(qty: Decimal, unit: &str) -> String {
    let canon = normalize_unit(unit);
    match canon.as_str() {
        "pt" | "qt" | "gal" => format_cleaned(qty, unit),
        "cup" => {
            if let Some(tsp) = kitchen_tsp(qty, unit) {
                let cups = tsp / Decimal::from(TSP_PER_CUP);
                if let Some(n) = snap(cups, &cup_targets(cups), TSP_PER_CUP) {
                    return join_qty_unit(&pretty_qty(n), &display_unit("cup", n));
                }
            }
            format_cleaned(qty, unit)
        }
        _ => {
            if let Some(tsp) = kitchen_tsp(qty, unit) {
                if let Some(text) = pick_easy_volume(tsp) {
                    return text;
                }
            }
            format_cleaned(qty, unit)
        }
    }
}

fn format_weights(
    qty: Decimal,
    unit: &str,
    ingredient: &str,
    density_g_per_cup: Option<Decimal>,
) -> Option<String> {
    let g = to_mass_g(qty, unit, ingredient, density_g_per_cup)?;
    Some(format_oz_lb(g))
}

fn format_cleaned(qty: Decimal, unit: &str) -> String {
    let canon = normalize_unit(unit);
    if canon.is_empty() {
        return format_quantity(&qty);
    }
    let pretty = pretty_qty(qty);
    join_qty_unit(&pretty, &display_unit(&canon, qty))
}

fn join_qty_unit(qty: &str, unit: &str) -> String {
    let u = unit.trim();
    if u.is_empty() {
        qty.to_string()
    } else {
        format!("{qty} {u}")
    }
}

fn pretty_qty(qty: Decimal) -> String {
    format_mixed_fraction(qty).unwrap_or_else(|| format_quantity(&qty))
}

fn display_unit(canon: &str, qty: Decimal) -> String {
    match canon {
        "cup" if qty.abs() <= Decimal::ONE => "cup".into(),
        "cup" => "cups".into(),
        other => other.to_string(),
    }
}

fn kitchen_tsp(qty: Decimal, unit: &str) -> Option<Decimal> {
    match normalize_unit(unit).as_str() {
        "tsp" => Some(qty),
        "tbsp" => Some(qty * Decimal::from(TSP_PER_TBSP)),
        "fl oz" => Some(qty * Decimal::from(TSP_PER_FLOZ)),
        "cup" => Some(qty * Decimal::from(TSP_PER_CUP)),
        "pt" => Some(qty * Decimal::from(TSP_PER_PT)),
        "qt" => Some(qty * Decimal::from(TSP_PER_QT)),
        "gal" => Some(qty * Decimal::from(TSP_PER_GAL)),
        "ml" => Some(qty / Decimal::from(ML_PER_TSP)),
        "l" => Some(qty * Decimal::from(200)),
        _ => None,
    }
}

fn to_mass_g(
    qty: Decimal,
    unit: &str,
    ingredient: &str,
    density_g_per_cup: Option<Decimal>,
) -> Option<Decimal> {
    match normalize_unit(unit).as_str() {
        "g" => Some(qty),
        "kg" => Some(qty * Decimal::from(1000)),
        "oz" => Some(qty * oz_g()),
        "lb" => Some(qty * lb_g()),
        _ => {
            let cups = kitchen_tsp(qty, unit)? / Decimal::from(TSP_PER_CUP);
            let dens = density_g_per_cup
                .or_else(|| heuristic_density_g_per_cup(ingredient))
                .or_else(|| liquid_unit_g_per_cup(unit))?;
            Some(cups * dens)
        }
    }
}

fn pick_easy_volume(tsp: Decimal) -> Option<String> {
    if tsp <= Decimal::ZERO {
        return None;
    }

    let cups = tsp / Decimal::from(TSP_PER_CUP);
    if let Some(n) = snap(cups, &cup_targets(cups), TSP_PER_CUP) {
        return Some(join_qty_unit(&pretty_qty(n), &display_unit("cup", n)));
    }

    let tbsp = tsp / Decimal::from(TSP_PER_TBSP);
    if let Some(n) = snap(tbsp, &tbsp_targets(tbsp), TSP_PER_TBSP) {
        if n >= Decimal::ONE {
            return Some(join_qty_unit(&pretty_qty(n), "tbsp"));
        }
    }

    if let Some(n) = snap(tsp, &tsp_targets(tsp), 1) {
        return Some(join_qty_unit(&pretty_qty(n), "tsp"));
    }
    None
}

fn near(actual: Decimal, target: Decimal, unit_tsp: i32) -> bool {
    if target <= Decimal::ZERO {
        return false;
    }
    let err_tsp = (actual - target).abs() * Decimal::from(unit_tsp);
    let rel = (actual - target).abs() / target;
    rel <= dec("0.02") || err_tsp <= dec("0.15")
}

fn snap(actual: Decimal, targets: &[Decimal], unit_tsp: i32) -> Option<Decimal> {
    let mut best: Option<(Decimal, Decimal)> = None;
    for &t in targets {
        if !near(actual, t, unit_tsp) {
            continue;
        }
        let err = (actual - t).abs() * Decimal::from(unit_tsp);
        if best.map_or(true, |(_, e)| err < e) {
            best = Some((t, err));
        }
    }
    best.map(|(t, _)| t)
}

fn cup_targets(around: Decimal) -> Vec<Decimal> {
    let fracs = [
        Decimal::from(1) / Decimal::from(4),
        Decimal::from(1) / Decimal::from(3),
        Decimal::from(1) / Decimal::from(2),
        Decimal::from(2) / Decimal::from(3),
        Decimal::from(3) / Decimal::from(4),
    ];
    let max = around.ceil() + Decimal::ONE;
    let mut n = Decimal::ZERO;
    let mut out = Vec::new();
    while n <= max {
        if n > Decimal::ZERO {
            out.push(n);
        }
        for f in fracs {
            let x = n + f;
            if x > Decimal::ZERO {
                out.push(x);
            }
        }
        n += Decimal::ONE;
    }
    out
}

fn tbsp_targets(around: Decimal) -> Vec<Decimal> {
    let fracs = [
        Decimal::from(1) / Decimal::from(2),
        Decimal::from(1) / Decimal::from(3),
        Decimal::from(2) / Decimal::from(3),
    ];
    let max = around.ceil() + Decimal::ONE;
    let mut n = Decimal::ZERO;
    let mut out = Vec::new();
    while n <= max {
        if n > Decimal::ZERO {
            out.push(n);
        }
        for f in fracs {
            let x = n + f;
            if x > Decimal::ZERO {
                out.push(x);
            }
        }
        n += Decimal::ONE;
    }
    out
}

fn tsp_targets(around: Decimal) -> Vec<Decimal> {
    let fracs = [
        Decimal::from(1) / Decimal::from(8),
        Decimal::from(1) / Decimal::from(4),
        Decimal::from(1) / Decimal::from(2),
        Decimal::from(3) / Decimal::from(4),
    ];
    let max = around.ceil() + Decimal::ONE;
    let mut n = Decimal::ZERO;
    let mut out = Vec::new();
    while n <= max {
        if n > Decimal::ZERO {
            out.push(n);
        }
        for f in fracs {
            let x = n + f;
            if x > Decimal::ZERO {
                out.push(x);
            }
        }
        n += Decimal::ONE;
    }
    out
}

fn format_oz_lb(grams: Decimal) -> String {
    let oz = grams / oz_g();
    if oz < Decimal::from(16) {
        return join_qty_unit(&pretty_qty(round_quarter(oz)), "oz");
    }
    let total = round_quarter(oz);
    let sixteen = Decimal::from(16);
    let mut lbs = (total / sixteen).trunc();
    let mut rem = total - lbs * sixteen;
    if rem == sixteen {
        lbs += Decimal::ONE;
        rem = Decimal::ZERO;
    }
    if rem.is_zero() {
        return join_qty_unit(&pretty_qty(lbs), "lb");
    }
    if rem == Decimal::from(8) {
        return join_qty_unit(&pretty_qty(lbs + Decimal::from(1) / Decimal::from(2)), "lb");
    }
    format!("{} lb {} oz", pretty_qty(lbs), pretty_qty(rem))
}

fn round_quarter(qty: Decimal) -> Decimal {
    (qty * Decimal::from(4)).round() / Decimal::from(4)
}

fn dec(s: &str) -> Decimal {
    s.parse().expect("decimal literal")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    fn std(qty: &str, unit: &str) -> String {
        format_ingredient_qty(
            Some(q(qty)),
            Some(unit),
            "x",
            None,
            Decimal::ONE,
            MeasureMode::Standard,
        )
    }

    fn wgt(qty: &str, unit: &str, name: &str, dens: Option<&str>) -> String {
        format_ingredient_qty(
            Some(q(qty)),
            Some(unit),
            name,
            dens.map(q),
            Decimal::ONE,
            MeasureMode::Weights,
        )
    }

    #[test]
    fn standard_promotes_easy_cups() {
        assert_eq!(std("16", "tbl"), "1 cup");
        assert_eq!(std("4", "tbl"), "¼ cup");
        assert_eq!(std("8", "tbl"), "½ cup");
        assert_eq!(std("1.25", "cup"), "1¼ cups");
    }

    #[test]
    fn standard_keeps_awkward_spoons() {
        assert_eq!(std("5", "tbl"), "5 tbsp");
        assert_eq!(std("2", "tbsp"), "2 tbsp");
        assert_eq!(std("1.33", "tbl"), "1⅓ tbsp");
    }

    #[test]
    fn standard_tsp_becomes_tbsp() {
        assert_eq!(std("3", "tsp"), "1 tbsp");
    }

    #[test]
    fn standard_keeps_written_cups() {
        assert_eq!(std("4", "cups"), "4 cups");
        assert_eq!(std("16", "cups"), "16 cups");
        assert_eq!(std("8", "cups"), "8 cups");
        assert_eq!(std("3", "cups"), "3 cups");
        assert_eq!(std("1", "gal"), "1 gal");
        assert_eq!(std("1", "qt"), "1 qt");
    }

    #[test]
    fn standard_passthrough_count() {
        assert_eq!(std("2", "ea"), "2 ea");
        assert_eq!(std("1", "#10 can"), "1 #10 can");
        assert_eq!(std("1", "bunch"), "1 bunch");
    }

    #[test]
    fn standard_leaves_weight_lines() {
        assert_eq!(std("2", "lb"), "2 lb");
        assert_eq!(std("8", "oz"), "8 oz");
    }

    #[test]
    fn default_keeps_tbl() {
        let s = format_ingredient_qty(
            Some(q("16")),
            Some("tbl"),
            "oil",
            None,
            Decimal::ONE,
            MeasureMode::Default,
        );
        assert_eq!(s, "16 tbl");
    }

    #[test]
    fn default_keeps_written_cups() {
        let s = format_ingredient_qty(
            Some(q("4")),
            Some("cup"),
            "sour cream",
            None,
            Decimal::ONE,
            MeasureMode::Default,
        );
        assert_eq!(s, "4 cup");
    }

    #[test]
    fn weights_uses_density() {
        assert_eq!(wgt("1", "cup", "flour", Some("120")), "4¼ oz");
    }

    #[test]
    fn weights_us_sour_cream_cup_is_eight_oz() {
        assert_eq!(wgt("1", "cup", "sour cream", None), "8 oz");
        assert_eq!(wgt("4", "cup", "sour cream", None), "2 lb");
    }

    #[test]
    fn weights_heuristic_flour() {
        assert_eq!(wgt("1", "cup", "bread flour", None), "4¼ oz");
    }

    #[test]
    fn weights_under_one_pound_is_ounces() {
        assert_eq!(wgt("8", "oz", "cheese", None), "8 oz");
    }

    #[test]
    fn weights_pounds_and_leftover() {
        assert_eq!(wgt("1", "lb", "chicken", None), "1 lb");
        assert_eq!(wgt("24", "oz", "chicken", None), "1½ lb");
        assert_eq!(wgt("20", "oz", "chicken", None), "1 lb 4 oz");
    }

    #[test]
    fn weights_unknown_volume_stays_default() {
        assert_eq!(wgt("1", "cup", "foofaraw mix", None), "1 cup");
    }

    #[test]
    fn weights_orange_juice_cup() {
        assert_eq!(wgt("1", "cup", "orange juice", None), "8¾ oz");
    }

    #[test]
    fn weights_liquid_unit_unknown_uses_waterish() {
        assert_eq!(wgt("1", "gal", "foofaraw mix", None), "8 lb");
    }

    #[test]
    fn weights_scales_with_factor() {
        let s = format_ingredient_qty(
            Some(q("1")),
            Some("cup"),
            "flour",
            Some(q("120")),
            Decimal::from(2),
            MeasureMode::Weights,
        );
        assert_eq!(s, "8½ oz");
    }

    #[test]
    fn standard_scales_then_promotes() {
        let s = format_ingredient_qty(
            Some(q("8")),
            Some("tbl"),
            "oil",
            None,
            Decimal::from(2),
            MeasureMode::Standard,
        );
        assert_eq!(s, "1 cup");
    }
}
