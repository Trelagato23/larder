use rust_decimal::Decimal;

/// Normalize common unit strings to canonical short form.
pub fn normalize_unit(unit: &str) -> String {
    match unit.trim().to_lowercase().as_str() {
        "g" | "gram" | "grams" => "g".into(),
        "kg" | "kilogram" | "kilograms" | "kilo" => "kg".into(),
        "oz" | "ounce" | "ounces" => "oz".into(),
        "lb" | "lbs" | "pound" | "pounds" => "lb".into(),
        "tsp" | "teaspoon" | "teaspoons" => "tsp".into(),
        "tbsp" | "tablespoon" | "tablespoons" | "tbl" | "tbs" | "t" => "tbsp".into(),
        "cup" | "cups" | "c" => "cup".into(),
        "pt" | "pint" | "pints" => "pt".into(),
        "qt" | "quart" | "quarts" => "qt".into(),
        "gal" | "gallon" | "gallons" => "gal".into(),
        "fl oz" | "floz" | "fl. oz" | "fl.oz" | "fluid ounce" | "fluid ounces" => "fl oz".into(),
        "ml" | "milliliter" | "milliliters" | "millilitre" | "millilitres" => "ml".into(),
        "l" | "liter" | "liters" | "litre" | "litres" => "l".into(),
        "ea" | "each" => "ea".into(),
        "pinch" | "pinches" => "pinch".into(),
        "clove" | "cloves" => "clove".into(),
        "slice" | "slices" => "slice".into(),
        "large" | "medium" | "small" => unit.trim().to_lowercase(),
        other => other.to_string(),
    }
}

/// Convert quantity between compatible units. Returns None if incompatible.
pub fn convert(qty: Decimal, from: &str, to: &str) -> Option<Decimal> {
    let from = normalize_unit(from);
    let to = normalize_unit(to);
    if from == to {
        return Some(qty);
    }
    let grams = to_grams(qty, &from)?;
    from_grams(grams, &to)
}

/// Convert with ingredient hint for volume↔weight (e.g. cup flour → g).
/// Prefer `density_g_per_cup` from ingredient master when provided; otherwise
/// fall back to name heuristics.
pub fn convert_with_ingredient(
    qty: Decimal,
    from: &str,
    to: &str,
    ingredient: Option<&str>,
) -> Option<Decimal> {
    convert_with_density(qty, from, to, None, ingredient)
}

pub fn convert_with_density(
    qty: Decimal,
    from: &str,
    to: &str,
    density_g_per_cup: Option<Decimal>,
    ingredient: Option<&str>,
) -> Option<Decimal> {
    let from = normalize_unit(from);
    let to = normalize_unit(to);
    if from == to {
        return Some(qty);
    }
    if let Some(density) = density_g_per_cup.or_else(|| ingredient.and_then(heuristic_density_g_per_cup)) {
        if from == "cup" && to == "g" {
            return Some(qty * density);
        }
        if from == "g" && to == "cup" {
            return Some(qty / density);
        }
    }
    convert(qty, &from, &to)
}

/// Preferred pull unit for bakery (grams) when conversion is possible.
pub fn to_pull_display(qty: Decimal, unit: Option<&str>, ingredient: Option<&str>) -> String {
    to_pull_display_with_density(qty, unit, None, ingredient)
}

pub fn to_pull_display_with_density(
    qty: Decimal,
    unit: Option<&str>,
    density_g_per_cup: Option<Decimal>,
    ingredient: Option<&str>,
) -> String {
    let u = unit.map(normalize_unit).unwrap_or_default();
    if u.is_empty() {
        return format!("{:.2}", qty);
    }
    if u == "g" || u == "kg" {
        let g = if u == "kg" { qty * Decimal::from(1000) } else { qty };
        if g >= Decimal::from(1000) {
            return format!("{:.2} kg", g / Decimal::from(1000));
        }
        return format!("{:.0} g", g);
    }
    if let Some(conv) = convert_with_density(qty, &u, "g", density_g_per_cup, ingredient) {
        if conv >= Decimal::from(1000) {
            return format!("{:.2} kg", conv / Decimal::from(1000));
        }
        return format!("{:.0} g", conv);
    }
    format!("{} {}", format_qty(qty), u)
}

pub fn format_qty(qty: Decimal) -> String {
    let s = qty.normalize().to_string();
    if s.contains('.') {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() == 2 && parts[1].chars().all(|c| c == '0') {
            return parts[0].to_string();
        }
    }
    s
}

fn to_grams(qty: Decimal, unit: &str) -> Option<Decimal> {
    match unit {
        "g" => Some(qty),
        "kg" => Some(qty * Decimal::from(1000)),
        "oz" => Some(qty * Decimal::new(283495, 4)),
        "lb" => Some(qty * Decimal::new(453592, 3)),
        "ml" => Some(qty), // water-ish 1:1 for rough kitchen use
        "l" => Some(qty * Decimal::from(1000)),
        "tsp" => Some(qty * Decimal::new(492892, 2)), // ~4.92892 g water
        "tbsp" => Some(qty * Decimal::new(147868, 1)), // ~14.7868 g
        "cup" => Some(qty * Decimal::from(240)), // water default
        _ => None,
    }
}

fn from_grams(grams: Decimal, unit: &str) -> Option<Decimal> {
    match unit {
        "g" => Some(grams),
        "kg" => Some(grams / Decimal::from(1000)),
        "oz" => Some(grams / Decimal::new(283495, 4)),
        "lb" => Some(grams / Decimal::new(453592, 3)),
        "ml" => Some(grams),
        "l" => Some(grams / Decimal::from(1000)),
        "tsp" => Some(grams / Decimal::new(492892, 2)),
        "tbsp" => Some(grams / Decimal::new(147868, 1)),
        "cup" => Some(grams / Decimal::from(240)),
        _ => None,
    }
}

/// US customary grams per cup.
/// King Arthur Baking ingredient chart (American cup / 8 oz dairy) plus USDA
/// spice/produce rows where KA has no entry. First match wins — keep specific
/// names above generic (`almond butter` before `butter`).
pub(crate) fn heuristic_density_g_per_cup(ingredient: &str) -> Option<Decimal> {
    let key = ingredient.trim().to_lowercase();
    us_g_per_cup(&key).map(Decimal::from)
}

/// Water-like density for units that are almost always liquid.
pub(crate) fn liquid_unit_g_per_cup(unit: &str) -> Option<Decimal> {
    match normalize_unit(unit).as_str() {
        "gal" | "qt" | "pt" | "fl oz" | "ml" | "l" => Some(Decimal::from(227)),
        _ => None,
    }
}

fn us_g_per_cup(key: &str) -> Option<i32> {
    const RULES: &[(&[&str], i32)] = &[
        // nut / seed butters before "butter" / "oil"
        (&["almond butter"], 272),
        (&["peanut butter"], 270),
        (&["cashew butter"], 256),
        (&["tahini", "sesame paste"], 256),
        (&["brown sugar"], 213),
        (&["powdered sugar", "confectioners", "confectioner"], 113),
        (&["cane sugar", "granulated"], 198),
        (&["sour cream"], 227),
        (&["cream cheese"], 227),
        (&["mayonnaise", "mayo", "veganaise", "vegenaise"], 226),
        (
            &[
                "heavy cream",
                "whipping cream",
                "half and half",
                "half-and-half",
            ],
            227,
        ),
        (&["buttermilk"], 227),
        (&["yogurt"], 227),
        (&["ricotta"], 227),
        (&["coconut milk"], 241),
        (&["evaporated milk"], 226),
        (&["soy creamer", "oat creamer", "creamer"], 227),
        (&["whole milk", "2% milk", "skim milk"], 227),
        (&["earth balance"], 226),
        (&["kosher salt"], 256),
        (&["almond flour"], 96),
        (&["whole wheat"], 113),
        (&["wheat germ"], 99),
        (&["wheat bran", "wheat berry", "wheat berries"], 184),
        (&["cornstarch", "corn starch"], 112),
        (&["cocoa"], 84),
        (&["olive oil"], 200),
        (&["coconut oil"], 226),
        (&["sesame oil"], 218),
        (&["shortening"], 184),
        (&["canola", "vegetable oil"], 198),
        (&["agave"], 336),
        (&["honey"], 336),
        (&["maple syrup"], 312),
        (&["brown rice syrup"], 340),
        (&["molasses"], 340),
        (&["corn syrup"], 312),
        (&["preserves", "jam", "jelly"], 340),
        (&["lemon juice", "lime juice"], 224),
        (&["orange juice", "apple juice", "pineapple juice", "cranberry juice"], 248),
        (&["apple cider"], 248),
        (&["cider vinegar"], 227),
        (&["tamari", "soy sauce"], 245),
        (&["worcestershire"], 285),
        (&["sriracha"], 245),
        (&["hot sauce", "frank"], 245),
        (&["barbecue sauce", "bbq sauce"], 272),
        (&["ketchup", "catsup"], 272),
        (&["dijon", "mustard"], 255),
        (&["horseradish"], 240),
        (&["pesto"], 224),
        (&["tomato paste"], 262),
        (&["tomato sauce"], 227),
        (&["sun dried tomato", "sundried tomato", "sun-dried tomato"], 164),
        (&["pumpkin seed"], 142),
        (&["pumpkin puree", "canned pumpkin", "pumpkin"], 227),
        (&["vanilla"], 224),
        (&["almond extract"], 224),
        (&["no chicken base", "chicken base", "better than bouillon"], 288),
        (&["liquid smoke"], 227),
        (&["mirin"], 230),
        (&["panko"], 50),
        (&["graham"], 99),
        (&["bread crumb", "breadcrumb"], 112),
        (&["rolled oat", "oats", "oatmeal"], 99),
        (&["elbow macaroni", "macaroni"], 140),
        (&["cous cous", "couscous"], 191),
        (&["bulgar", "bulgur"], 152),
        (&["brown rice", "basmati", "white rice", "jasmine"], 198),
        (&["quinoa"], 177),
        (&["french lentil", "lentil"], 191),
        (&["nutritional yeast"], 45),
        (&["sliced almond"], 85),
        (&["slivered almond"], 113),
        (&["pine nut", "pignoli"], 135),
        (&["sesame seed"], 142),
        (&["sunflower seed"], 140),
        (&["unsweetened coconut", "shredded coconut"], 85),
        (&["kalamata", "olive"], 142),
        (&["caper"], 142),
        (&["dried cranberr"], 114),
        (&["black currant", "currant"], 161),
        (&["dried apricot", "apricot"], 128),
        (&["dried cherr"], 142),
        (&["pitted date", "dates"], 149),
        (&["chocolate chip", "chocolate chunk", "choclate", "calet", "semisweet", "semi d"], 170),
        (&["chipotle"], 110),
        (&["grated asiago", "asiago", "romano", "parmesan"], 100),
        (&["monterey jack", "jack cheese"], 113),
        (&["shredded mozzerella", "shredded mozzarella", "mozzerella"], 113),
        (&["feta"], 113),
        (&["stock", "broth", "water"], 227),
        (&["brewed coffee", "coffee"], 227),
        (&["starter", "sourdough"], 227),
        (&["baking powder"], 192),
        (&["baking soda"], 288),
        (&["garlic powder", "granulated garlic"], 149),
        (&["onion powder"], 115),
        (&["minced garlic", "garlic minced"], 224),
        (
            &[
                "fresh rosemary",
                "fresh dill",
                "fresh mint",
                "fresh thyme",
                "fresh sage",
                "fresh chives",
                "fresh basil",
                "fresh oregano",
            ],
            30,
        ),
        (&["dried sage", "dried rosemary", "dried thyme", "dried oregano", "dried basil", "dried parsley", "dried chives", "dried tarragon", "oreagno"], 48),
        (&["dill weed"], 48),
        (&["poultry seasoning", "garam masala", "cajun", "cajuin", "tuscan seasoning"], 110),
        (&["ground nutmeg", "nutmeg", "cardamom", "fennel seed", "caraway"], 110),
        (&["lemon zest", "orange zest", "lime zest", "zest"], 96),
        (&["garlic"], 224),
        (&["fresh ginger", "ginger"], 228),
        (&["oregano", "basil", "thyme", "rosemary", "sage", "tarragon", "chives", "mint", "dill"], 48),
        (&["crushed red pepper", "crushed hot pepper", "pepper flake", "red pepper flake"], 110),
        (&["bell pepper", "green pepper", "red pepper", "yellow pepper", "jalapeño", "jalapeno"], 150),
        (
            &[
                "black pepper",
                "white pepper",
                "cayenne",
                "pepper",
                "paprika",
                "cumin",
                "turmeric",
                "chili powder",
                "coriander",
                "allspice",
                "cloves",
                "curry",
                "cinnamon",
            ],
            110,
        ),
        (&["scallion", "green onion"], 100),
        (&["cabbage"], 89),
        (&["zucchini"], 142),
        (&["potato", "potatoes"], 170),
        (&["mushroom"], 78),
        (&["spinach", "arugula"], 30),
        (&["broccoli"], 71),
        (&["romaine", "lettuce"], 47),
        (&["artichoke"], 168),
        (&["frozen pea", "peas"], 145),
        (&["frozen corn", "corn kernel", "fresh corn"], 152),
        (&["blueberry", "blueberries"], 170),
        (&["cranberries", "cranberry"], 100),
        (&["pineapple"], 170),
        (&["apple"], 113),
        (&["onion"], 142),
        (&["carrot"], 142),
        (&["celery"], 142),
        (&["parsley", "cilantro"], 60),
        (&["egg white", "egg whites"], 243),
        (&["seitan"], 140),
        (&["chicken breast", "cooked chicken"], 140),
        (&["cream"], 227),
        (&["milk"], 227),
        (&["sugar"], 198),
        (&["salt"], 288),
        (&["flour"], 120),
        (&["oil"], 198),
        (&["butter"], 226),
        (&["cheddar", "mozzarella", "swiss"], 113),
        (&["raisin"], 149),
        (&["almond", "pecan", "walnut", "cashew"], 113),
        (&["coconut"], 85),
        (&["chocolate"], 170),
        (&["cornmeal", "polenta", "grits"], 138),
        (&["wine", "bourbon", "beer"], 227),
        (&["vinegar"], 227),
        (&["tomato"], 227),
        (&["rice"], 198),
        (&["oat"], 99),
    ];
    for (needles, g) in RULES {
        if needles.iter().any(|n| key.contains(n)) {
            return Some(*g);
        }
    }
    category_g_per_cup(key)
}

fn category_g_per_cup(key: &str) -> Option<i32> {
    const CATS: &[(&[&str], i32)] = &[
        (&["panko"], 50),
        (&["crumb"], 112),
        (&["juice", "cider"], 248),
        (
            &[
                "wine", "beer", "bourbon", "rum", "vodka", "whiskey", "whisky", "brandy",
                "sherry", "vermouth", "tequila", "liquor", "liqueur",
            ],
            227,
        ),
        (&["extract"], 224),
        (&["syrup", "nectar"], 336),
        (&["mustard"], 255),
        (&["sauce", "dressing", "salsa", "sofrito"], 245),
        (&["paste", "base"], 262),
        (&["seasoning", "masala", "spice"], 110),
        (&["seed"], 142),
        (&["zest"], 96),
        (&["pasta", "macaroni", "noodle", "spaghetti", "penne", "orzo", "lasagna"], 140),
        (&["quinoa", "couscous", "farro", "barley", "millet", "bulgur", "bulgar"], 185),
        (&["lentil", "bean", "chickpea", "garbanzo"], 191),
        (&["cheese"], 113),
        (&["olive", "caper"], 142),
        (&["yeast"], 45),
        (&["powder"], 110),
        (&["flake"], 45),
        (&["kernel"], 152),
        (&["puree", "purée"], 227),
        (&["coffee"], 227),
        (&["ham"], 140),
    ];
    for (needles, g) in CATS {
        if needles.iter().any(|n| key.contains(n)) {
            return Some(*g);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn converts_lb_to_oz() {
        let oz = convert(Decimal::ONE, "lb", "oz").unwrap();
        assert_eq!(oz, Decimal::from(16));
    }

    #[test]
    fn converts_cup_flour_to_g() {
        let g = convert_with_ingredient(Decimal::ONE, "cup", "g", Some("bread flour")).unwrap();
        assert_eq!(g, Decimal::from(120));
    }

    #[test]
    fn us_cup_sour_cream_is_eight_ounces() {
        let g = convert_with_ingredient(Decimal::ONE, "cup", "g", Some("sour cream")).unwrap();
        assert_eq!(g, Decimal::from(227));
    }

    #[test]
    fn almond_butter_is_not_dairy_butter() {
        let g = convert_with_ingredient(Decimal::ONE, "cup", "g", Some("almond butter")).unwrap();
        assert_eq!(g, Decimal::from(272));
    }

    #[test]
    fn orange_juice_and_panko() {
        assert_eq!(
            convert_with_ingredient(Decimal::ONE, "cup", "g", Some("orange juice")).unwrap(),
            Decimal::from(248)
        );
        assert_eq!(
            convert_with_ingredient(Decimal::ONE, "cup", "g", Some("panko bread crumbs")).unwrap(),
            Decimal::from(50)
        );
    }

    #[test]
    fn dijon_and_plain_cream() {
        assert_eq!(
            convert_with_ingredient(Decimal::ONE, "cup", "g", Some("Dijon mustard")).unwrap(),
            Decimal::from(255)
        );
        assert_eq!(
            convert_with_ingredient(Decimal::ONE, "cup", "g", Some("cream")).unwrap(),
            Decimal::from(227)
        );
    }

    #[test]
    fn bell_pepper_is_produce_not_spice() {
        assert_eq!(
            convert_with_ingredient(Decimal::ONE, "cup", "g", Some("red bell pepper")).unwrap(),
            Decimal::from(150)
        );
    }

    #[test]
    fn master_density_overrides_heuristic() {
        let g = convert_with_density(
            Decimal::ONE,
            "cup",
            "g",
            Some(Decimal::from(150)),
            Some("bread flour"),
        )
        .unwrap();
        assert_eq!(g, Decimal::from(150));
    }
}
