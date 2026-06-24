use anyhow::Result;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::DEFAULT_USER_ID;
use crate::models::{Difficulty, MealType, Recipe, RecipeIngredient, RecipeStep, Tag};
use crate::services::{MealPlanService, RecipeService};

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
    let today = Utc::now().date_naive();

    let samples: [(&str, &str, MealType, u32, u32, u32, Difficulty, u8, &[(&str, &str, &str)], &[&str]); 4] = [
        (
            "Scrambled Eggs",
            "Quick morning eggs with butter and salt.",
            MealType::Breakfast,
            2,
            2,
            5,
            Difficulty::Easy,
            4,
            &[
                ("eggs", "3", "large"),
                ("butter", "1", "tbsp"),
                ("salt", "1", "pinch"),
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
            &[
                ("bread", "2", "slices"),
                ("turkey", "4", "oz"),
                ("mustard", "1", "tbsp"),
                ("lettuce", "2", "leaves"),
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
            &[
                ("pasta", "200", "g"),
                ("olive oil", "2", "tbsp"),
                ("garlic", "3", "cloves"),
                ("parmesan", "0.25", "cup"),
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
            &[
                ("apple", "1", "medium"),
                ("peanut butter", "2", "tbsp"),
            ],
            &[
                "Core the apple and slice into wedges.",
                "Serve with peanut butter on the side for dipping.",
            ],
        ),
    ];

    for (name, description, meal_type, servings, prep, cook, difficulty, rating, ingredients, steps) in
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
        };

        let recipe_id = recipes.create_recipe(&recipe).await?;

        for (ingredient, qty, unit) in ingredients.iter() {
            let display = format!("{} {} {}", qty, unit, ingredient);
            recipes
                .add_ingredient(&RecipeIngredient {
                    id: Uuid::new_v4(),
                    recipe_id,
                    ingredient: ingredient.to_string(),
                    quantity: qty.parse::<Decimal>().ok(),
                    unit: Some(unit.to_string()),
                    note: None,
                    display,
                    category: None,
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
