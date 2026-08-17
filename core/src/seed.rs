use anyhow::Result;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::DEFAULT_USER_ID;
use crate::models::{Difficulty, MealType, Recipe, RecipeIngredient, RecipeStep, Tag};
use crate::services::{IngredientMasterService, MealPlanService, RecipeService, TagService};

pub async fn seed_if_empty(pool: &SqlitePool) -> Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM recipes WHERE user_id = ?")
        .bind(DEFAULT_USER_ID.to_string())
        .fetch_one(pool)
        .await?;

    if count.0 > 0 {
        return Ok(());
    }

    let recipes = RecipeService::new(pool.clone());
    let meal_plans = MealPlanService::new(pool.clone());
    let masters = IngredientMasterService::new(pool.clone());
    let today = Utc::now().date_naive();

    let samples: [(&str, &str, MealType, u32, u32, u32, Difficulty, u8, Option<&str>, &[(&str, &str, &str, Option<&str>)], &[&str]); 4] = [
        (
            "Scrambled Eggs",
            "Quick morning eggs with butter and salt.",
            MealType::Breakfast,
            2,
            2,
            5,
            Difficulty::Easy,
            4,
            Some("6.00"),
            &[
                ("eggs", "3", "large", Some("1.20")),
                ("butter", "1", "tbsp", Some("0.25")),
                ("salt", "1", "pinch", Some("0.02")),
            ],
            &[
                "Beat eggs with a pinch of salt.",
                "Melt butter in a nonstick pan over medium heat.",
                "Pour in eggs. Stir gently until just set, about 3 minutes. Serve immediately.",
            ],
        ),
        (
            "Turkey Sandwich",
            "Simple deli sandwich with mustard.",
            MealType::Lunch,
            1,
            5,
            0,
            Difficulty::Easy,
            4,
            Some("8.00"),
            &[
                ("bread", "2", "slices", Some("0.60")),
                ("turkey", "4", "oz", Some("2.50")),
                ("mustard", "1", "tbsp", Some("0.15")),
                ("lettuce", "2", "leaves", Some("0.20")),
            ],
            &[
                "Spread mustard on one slice of bread.",
                "Layer turkey and lettuce. Top with the second slice, cut in half, and serve.",
            ],
        ),
        (
            "Garlic Pasta",
            "Olive oil, garlic, and parmesan.",
            MealType::Dinner,
            2,
            5,
            15,
            Difficulty::Easy,
            5,
            Some("12.00"),
            &[
                ("pasta", "200", "g", Some("1.20")),
                ("olive oil", "2", "tbsp", Some("0.40")),
                ("garlic", "3", "cloves", Some("0.15")),
                ("parmesan", "0.25", "cup", Some("0.75")),
            ],
            &[
                "Boil salted water and cook pasta until al dente. Reserve 1/2 cup pasta water.",
                "Warm olive oil in a pan. Add minced garlic and cook 1 minute.",
                "Toss pasta with garlic oil, a splash of pasta water, and parmesan. Serve.",
            ],
        ),
        (
            "Apple and Peanut Butter",
            "Sliced apple with peanut butter for dipping.",
            MealType::Snack,
            1,
            3,
            0,
            Difficulty::Easy,
            4,
            Some("4.00"),
            &[
                ("apple", "1", "medium", Some("0.80")),
                ("peanut butter", "2", "tbsp", Some("0.35")),
            ],
            &[
                "Core the apple and slice into wedges.",
                "Serve with peanut butter on the side for dipping.",
            ],
        ),
    ];

    for (name, description, meal_type, servings, prep, cook, difficulty, rating, menu_price, ingredients, steps) in
        samples
    {
        let recipe = Recipe {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: Some(description.to_string()),
            image_url: None,
            servings,
            prep_time_minutes: Some(prep),
            cook_time_minutes: Some(cook),
            total_time_minutes: Some(prep + cook),
            source_url: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: DEFAULT_USER_ID,
            rating: Some(rating),
            difficulty: Some(difficulty),
            menu_price: menu_price.and_then(|p| p.parse().ok()),
            yield_quantity: None,
            yield_unit: None,
            waste_percent: None,
            author: None,
            estimated_calories: None,
            allergens: None,
            last_opened_at: None,
            open_count: None,
        };

        let recipe_id = recipes.create_recipe(&recipe).await?;

        for (ingredient, qty, unit, line_cost) in ingredients.iter() {
            let display = format!("{} {} {}", qty, unit, ingredient);
            let qty_dec = qty.parse::<Decimal>().ok();
            let line = line_cost.and_then(|c| c.parse::<Decimal>().ok());
            // Derive unit cost for master so price roll-through works
            let cpu = match (line, qty_dec) {
                (Some(lc), Some(q)) if q > Decimal::ZERO => Some(lc / q),
                _ => None,
            };
            let master = masters
                .find_or_create(ingredient, Some(unit), cpu)
                .await?;
            recipes
                .add_ingredient(&RecipeIngredient {
                    id: Uuid::new_v4(),
                    recipe_id,
                    ingredient: ingredient.to_string(),
                    quantity: qty_dec,
                    unit: Some(unit.to_string()),
                    note: None,
                    display,
                    category: None,
                    cost_per_unit: master.cost_per_unit.or(cpu),
                    line_cost: None,
                    master_ingredient_id: Some(master.id),
                    master_name: Some(master.name.clone()),
                    master_g_per_cup: None,
                    prep_yield_percent: None,
                })
                .await?;
        }

        for (position, instruction) in steps.iter().enumerate() {
            recipes
                .add_step(&RecipeStep {
                    id: Uuid::new_v4(),
                    recipe_id,
                    position: (position + 1) as u32,
                    instruction: (*instruction).to_string(),
                    timer_seconds: None,
                })
                .await?;
        }

        let tag_name = match meal_type {
            MealType::Breakfast => "breakfast",
            MealType::Lunch => "lunch",
            MealType::Dinner => "dinner",
            MealType::Snack => "snack",
        };
        recipes
            .add_tags(
                recipe_id,
                vec![Tag {
                    id: Uuid::new_v4(),
                    name: tag_name.to_string(),
                    color: None,
                }],
            )
            .await?;

        meal_plans
            .set_recipe(DEFAULT_USER_ID, today, meal_type, recipe_id)
            .await?;
    }

    Ok(())
}

/// Idempotent: ensure a bakery-tagged demo recipe exists (for dept filter demos).
pub async fn ensure_bakery_demo(pool: &SqlitePool) -> Result<()> {
    let bakery_tagged: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM recipe_tags rt
        JOIN tags t ON t.id = rt.tag_id
        WHERE lower(t.name) = 'bakery'
        "#,
    )
    .fetch_one(pool)
    .await?;
    if bakery_tagged.0 > 0 {
        return Ok(());
    }
    // Also skip if the demo recipe already exists under another tag / prior seed.
    if recipe_named_exists(pool, "Country Sourdough Loaf").await? {
        let tags = TagService::new(pool.clone());
        if let Some(id) = recipe_id_by_name(pool, "Country Sourdough Loaf").await? {
            tags.add_to_recipe(id, "bakery").await?;
        }
        return Ok(());
    }

    let recipes = RecipeService::new(pool.clone());
    let masters = IngredientMasterService::new(pool.clone());
    let tags = TagService::new(pool.clone());

    let recipe = Recipe {
        id: Uuid::new_v4(),
        name: "Country Sourdough Loaf".to_string(),
        description: Some(
            "Standard bakery pull — starter feed, mix, bulk ferment, shape, and bake.".into(),
        ),
        image_url: None,
        servings: 2,
        prep_time_minutes: Some(25),
        cook_time_minutes: Some(45),
        total_time_minutes: Some(70),
        source_url: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        user_id: DEFAULT_USER_ID,
        rating: Some(5),
        difficulty: Some(Difficulty::Medium),
        menu_price: Some("8.00".parse().unwrap()),
        yield_quantity: Some(Decimal::from(2)),
        yield_unit: Some("loaf".into()),
        waste_percent: Some(Decimal::from(5)),
        author: Some("Bakery".into()),
        estimated_calories: Some(220),
        allergens: Some("gluten, dairy, egg".into()),
        last_opened_at: None,
        open_count: None,
    };

    let recipe_id = recipes.create_recipe(&recipe).await?;

    let ingredients: [(&str, &str, &str, Option<&str>); 4] = [
        ("bread flour", "500", "g", Some("2.50")),
        ("water", "350", "g", Some("0.00")),
        ("sourdough starter", "100", "g", Some("0.80")),
        ("salt", "10", "g", Some("0.05")),
    ];

    for (ingredient, qty, unit, line_cost) in ingredients.iter() {
        let display = format!("{} {} {}", qty, unit, ingredient);
        let qty_dec = qty.parse::<Decimal>().ok();
        let line = line_cost.and_then(|c| c.parse::<Decimal>().ok());
        let cpu = match (line, qty_dec) {
            (Some(lc), Some(q)) if q > Decimal::ZERO => Some(lc / q),
            _ => None,
        };
        let master = masters
            .find_or_create(ingredient, Some(unit), cpu)
            .await?;
        recipes
            .add_ingredient(&RecipeIngredient {
                id: Uuid::new_v4(),
                recipe_id,
                ingredient: ingredient.to_string(),
                quantity: qty_dec,
                unit: Some(unit.to_string()),
                note: None,
                display,
                category: None,
                cost_per_unit: master.cost_per_unit.or(cpu),
                line_cost: None,
                master_ingredient_id: Some(master.id),
                master_name: Some(master.name.clone()),
                master_g_per_cup: None,
                prep_yield_percent: None,
            })
            .await?;
    }

    let steps = [
        "Mix flour and water; autolyse 30 minutes.",
        "Add starter and salt. Stretch-and-fold every 30 minutes for 2 hours.",
        "Shape into loaves; proof 2–4 hours until puffy.",
        "Score and bake at 450°F with steam for 20 minutes, then 425°F for 25 minutes.",
    ];
    for (position, instruction) in steps.iter().enumerate() {
        recipes
            .add_step(&RecipeStep {
                id: Uuid::new_v4(),
                recipe_id,
                position: (position + 1) as u32,
                instruction: (*instruction).to_string(),
                timer_seconds: None,
            })
            .await?;
    }

    tags.add_to_recipe(recipe_id, "bakery").await?;
    Ok(())
}

/// Idempotent: ensure department tags exist so quick filters never come back empty.
pub async fn ensure_department_tags(pool: &SqlitePool) -> Result<()> {
    let tags = TagService::new(pool.clone());
    for name in ["bakery", "breakfast", "lunch", "dinner", "deli", "snack"] {
        tags.get_or_create(name).await?;
    }
    // Heal known bakery cookie tags.
    for (recipe_name, tag) in [
        ("Best Chocolate Chip Cookies", "bakery"),
        ("Peanut Butter Cookies", "bakery"),
        ("White Chocolate Macadamia Cookies", "bakery"),
    ] {
        if let Some(id) = recipe_id_by_name(pool, recipe_name).await? {
            tags.add_to_recipe(id, tag).await?;
        }
    }
    Ok(())
}

/// Idempotent: pantry items + high-protein house/channel recipes. Sweets limited to PB / CC / macadamia cookies.
pub async fn ensure_stable_catalog(pool: &SqlitePool) -> Result<()> {
    ensure_department_tags(pool).await?;
    ensure_common_ingredients(pool).await?;
    prune_off_focus_recipes(pool).await?;
    ensure_stable_recipes(pool).await?;
    ensure_channel_recipes(pool).await?;
    Ok(())
}

/// Drop sugary / low-protein filler recipes so the board stays high-protein (+ three bakery cookies only).
async fn prune_off_focus_recipes(pool: &SqlitePool) -> Result<()> {
    const DROP: &[&str] = &[
        "Apple Pie",
        "Banana Bread",
        "PB&J with Banana",
        "Grilled Cheese",
        "Pancakes",
        "Caesar Salad",
        "Mac and Cheese",
        "Oatmeal with Banana",
        "Country Sourdough Loaf",
        "3-Ingredient Protein Peanut Butter Balls",
        "Salted Caramel Protein Cookies",
        "Levain-Style Protein Cookie",
        "Mug Chocolate Lava Cake",
        "Low-Calorie Gooey Protein Brownies",
        "White Chocolate Raspberry Protein Cookies",
        "Date Peanut Butter Protein Bars",
        "Vanilla Protein Soft Serve Pint",
    ];
    for name in DROP {
        if let Some(id) = recipe_id_by_name(pool, name).await? {
            delete_recipe_cascade(pool, id).await?;
            tracing::info!("Pruned off-focus recipe: {name}");
        }
    }
    Ok(())
}

async fn delete_recipe_cascade(pool: &SqlitePool, id: Uuid) -> Result<()> {
    let id_s = id.to_string();
    for sql in [
        "DELETE FROM recipe_ingredients WHERE recipe_id = ?",
        "DELETE FROM recipe_steps WHERE recipe_id = ?",
        "DELETE FROM recipe_tags WHERE recipe_id = ?",
        "DELETE FROM meal_plans WHERE recipe_id = ?",
        "DELETE FROM cookbook_recipes WHERE recipe_id = ?",
        "DELETE FROM production_plan_items WHERE recipe_id = ?",
        "DELETE FROM recipes WHERE id = ?",
    ] {
        sqlx::query(sql).bind(&id_s).execute(pool).await?;
    }
    Ok(())
}

async fn ensure_common_ingredients(pool: &SqlitePool) -> Result<()> {
    let masters = IngredientMasterService::new(pool.clone());
    // name, default_unit, rough cost_per_unit (USD)
    let items: &[(&str, &str, &str)] = &[
        // Produce
        ("apple", "each", "0.80"),
        ("banana", "each", "0.30"),
        ("lemon", "each", "0.60"),
        ("lime", "each", "0.40"),
        ("orange", "each", "0.70"),
        ("onion", "each", "0.50"),
        ("garlic", "clove", "0.08"),
        ("potato", "lb", "0.80"),
        ("carrot", "lb", "0.90"),
        ("celery", "bunch", "1.80"),
        ("tomato", "each", "0.70"),
        ("romaine lettuce", "head", "2.00"),
        ("spinach", "oz", "0.25"),
        ("cucumber", "each", "0.90"),
        ("bell pepper", "each", "1.20"),
        ("mushroom", "oz", "0.40"),
        ("avocado", "each", "1.50"),
        ("broccoli", "lb", "1.80"),
        ("cabbage", "lb", "0.70"),
        ("ginger", "oz", "0.50"),
        ("jalapeño", "each", "0.25"),
        ("cilantro", "bunch", "1.00"),
        ("parsley", "bunch", "1.00"),
        ("green onion", "bunch", "0.80"),
        ("blueberry", "pint", "3.50"),
        ("strawberry", "lb", "3.00"),
        ("grape", "lb", "2.50"),
        // Dairy & eggs
        ("eggs", "each", "0.35"),
        ("butter", "tbsp", "0.15"),
        ("whole milk", "cup", "0.30"),
        ("heavy cream", "cup", "1.20"),
        ("sour cream", "cup", "1.00"),
        ("yogurt", "cup", "0.80"),
        ("cheddar cheese", "oz", "0.40"),
        ("swiss cheese", "oz", "0.50"),
        ("american cheese", "slice", "0.25"),
        ("parmesan", "oz", "0.60"),
        ("cream cheese", "oz", "0.35"),
        ("mozzarella", "oz", "0.40"),
        // Meat & deli
        ("bacon", "slice", "0.45"),
        ("turkey breast", "oz", "0.45"),
        ("ham", "oz", "0.40"),
        ("roast beef", "oz", "0.55"),
        ("chicken breast", "lb", "3.50"),
        ("ground beef", "lb", "5.00"),
        ("ground turkey", "lb", "4.50"),
        ("pork loin", "lb", "4.00"),
        ("salmon fillet", "lb", "12.00"),
        ("tuna canned", "can", "1.50"),
        // Bakery / grains
        ("sandwich bread", "slice", "0.25"),
        ("sourdough bread", "slice", "0.40"),
        ("hamburger bun", "each", "0.40"),
        ("tortilla", "each", "0.30"),
        ("pita", "each", "0.50"),
        ("all-purpose flour", "cup", "0.20"),
        ("bread flour", "g", "0.005"),
        ("whole wheat flour", "cup", "0.25"),
        ("rolled oats", "cup", "0.40"),
        ("rice", "cup", "0.50"),
        ("pasta", "oz", "0.15"),
        ("spaghetti", "oz", "0.15"),
        ("quinoa", "cup", "1.20"),
        ("cornmeal", "cup", "0.35"),
        ("breadcrumbs", "cup", "0.50"),
        ("pie crust", "each", "2.50"),
        ("graham cracker crumbs", "cup", "1.00"),
        // Pantry
        ("peanut butter", "tbsp", "0.18"),
        ("grape jelly", "tbsp", "0.12"),
        ("strawberry jam", "tbsp", "0.15"),
        ("mayonnaise", "tbsp", "0.10"),
        ("mustard", "tbsp", "0.08"),
        ("ketchup", "tbsp", "0.06"),
        ("soy sauce", "tbsp", "0.08"),
        ("hot sauce", "tbsp", "0.10"),
        ("olive oil", "tbsp", "0.20"),
        ("vegetable oil", "tbsp", "0.08"),
        ("vinegar", "tbsp", "0.05"),
        ("apple cider vinegar", "tbsp", "0.06"),
        ("honey", "tbsp", "0.20"),
        ("maple syrup", "tbsp", "0.35"),
        ("sugar", "cup", "0.25"),
        ("brown sugar", "cup", "0.35"),
        ("powdered sugar", "cup", "0.40"),
        ("baking powder", "tsp", "0.05"),
        ("baking soda", "tsp", "0.03"),
        ("yeast", "tsp", "0.10"),
        ("vanilla extract", "tsp", "0.25"),
        ("cocoa powder", "tbsp", "0.20"),
        ("chocolate chips", "cup", "1.50"),
        ("walnuts", "cup", "2.00"),
        ("almonds", "cup", "2.50"),
        ("raisins", "cup", "1.20"),
        ("chicken stock", "cup", "0.50"),
        ("beef stock", "cup", "0.55"),
        ("tomato sauce", "cup", "0.60"),
        ("diced tomatoes", "can", "1.20"),
        ("black beans", "can", "1.00"),
        ("chickpeas", "can", "1.00"),
        ("coconut milk", "can", "1.80"),
        // Spices & seasonings
        ("salt", "tsp", "0.01"),
        ("black pepper", "tsp", "0.05"),
        ("cinnamon", "tsp", "0.08"),
        ("nutmeg", "tsp", "0.10"),
        ("paprika", "tsp", "0.08"),
        ("cumin", "tsp", "0.08"),
        ("chili powder", "tsp", "0.08"),
        ("oregano", "tsp", "0.08"),
        ("basil", "tsp", "0.10"),
        ("thyme", "tsp", "0.10"),
        ("rosemary", "tsp", "0.12"),
        ("garlic powder", "tsp", "0.06"),
        ("onion powder", "tsp", "0.06"),
        ("cayenne", "tsp", "0.08"),
        ("bay leaf", "each", "0.05"),
        ("red pepper flakes", "tsp", "0.06"),
        // Fridge extras / deli sides
        ("pickles", "slice", "0.10"),
        ("olives", "oz", "0.40"),
        ("hummus", "oz", "0.35"),
        ("salsa", "cup", "1.00"),
        ("tofu", "oz", "0.25"),
        ("sourdough starter", "g", "0.008"),
        ("water", "cup", "0.00"),
        ("iceberg lettuce", "leaf", "0.10"),
        ("tomato slice", "slice", "0.15"),
        ("pie filling apple", "can", "2.50"),
        // High-protein / bakery extras (YouTube-sourced recipes)
        ("greek yogurt", "g", "0.004"),
        ("self-rising flour", "g", "0.002"),
        ("whey protein powder", "g", "0.05"),
        ("casein protein powder", "g", "0.055"),
        ("peanut butter powder", "g", "0.015"),
        ("erythritol", "g", "0.01"),
        ("applesauce", "g", "0.002"),
        ("pumpkin puree", "g", "0.003"),
        ("gelatin", "g", "0.03"),
        ("coconut oil", "tbsp", "0.15"),
        ("chocolate", "g", "0.02"),
        ("oat flour", "g", "0.004"),
        ("sugar-free maple syrup", "g", "0.02"),
        ("chicken thigh", "lb", "2.80"),
        ("frozen peppers and onions", "lb", "2.00"),
        ("frozen sweet corn", "oz", "0.10"),
        ("taco seasoning", "packet", "0.90"),
        ("pizza sauce", "g", "0.004"),
        ("pork tenderloin", "oz", "0.35"),
        ("vital wheat gluten", "g", "0.01"),
        ("crushed tomatoes", "can", "1.50"),
        ("fat-free mozzarella", "g", "0.012"),
        ("cornstarch", "g", "0.003"),
        ("chicken wing", "each", "0.55"),
        ("russet potato", "g", "0.002"),
        ("worcestershire sauce", "tbsp", "0.08"),
        ("dates", "each", "0.20"),
        ("tomato paste", "g", "0.008"),
        ("beef bone broth", "g", "0.003"),
        ("ground pork", "g", "0.008"),
        ("english muffin", "each", "0.60"),
        ("light butter", "g", "0.012"),
        ("toasted sesame oil", "tsp", "0.10"),
        ("white chocolate chips", "cup", "2.00"),
        ("macadamia nuts", "cup", "4.00"),
        ("00 flour", "g", "0.003"),
        ("fresh mozzarella", "g", "0.02"),
        ("basil", "leaf", "0.05"),
    ];

    for (name, unit, cost) in items {
        let cpu = cost.parse::<Decimal>().ok();
        masters.find_or_create(name, Some(unit), cpu).await?;
    }
    Ok(())
}

async fn recipe_named_exists(pool: &SqlitePool, name: &str) -> Result<bool> {
    Ok(recipe_id_by_name(pool, name).await?.is_some())
}

async fn recipe_id_by_name(pool: &SqlitePool, name: &str) -> Result<Option<Uuid>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM recipes WHERE lower(name) = lower(?) LIMIT 1")
            .bind(name)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(id,)| Uuid::parse_str(&id).ok()))
}

struct StableRecipe {
    name: &'static str,
    description: &'static str,
    meal_type: MealType,
    tag: &'static str,
    servings: u32,
    prep: u32,
    cook: u32,
    difficulty: Difficulty,
    menu_price: &'static str,
    author: Option<&'static str>,
    source_url: Option<&'static str>,
    estimated_calories: Option<u32>,
    allergens: Option<&'static str>,
    ingredients: &'static [(&'static str, &'static str, &'static str, Option<&'static str>)],
    steps: &'static [&'static str],
}

async fn ensure_stable_recipes(pool: &SqlitePool) -> Result<()> {
    let recipes = RecipeService::new(pool.clone());
    let masters = IngredientMasterService::new(pool.clone());
    let tags = TagService::new(pool.clone());

    // High-protein savory staples + exactly three bakery sweets (PB, chocolate chip, macadamia).
    let catalog: &[StableRecipe] = &[
        StableRecipe {
            name: "Club Sandwich",
            description: "Triple-decker turkey club with bacon, lettuce, tomato, and mayo — protein-forward deli plate.",
            meal_type: MealType::Lunch,
            tag: "deli",
            servings: 1,
            prep: 15,
            cook: 10,
            difficulty: Difficulty::Easy,
            menu_price: "9.50",
            author: Some("Larder Kitchen"),
            source_url: None,
            estimated_calories: Some(520),
            allergens: None,
            ingredients: &[
                ("sandwich bread", "3", "slice", Some("0.75")),
                ("turkey breast", "4", "oz", Some("1.80")),
                ("bacon", "3", "slice", Some("1.35")),
                ("iceberg lettuce", "2", "leaf", Some("0.20")),
                ("tomato", "3", "slice", Some("0.45")),
                ("mayonnaise", "2", "tbsp", Some("0.20")),
                ("salt", "1", "pinch", Some("0.01")),
                ("black pepper", "1", "pinch", Some("0.01")),
            ],
            steps: &[
                "Toast bread. Cook bacon until crisp; drain.",
                "Spread mayo on all three slices. Stack turkey, lettuce, and tomato on the first slice; season.",
                "Add second slice, then bacon and more lettuce/tomato. Top with third slice; secure with picks and cut into quarters.",
            ],
        },
        StableRecipe {
            name: "Chicken Noodle Soup",
            description: "Clear broth soup with chicken, noodles, carrot, and celery.",
            meal_type: MealType::Dinner,
            tag: "dinner",
            servings: 6,
            prep: 15,
            cook: 35,
            difficulty: Difficulty::Easy,
            menu_price: "7.00",
            author: Some("Larder Kitchen"),
            source_url: None,
            estimated_calories: Some(280),
            allergens: None,
            ingredients: &[
                ("chicken breast", "1", "lb", Some("3.50")),
                ("chicken stock", "8", "cup", Some("4.00")),
                ("carrot", "2", "each", Some("0.60")),
                ("celery", "2", "stalk", Some("0.40")),
                ("onion", "1", "each", Some("0.50")),
                ("pasta", "6", "oz", Some("0.90")),
                ("parsley", "2", "tbsp", Some("0.15")),
                ("salt", "1", "tsp", Some("0.01")),
                ("black pepper", "0.5", "tsp", Some("0.03")),
            ],
            steps: &[
                "Simmer chicken in stock until cooked through; remove, shred, and return.",
                "Add diced onion, carrot, and celery; simmer 15 minutes. Add noodles and cook until tender.",
                "Season and finish with parsley.",
            ],
        },
        StableRecipe {
            name: "Peanut Butter Cookies",
            description: "House bakery peanut butter cookies — one of three allowed sweets on the board.",
            meal_type: MealType::Snack,
            tag: "bakery",
            servings: 24,
            prep: 15,
            cook: 12,
            difficulty: Difficulty::Easy,
            menu_price: "1.50",
            author: Some("Bakery"),
            source_url: None,
            estimated_calories: Some(95),
            allergens: Some("gluten, dairy, egg"),
            ingredients: &[
                ("peanut butter", "1", "cup", Some("2.88")),
                ("sugar", "0.5", "cup", Some("0.13")),
                ("brown sugar", "0.5", "cup", Some("0.18")),
                ("eggs", "1", "each", Some("0.35")),
                ("vanilla extract", "1", "tsp", Some("0.25")),
                ("all-purpose flour", "1.25", "cup", Some("0.25")),
                ("baking soda", "0.75", "tsp", Some("0.02")),
                ("salt", "0.5", "tsp", Some("0.01")),
                ("butter", "4", "tbsp", Some("0.60")),
            ],
            steps: &[
                "Preheat oven to 350°F. Cream butter with peanut butter and both sugars until fluffy; beat in egg and vanilla.",
                "Whisk flour, baking soda, and salt; mix into the peanut butter base just until combined.",
                "Scoop 1-tbsp balls onto a sheet, flatten with a fork in a criss-cross. Bake 10–12 minutes until edges set. Cool on the pan 5 minutes.",
            ],
        },
        StableRecipe {
            name: "Best Chocolate Chip Cookies",
            description: "Classic chocolate chip bakery cookies — one of three allowed sweets on the board.",
            meal_type: MealType::Snack,
            tag: "bakery",
            servings: 24,
            prep: 15,
            cook: 12,
            difficulty: Difficulty::Easy,
            menu_price: "1.75",
            author: Some("Bakery"),
            source_url: None,
            estimated_calories: Some(110),
            allergens: Some("gluten, dairy, egg"),
            ingredients: &[
                ("butter", "0.5", "cup", Some("1.20")),
                ("brown sugar", "0.75", "cup", Some("0.26")),
                ("sugar", "0.25", "cup", Some("0.06")),
                ("eggs", "1", "each", Some("0.35")),
                ("vanilla extract", "1", "tsp", Some("0.25")),
                ("all-purpose flour", "1.5", "cup", Some("0.30")),
                ("baking soda", "0.5", "tsp", Some("0.02")),
                ("salt", "0.5", "tsp", Some("0.01")),
                ("chocolate chips", "1", "cup", Some("1.50")),
            ],
            steps: &[
                "Preheat oven to 350°F. Cream butter with sugars; beat in egg and vanilla.",
                "Whisk flour, baking soda, and salt; mix in, then fold chocolate chips.",
                "Scoop onto a sheet and bake 10–12 minutes until edges are golden. Cool briefly on the pan.",
            ],
        },
        StableRecipe {
            name: "White Chocolate Macadamia Cookies",
            description: "Bakery white chocolate macadamia nut cookies — the third (and last) sweet on the board.",
            meal_type: MealType::Snack,
            tag: "bakery",
            servings: 24,
            prep: 15,
            cook: 12,
            difficulty: Difficulty::Easy,
            menu_price: "2.00",
            author: Some("Bakery"),
            source_url: None,
            estimated_calories: Some(120),
            allergens: Some("gluten, dairy, egg"),
            ingredients: &[
                ("butter", "0.5", "cup", Some("1.20")),
                ("brown sugar", "0.5", "cup", Some("0.18")),
                ("sugar", "0.5", "cup", Some("0.13")),
                ("eggs", "1", "each", Some("0.35")),
                ("vanilla extract", "1", "tsp", Some("0.25")),
                ("all-purpose flour", "1.5", "cup", Some("0.30")),
                ("baking soda", "0.5", "tsp", Some("0.02")),
                ("salt", "0.5", "tsp", Some("0.01")),
                ("white chocolate chips", "1", "cup", Some("2.00")),
                ("macadamia nuts", "0.75", "cup", Some("3.00")),
            ],
            steps: &[
                "Preheat oven to 350°F. Cream butter with sugars; beat in egg and vanilla.",
                "Whisk flour, baking soda, and salt; mix in, then fold white chocolate and chopped macadamias.",
                "Scoop onto a sheet and bake 10–12 minutes until edges set. Cool on the pan 5 minutes.",
            ],
        },
    ];

    for spec in catalog {
        upsert_catalog_recipe(pool, &recipes, &masters, &tags, spec).await?;
    }

    Ok(())
}

async fn upsert_catalog_recipe(
    pool: &SqlitePool,
    recipes: &RecipeService,
    masters: &IngredientMasterService,
    tags: &TagService,
    spec: &StableRecipe,
) -> Result<()> {
    if let Some(existing_id) = recipe_id_by_name(pool, spec.name).await? {
        // Heal tags on restart without duplicating the recipe.
        tags.add_to_recipe(existing_id, spec.tag).await?;
        return Ok(());
    }

    let recipe = Recipe {
        id: Uuid::new_v4(),
        name: spec.name.to_string(),
        description: Some(spec.description.to_string()),
        image_url: None,
        servings: spec.servings,
        prep_time_minutes: Some(spec.prep),
        cook_time_minutes: Some(spec.cook),
        total_time_minutes: Some(spec.prep + spec.cook),
        source_url: spec.source_url.map(|s| s.to_string()),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        user_id: DEFAULT_USER_ID,
        rating: Some(4),
        difficulty: Some(spec.difficulty),
        menu_price: spec.menu_price.parse().ok(),
        yield_quantity: None,
        yield_unit: None,
        waste_percent: None,
        author: spec.author.map(|s| s.to_string()),
        estimated_calories: spec.estimated_calories,
        allergens: spec.allergens.map(|s| s.to_string()),
            last_opened_at: None,
            open_count: None,
    };
    let recipe_id = recipes.create_recipe(&recipe).await?;

    for (ingredient, qty, unit, line_cost) in spec.ingredients.iter() {
        let ingredient = if *ingredient == "egg" {
            "eggs"
        } else {
            ingredient
        };
        let display = format!("{} {} {}", qty, unit, ingredient);
        let qty_dec = qty.parse::<Decimal>().ok();
        let line = line_cost.and_then(|c| c.parse::<Decimal>().ok());
        let cpu = match (line, qty_dec) {
            (Some(lc), Some(q)) if q > Decimal::ZERO => Some(lc / q),
            _ => None,
        };
        let master = masters
            .find_or_create(ingredient, Some(unit), cpu)
            .await?;
        recipes
            .add_ingredient(&RecipeIngredient {
                id: Uuid::new_v4(),
                recipe_id,
                ingredient: ingredient.to_string(),
                quantity: qty_dec,
                unit: Some(unit.to_string()),
                note: None,
                display,
                category: None,
                cost_per_unit: master.cost_per_unit.or(cpu),
                line_cost: None,
                master_ingredient_id: Some(master.id),
                master_name: Some(master.name.clone()),
                master_g_per_cup: None,
                prep_yield_percent: None,
            })
            .await?;
    }

    for (position, instruction) in spec.steps.iter().enumerate() {
        recipes
            .add_step(&RecipeStep {
                id: Uuid::new_v4(),
                recipe_id,
                position: (position + 1) as u32,
                instruction: (*instruction).to_string(),
                timer_seconds: None,
            })
            .await?;
    }

    tags.add_to_recipe(recipe_id, spec.tag).await?;
    let _ = spec.meal_type;
    Ok(())
}

/// High-protein recipes from Rahul Kamat / Exercise4CheatMeals (desc or auto-caption grams).
/// Sweets intentionally omitted — house board only allows PB / CC / macadamia cookies.
async fn ensure_channel_recipes(pool: &SqlitePool) -> Result<()> {
    let recipes = RecipeService::new(pool.clone());
    let masters = IngredientMasterService::new(pool.clone());
    let tags = TagService::new(pool.clone());

    let catalog: &[StableRecipe] = &[
        StableRecipe {
            name: "Honey Mustard Air-Fryer Pork Tenderloin",
            description: "Mustard-honey coated pork tenderloin — from Rahul Kamat video description (290 cal/serving).",
            meal_type: MealType::Dinner,
            tag: "dinner",
            servings: 2,
            prep: 5,
            cook: 20,
            difficulty: Difficulty::Easy,
            menu_price: "9.00",
            author: Some("Rahul Kamat"),
            source_url: Some("https://www.youtube.com/watch?v=TeIbsZFHZNA"),
            estimated_calories: Some(290),
            allergens: None,
            ingredients: &[
                ("pork tenderloin", "16", "oz", Some("5.60")),
                ("mustard", "1.5", "tbsp", Some("0.12")),
                ("honey", "1", "tbsp", Some("0.20")),
                ("salt", "1", "tsp", Some("0.01")),
                ("black pepper", "0.5", "tsp", Some("0.03")),
                ("garlic powder", "0.5", "tsp", Some("0.03")),
            ],
            steps: &[
                "Pat tenderloin dry. Mix mustard, honey, salt, pepper, and garlic powder; coat the pork.",
                "Air fry at 350°F for about 20 minutes until the center reaches 140°F. Rest 5 minutes, slice, and serve.",
            ],
        },
        StableRecipe {
            name: "10-Minute Greek Yogurt Protein Pizza",
            description: "Self-rising flour + Greek yogurt air-fryer pizza dough with sauce and mozzarella — Rahul Kamat.",
            meal_type: MealType::Dinner,
            tag: "dinner",
            servings: 1,
            prep: 5,
            cook: 10,
            difficulty: Difficulty::Easy,
            menu_price: "6.50",
            author: Some("Rahul Kamat"),
            source_url: Some("https://www.youtube.com/watch?v=c5uoMGBTqSo"),
            estimated_calories: Some(450),
            allergens: None,
            ingredients: &[
                ("self-rising flour", "120", "g", Some("0.24")),
                ("greek yogurt", "100", "g", Some("0.40")),
                ("pizza sauce", "80", "g", Some("0.32")),
                ("mozzarella", "56", "g", Some("0.80")),
            ],
            steps: &[
                "Mix self-rising flour and Greek yogurt into a soft dough; press into a round.",
                "Air fry at 350°F for 5 minutes. Top with pizza sauce and mozzarella; air fry 5 minutes more until cheese melts.",
            ],
        },
        StableRecipe {
            name: "Chicken Fajita Burrito Meal Prep",
            description: "Seasoned chicken thighs with peppers, onions, corn, yogurt, and cheese in tortillas — Rahul Kamat.",
            meal_type: MealType::Lunch,
            tag: "lunch",
            servings: 7,
            prep: 10,
            cook: 15,
            difficulty: Difficulty::Easy,
            menu_price: "5.50",
            author: Some("Rahul Kamat"),
            source_url: Some("https://www.youtube.com/watch?v=glvitpog9Qo"),
            estimated_calories: Some(420),
            allergens: None,
            ingredients: &[
                ("tortilla", "7", "each", Some("2.10")),
                ("chicken thigh", "2", "lb", Some("5.60")),
                ("frozen peppers and onions", "1", "lb", Some("2.00")),
                ("frozen sweet corn", "12", "oz", Some("1.20")),
                ("greek yogurt", "200", "g", Some("0.80")),
                ("cheddar cheese", "56", "g", Some("0.90")),
                ("taco seasoning", "2", "packet", Some("1.80")),
                ("salt", "1", "tsp", Some("0.01")),
            ],
            steps: &[
                "Season chicken with one taco packet and salt; cook through, then chop.",
                "Sauté peppers, onions, and corn; fold in chicken, second taco packet, yogurt, and cheese.",
                "Divide among 7 tortillas, wrap, and refrigerate or freeze for meal prep.",
            ],
        },
        StableRecipe {
            name: "High-Protein Sheet Pizza Dough",
            description: "Vital wheat gluten enriched dough with crushed tomato sauce and fat-free mozzarella — Exercise4CheatMeals.",
            meal_type: MealType::Dinner,
            tag: "dinner",
            servings: 4,
            prep: 25,
            cook: 15,
            difficulty: Difficulty::Medium,
            menu_price: "7.00",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=2qe_Pkc402s"),
            estimated_calories: Some(380),
            allergens: None,
            ingredients: &[
                ("all-purpose flour", "240", "g", Some("0.48")),
                ("vital wheat gluten", "40", "g", Some("0.40")),
                ("yeast", "7", "g", Some("0.20")),
                ("salt", "6", "g", Some("0.01")),
                ("water", "180", "g", Some("0.00")),
                ("crushed tomatoes", "1", "can", Some("1.50")),
                ("fat-free mozzarella", "200", "g", Some("2.40")),
            ],
            steps: &[
                "Mix flour, gluten, yeast, and salt; add water and knead until smooth. Rest 20–30 minutes.",
                "Press onto a sheet, top with crushed tomatoes and mozzarella, and bake hot until browned.",
            ],
        },
        StableRecipe {
            name: "Obsessed Protein Pizza",
            description: "00 flour + VWG Neapolitan-style protein pizza (~40g protein pie) — Exercise4CheatMeals captions (The Protein Pizza We're Obsessed With).",
            meal_type: MealType::Dinner,
            tag: "dinner",
            servings: 2,
            prep: 40,
            cook: 8,
            difficulty: Difficulty::Medium,
            menu_price: "8.00",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=_jxpLiSAopc"),
            estimated_calories: Some(420),
            allergens: None,
            ingredients: &[
                ("00 flour", "190", "g", Some("0.50")),
                ("vital wheat gluten", "40", "g", Some("0.40")),
                ("salt", "5", "g", Some("0.01")),
                ("yeast", "3", "g", Some("0.10")),
                ("water", "170", "g", Some("0.00")),
                ("crushed tomatoes", "70", "g", Some("0.35")),
                ("fresh mozzarella", "60", "g", Some("1.20")),
                ("parmesan", "5", "g", Some("0.15")),
                ("olive oil", "5", "g", Some("0.10")),
                ("basil", "4", "leaf", Some("0.10")),
            ],
            steps: &[
                "Whisk 00 flour, vital wheat gluten, salt, and yeast. Add water; mix and stretch-and-fold until smooth. Rest, then split into two ~200 g balls; cold ferment if time allows.",
                "Preheat steel/stone very hot. Stretch each dough to 9–11 in on parchment; add 70 g tomatoes.",
                "Par-bake briefly, add mozzarella, finish until spotted. Top with parmesan, basil, and olive oil. (~30 g protein/slice).",
            ],
        },
        StableRecipe {
            name: "Protein Pitas",
            description: "~20 g protein per pita using AP flour, VWG, and 2% Greek yogurt — Exercise4CheatMeals captions.",
            meal_type: MealType::Lunch,
            tag: "bakery",
            servings: 6,
            prep: 25,
            cook: 15,
            difficulty: Difficulty::Medium,
            menu_price: "1.25",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=Y8rdOSKtvtc"),
            estimated_calories: Some(160),
            allergens: Some("gluten, dairy, egg"),
            ingredients: &[
                ("all-purpose flour", "195", "g", Some("0.39")),
                ("vital wheat gluten", "78", "g", Some("0.78")),
                ("salt", "3.6", "g", Some("0.01")),
                ("baking powder", "4.5", "g", Some("0.05")),
                ("greek yogurt", "365", "g", Some("1.46")),
            ],
            steps: &[
                "Whisk flour, gluten, salt, and baking powder. Mix in 2% Greek yogurt until no dry flour remains.",
                "Divide into 6 balls (~107 g). Roll thin disks; cook on a hot dry skillet or bake until puffed and spotted.",
                "Cool, bag, and use for meal prep sandwiches or gyros.",
            ],
        },
        StableRecipe {
            name: "Protein Hoagie Rolls",
            description: "Turano-style high-protein hoagie rolls (bread flour + VWG) — Exercise4CheatMeals captions (~28¢/roll).",
            meal_type: MealType::Lunch,
            tag: "bakery",
            servings: 4,
            prep: 40,
            cook: 20,
            difficulty: Difficulty::Medium,
            menu_price: "1.50",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=orP2p5zS4jk"),
            estimated_calories: Some(220),
            allergens: Some("gluten, dairy, egg"),
            ingredients: &[
                ("bread flour", "240", "g", Some("0.48")),
                ("vital wheat gluten", "60", "g", Some("0.60")),
                ("yeast", "2.5", "g", Some("0.08")),
                ("salt", "5", "g", Some("0.01")),
                ("vegetable oil", "10", "g", Some("0.08")),
                ("water", "192", "g", Some("0.00")),
                ("cornmeal", "10", "g", Some("0.05")),
            ],
            steps: &[
                "Pulse bread flour, gluten, yeast, and salt; stream in warm water and oil until a soft dough forms (~12 minutes total mix/knead).",
                "Divide into 4 pieces (~129 g), shape balls, rest, then fold into 6–7 in rolls on cornmeal.",
                "Bake hot with a little steam (hot water pan) until deep golden. Cool before slicing for sandwiches.",
            ],
        },
        StableRecipe {
            name: "Protein Cheesy Bread",
            description: "Domino’s-style garlic cheesy bread rebuilt as a high-protein meal prep — Exercise4CheatMeals (store attribution; video has no printed gram list).",
            meal_type: MealType::Dinner,
            tag: "bakery",
            servings: 4,
            prep: 30,
            cook: 15,
            difficulty: Difficulty::Medium,
            menu_price: "4.50",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=RjO0JCSR7K0"),
            estimated_calories: Some(320),
            allergens: Some("gluten, dairy, egg"),
            ingredients: &[
                ("all-purpose flour", "200", "g", Some("0.40")),
                ("vital wheat gluten", "40", "g", Some("0.40")),
                ("yeast", "4", "g", Some("0.12")),
                ("salt", "5", "g", Some("0.01")),
                ("water", "160", "g", Some("0.00")),
                ("butter", "20", "g", Some("0.40")),
                ("garlic", "4", "clove", Some("0.32")),
                ("mozzarella", "120", "g", Some("1.70")),
                ("parmesan", "20", "g", Some("0.60")),
            ],
            steps: &[
                "Make a VWG-enriched pizza dough; rest until puffy. Press into a rectangle on parchment.",
                "Brush with garlic butter, cover with mozzarella and parmesan, bake on a hot steel until blistered.",
                "Cut into sticks for meal prep — pair with protein sauce or lean meat for a full meal.",
            ],
        },
        StableRecipe {
            name: "Buffalo Wings with Broth Fries",
            description: "Crispy wings and beef-broth soaked fries with buffalo sauce — Exercise4CheatMeals description recipe.",
            meal_type: MealType::Dinner,
            tag: "dinner",
            servings: 4,
            prep: 30,
            cook: 45,
            difficulty: Difficulty::Medium,
            menu_price: "11.00",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=IpePaktW6aQ"),
            estimated_calories: Some(520),
            allergens: None,
            ingredients: &[
                ("chicken wing", "12", "each", Some("6.60")),
                ("russet potato", "600", "g", Some("1.20")),
                ("beef bone broth", "400", "g", Some("1.20")),
                ("hot sauce", "60", "g", Some("0.40")),
                ("light butter", "15", "g", Some("0.18")),
                ("garlic powder", "1", "tsp", Some("0.06")),
                ("salt", "1", "tsp", Some("0.01")),
            ],
            steps: &[
                "Soak fry-cut potatoes in seasoned beef broth, then air-fry or oven-crisp.",
                "Cook wings until crispy; toss with hot sauce and melted light butter. Serve with broth fries.",
            ],
        },
        StableRecipe {
            name: "Lean Beef Burrito Meal Prep",
            description: "Seasoned lean beef thickened with broth and yogurt for high-protein burritos — Exercise4CheatMeals.",
            meal_type: MealType::Lunch,
            tag: "lunch",
            servings: 8,
            prep: 20,
            cook: 25,
            difficulty: Difficulty::Easy,
            menu_price: "5.00",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=cblWrsEgr2o"),
            estimated_calories: Some(259),
            allergens: None,
            ingredients: &[
                ("ground beef", "2", "lb", Some("10.00")),
                ("tortilla", "8", "each", Some("2.40")),
                ("beef bone broth", "300", "g", Some("0.90")),
                ("greek yogurt", "200", "g", Some("0.80")),
                ("tomato paste", "40", "g", Some("0.32")),
                ("gelatin", "10", "g", Some("0.30")),
                ("chili powder", "2", "tsp", Some("0.16")),
                ("cumin", "1", "tsp", Some("0.08")),
                ("garlic powder", "1", "tsp", Some("0.06")),
                ("salt", "1", "tsp", Some("0.01")),
            ],
            steps: &[
                "Brown beef with spices and flour. Bloom gelatin in broth; stir in with tomato paste and simmer until thick.",
                "Off heat, fold in Greek yogurt. Portion into tortillas; wrap for the week.",
            ],
        },
        StableRecipe {
            name: "Breakfast Burrito Freezer Pack",
            description: "Pork sausage seasoning, eggs, cheese, and tortillas for freezer breakfast burritos — Exercise4CheatMeals.",
            meal_type: MealType::Breakfast,
            tag: "breakfast",
            servings: 5,
            prep: 20,
            cook: 20,
            difficulty: Difficulty::Easy,
            menu_price: "4.50",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=y4-d6je7zS0"),
            estimated_calories: Some(461),
            allergens: None,
            ingredients: &[
                ("tortilla", "5", "each", Some("1.50")),
                ("cheddar cheese", "112", "g", Some("1.80")),
                ("ground pork", "224", "g", Some("1.80")),
                ("garlic", "5", "g", Some("0.40")),
                ("salt", "7", "g", Some("0.01")),
                ("black pepper", "2", "g", Some("0.10")),
                ("sage", "1", "tsp", Some("0.12")),
                ("thyme", "0.5", "tsp", Some("0.05")),
                ("apple cider vinegar", "1", "tsp", Some("0.06")),
                ("sugar-free maple syrup", "10", "g", Some("0.20")),
                ("eggs", "12", "each", Some("4.20")),
            ],
            steps: &[
                "Cook ground pork with garlic, spices, vinegar, and maple until crumbled and browned.",
                "Scramble eggs with salt and pepper. Fill tortillas with pork, eggs, and cheese; wrap and freeze.",
                "Reheat refrigerated wraps 60–90 seconds, or frozen wraps 3–4 minutes, in the microwave.",
            ],
        },
        StableRecipe {
            name: "One-Pot High-Protein Chicken Alfredo",
            description: "Store-kitchen Alfredo meal-prep inspired by Exercise4CheatMeals one-pot chicken Alfredo video.",
            meal_type: MealType::Dinner,
            tag: "dinner",
            servings: 6,
            prep: 15,
            cook: 25,
            difficulty: Difficulty::Easy,
            menu_price: "8.00",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=6Qk6UuwzIT8"),
            estimated_calories: Some(420),
            allergens: None,
            ingredients: &[
                ("chicken breast", "1.5", "lb", Some("5.25")),
                ("pasta", "12", "oz", Some("1.80")),
                ("chicken stock", "4", "cup", Some("2.00")),
                ("cream cheese", "6", "oz", Some("2.10")),
                ("parmesan", "2", "oz", Some("1.20")),
                ("garlic", "3", "clove", Some("0.24")),
                ("salt", "1", "tsp", Some("0.01")),
                ("black pepper", "0.5", "tsp", Some("0.03")),
            ],
            steps: &[
                "Sauté diced chicken with garlic until mostly cooked. Add stock and pasta; simmer until noodles are tender.",
                "Stir in cream cheese and parmesan until creamy. Season and portion for meal prep.",
            ],
        },
        StableRecipe {
            name: "Protein Breakfast Waffles",
            description: "Kitchen-board protein waffles attributed to Exercise4CheatMeals waffle video.",
            meal_type: MealType::Breakfast,
            tag: "breakfast",
            servings: 4,
            prep: 10,
            cook: 15,
            difficulty: Difficulty::Easy,
            menu_price: "5.00",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=bhg4t-1yX8k"),
            estimated_calories: Some(220),
            allergens: None,
            ingredients: &[
                ("whey protein powder", "60", "g", Some("3.00")),
                ("all-purpose flour", "0.75", "cup", Some("0.15")),
                ("eggs", "2", "each", Some("0.70")),
                ("greek yogurt", "150", "g", Some("0.60")),
                ("baking powder", "1", "tsp", Some("0.05")),
                ("vanilla extract", "1", "tsp", Some("0.25")),
                ("salt", "1", "pinch", Some("0.01")),
            ],
            steps: &[
                "Whisk dry ingredients. Mix eggs, yogurt, and vanilla; combine into a thick batter.",
                "Cook in a waffle iron until golden. Serve with berries or sugar-free syrup.",
            ],
        },
        StableRecipe {
            name: "Gyro-Style Chicken Meal Prep",
            description: "Gyro bowls attributed to Exercise4CheatMeals — pita, seasoned chicken, yogurt sauce.",
            meal_type: MealType::Lunch,
            tag: "lunch",
            servings: 6,
            prep: 20,
            cook: 25,
            difficulty: Difficulty::Easy,
            menu_price: "7.50",
            author: Some("Exercise4CheatMeals"),
            source_url: Some("https://www.youtube.com/watch?v=cr-Lbmyyv0k"),
            estimated_calories: Some(390),
            allergens: None,
            ingredients: &[
                ("chicken breast", "2", "lb", Some("7.00")),
                ("pita", "6", "each", Some("3.00")),
                ("greek yogurt", "300", "g", Some("1.20")),
                ("lemon", "1", "each", Some("0.60")),
                ("garlic", "3", "clove", Some("0.24")),
                ("cucumber", "1", "each", Some("0.90")),
                ("oregano", "1", "tsp", Some("0.08")),
                ("olive oil", "2", "tbsp", Some("0.40")),
                ("salt", "1", "tsp", Some("0.01")),
                ("black pepper", "0.5", "tsp", Some("0.03")),
            ],
            steps: &[
                "Toss chicken with oil, oregano, salt, and pepper; roast or grill until done; slice.",
                "Stir yogurt with grated cucumber, lemon, and garlic for sauce. Serve in pita or bowls.",
            ],
        },
    ];

    for spec in catalog {
        upsert_catalog_recipe(pool, &recipes, &masters, &tags, spec).await?;
    }
    Ok(())
}
