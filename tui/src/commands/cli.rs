use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate};
use clap::{Parser, Subcommand};
use larder_core::{
    db::init_db,
    models::{MealPlan, MealType},
    services::{
        ExportService, ImportService, MealPlanService, RecipeService, ShoppingListService,
        TagService,
    },
};
use std::io::{self, Write};
use std::time::Duration as StdDuration;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "larder")]
#[command(about = "Recipe manager")]
#[command(version)]
struct Cli {
    #[arg(long, default_value = "sqlite:larder.db")]
    database: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Tui,
    Serve,
    Search {
        query: String,
    },
    Show {
        id: String,
    },
    Import {
        url: String,
    },
    List,
    Random,
    Stats,
    Export {
        #[arg(short, long, default_value = "json")]
        format: String,
    },
    Backup {
        #[arg(short, long)]
        output: Option<String>,
    },
    Cook {
        id: String,
    },
    #[command(name = "meal-plan", alias = "mealplan")]
    MealPlan {
        #[arg(short, long)]
        generate: bool,
    },
    Shopping {
        #[arg(short, long)]
        generate: bool,
    },
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
}

#[derive(Subcommand)]
enum TagAction {
    List,
    Add { recipe: String, name: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let database_url = cli.database.clone();
    let pool = init_db(&database_url).await?;
    let recipes = RecipeService::new(pool.clone());
    let importer = ImportService::new();
    let meal_plans = MealPlanService::new(pool.clone());
    let shopping = ShoppingListService::new(pool.clone());
    let tags = TagService::new(pool.clone());

    match cli.command {
        Commands::Init => {
            println!("Database initialized successfully at {}", database_url);
        }
        Commands::Tui => {
            run_sibling_binary("larder-tui", &database_url)?;
        }
        Commands::Serve => {
            run_sibling_binary("larder-server", &database_url)?;
        }
        Commands::Search { query } => {
            let results = recipes.search_recipes(&query).await?;
            if results.is_empty() {
                println!("No recipes found for '{}'", query);
            } else {
                println!("Found {} recipe(s):\n", results.len());
                for r in &results {
                    let time = r
                        .total_time()
                        .map(|t| format!("{}m", t))
                        .unwrap_or("?".to_string());
                    println!("  {} ({})", r.name, time);
                }
            }
        }
        Commands::Show { id } => {
            let uuid = resolve_recipe_id(&recipes, &id).await?;

            let recipe = recipes
                .get_recipe(uuid)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Recipe not found"))?;

            let ingredients = recipes.get_ingredients(uuid).await?;
            let steps = recipes.get_steps(uuid).await?;
            let tags = recipes.get_tags(uuid).await?;

            println!("\n{}", "=".repeat(50));
            println!("{}", recipe.name);
            println!("{}", "=".repeat(50));

            if let Some(ref desc) = recipe.description {
                println!("\n{}", desc);
            }

            let mut meta = Vec::new();
            if let Some(t) = recipe.total_time() {
                meta.push(format!("Time: {} min", t));
            }
            meta.push(format!("Servings: {}", recipe.servings));
            if let Some(d) = &recipe.difficulty {
                meta.push(format!("Difficulty: {:?}", d));
            }
            if let Some(r) = recipe.rating {
                meta.push(format!("Rating: {}", "★".repeat(r as usize)));
            }
            println!("\n{}", meta.join(" | "));

            if !tags.is_empty() {
                println!(
                    "\nTags: {}",
                    tags.iter()
                        .map(|t| format!("#{}", t.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            if !ingredients.is_empty() {
                println!("\nIngredients:");
                for ing in &ingredients {
                    println!("  - {}", ing.display);
                }
            }

            if !steps.is_empty() {
                println!("\nSteps:");
                for (i, step) in steps.iter().enumerate() {
                    println!("  {}. {}", i + 1, step.instruction);
                    if let Some(timer) = step.timer_seconds {
                        println!("     [timer: {}:{:02}]", timer / 60, timer % 60);
                    }
                }
            }
            println!();
        }
        Commands::Import { url } => {
            println!("Importing from: {}", url);
            let imported = importer.import_from_url(&url).await?;
            let name = imported.recipe.name.clone();
            let servings = imported.recipe.servings;
            let ingredient_count = imported.ingredients.len();
            let step_count = imported.steps.len();

            let recipe_id = recipes.create_recipe(&imported.recipe).await?;
            for mut ingredient in imported.ingredients {
                ingredient.recipe_id = recipe_id;
                recipes.add_ingredient(&ingredient).await?;
            }
            for mut step in imported.steps {
                step.recipe_id = recipe_id;
                recipes.add_step(&step).await?;
            }

            println!("Imported: {} ({} servings)", name, servings);
            println!("  ID: {}", recipe_id);
            println!("  Ingredients: {}", ingredient_count);
            println!("  Steps: {}", step_count);
        }
        Commands::List => {
            let user_id = Uuid::nil();
            let all = recipes.list_recipes(user_id).await?;
            if all.is_empty() {
                println!(
                    "No recipes yet. Use 'larder import <url>' or press n in the TUI."
                );
            } else {
                println!("{} recipe(s):\n", all.len());
                for r in &all {
                    let time = r
                        .total_time()
                        .map(|t| format!("{}m", t))
                        .unwrap_or("?".to_string());
                    let rating = r.rating.map(|r| "★".repeat(r as usize)).unwrap_or_default();
                    println!("  {} ({}) {}", r.name, time, rating);
                }
            }
        }
        Commands::Random => {
            let user_id = Uuid::nil();
            let all = recipes.list_recipes(user_id).await?;
            if all.is_empty() {
                println!("No recipes yet.");
            } else {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let recipe = &all[rng.gen_range(0..all.len())];
                println!(
                    "Random recipe: {} ({})",
                    recipe.name,
                    recipe
                        .total_time()
                        .map(|t| format!("{}m", t))
                        .unwrap_or("?".to_string())
                );
            }
        }
        Commands::Stats => {
            let user_id = Uuid::nil();
            let all = recipes.list_recipes(user_id).await?;
            println!("Collection Statistics:");
            println!("  Total recipes: {}", all.len());

            let total_time: u32 = all.iter().filter_map(|r| r.total_time()).sum();
            println!(
                "  Total cook time: {} hours {} min",
                total_time / 60,
                total_time % 60
            );

            let avg_time = if !all.is_empty() {
                total_time / all.len() as u32
            } else {
                0
            };
            println!("  Average cook time: {} min", avg_time);

            let with_rating = all.iter().filter(|r| r.rating.is_some()).count();
            if with_rating > 0 {
                let avg_rating: f32 = all
                    .iter()
                    .filter_map(|r| r.rating)
                    .map(|r| r as f32)
                    .sum::<f32>()
                    / with_rating as f32;
                println!("  Average rating: {:.1}/5", avg_rating);
            }
        }
        Commands::Export { format } => {
            let user_id = Uuid::nil();
            let all = recipes.list_recipes(user_id).await?;

            let mut ingredients_map = Vec::new();
            let mut steps_map = Vec::new();

            for recipe in &all {
                let ings = recipes.get_ingredients(recipe.id).await?;
                let stps = recipes.get_steps(recipe.id).await?;
                ingredients_map.push((recipe.id, ings));
                steps_map.push((recipe.id, stps));
            }

            let output = match format.as_str() {
                "json" => ExportService::to_json(&all, &ingredients_map, &steps_map)?,
                "markdown" | "md" => {
                    ExportService::to_markdown(&all, &ingredients_map, &steps_map)?
                }
                _ => anyhow::bail!(
                    "Unknown export format: {}. Use 'json' or 'markdown'.",
                    format
                ),
            };

            println!("{}", output);
        }
        Commands::Backup { output } => {
            let output_path = output.unwrap_or_else(|| {
                format!(
                    "larder_backup_{}.db",
                    chrono::Local::now().format("%Y%m%d_%H%M%S")
                )
            });

            let db_path = sqlite_file_path(&database_url).ok_or_else(|| {
                anyhow::anyhow!("Backup only supports file-backed SQLite databases")
            })?;
            std::fs::copy(db_path, &output_path)?;
            println!("Backup saved to: {}", output_path);
        }
        Commands::Cook { id } => {
            let uuid = resolve_recipe_id(&recipes, &id).await?;
            let recipe = recipes
                .get_recipe(uuid)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Recipe not found"))?;
            let steps = recipes.get_steps(uuid).await?;
            if steps.is_empty() {
                anyhow::bail!("Recipe has no steps");
            }

            println!("\nCooking: {}\n", recipe.name);
            for (i, step) in steps.iter().enumerate() {
                println!("--- Step {} of {} ---", i + 1, steps.len());
                println!("{}\n", step.instruction);

                if let Some(timer) = step.timer_seconds {
                    print!(
                        "Timer {}:{:02} — Enter to start, 's' to skip: ",
                        timer / 60,
                        timer % 60
                    );
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("s") {
                        run_countdown(timer);
                    }
                }

                if i + 1 < steps.len() {
                    print!("Press Enter for next step...");
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                }
            }
            println!("\nDone.");
        }
        Commands::MealPlan { generate } => {
            let user_id = Uuid::nil();
            let today = chrono::Local::now().date_naive();
            let week_start =
                today - Duration::days(today.weekday().num_days_from_monday() as i64);
            let meals = meal_plans.get_week(user_id, week_start).await?;
            print_meal_plan(&recipes, week_start, &meals).await?;

            if generate {
                let count = shopping.generate_from_meal_plan(user_id, week_start).await?;
                println!("\nAdded {} item(s) to shopping list", count);
            }
        }
        Commands::Shopping { generate } => {
            let user_id = Uuid::nil();
            if generate {
                let today = chrono::Local::now().date_naive();
                let week_start =
                    today - Duration::days(today.weekday().num_days_from_monday() as i64);
                let count = shopping.generate_from_meal_plan(user_id, week_start).await?;
                println!("Added {} item(s) from meal plan", count);
            }

            let items = shopping.get_list(user_id).await?;
            if items.is_empty() {
                println!("Shopping list is empty.");
            } else {
                println!("Shopping list:\n");
                let mut current_cat = String::new();
                for item in items {
                    let cat = item.category.clone().unwrap_or_else(|| "Other".to_string());
                    if cat != current_cat {
                        println!("── {} ──", cat);
                        current_cat = cat;
                    }
                    let mark = if item.checked { "[x]" } else { "[ ]" };
                    let label = match (&item.quantity, &item.unit) {
                        (Some(q), Some(u)) => format!("{} {} {}", q, u, item.item),
                        (Some(q), None) => format!("{} {}", q, item.item),
                        _ => item.item.clone(),
                    };
                    println!("  {} {}", mark, label);
                }
            }
        }
        Commands::Tag { action } => match action {
            TagAction::List => {
                let all = tags.list_all().await?;
                if all.is_empty() {
                    println!("No tags yet.");
                } else {
                    for tag in all {
                        println!("  #{}", tag.name);
                    }
                }
            }
            TagAction::Add { recipe, name } => {
                let uuid = resolve_recipe_id(&recipes, &recipe).await?;
                let tag = tags.add_to_recipe(uuid, &name).await?;
                println!("Tagged recipe with #{}", tag.name);
            }
        },
    }

    Ok(())
}

fn run_sibling_binary(name: &str, database_url: &str) -> Result<()> {
    use std::path::PathBuf;
    use std::process::Stdio;

    let program = {
        let current = std::env::current_exe()?;
        let sibling = current.with_file_name(name);
        if sibling.exists() {
            sibling
        } else {
            PathBuf::from(name)
        }
    };

    let status = std::process::Command::new(&program)
        .env("DATABASE_URL", database_url)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to launch {} (tried {}): {}\nInstall with: cargo install --path tui --bin larder --bin larder-tui && cargo install --path server --bin larder-server",
                name,
                program.display(),
                e
            )
        })?;

    if !status.success() {
        anyhow::bail!("{} exited with {}", name, status);
    }

    Ok(())
}

fn sqlite_file_path(database_url: &str) -> Option<String> {
    database_url
        .strip_prefix("sqlite:")
        .filter(|path| !path.is_empty() && *path != ":memory:")
        .map(|path| path.to_string())
}

async fn resolve_recipe_id(recipes: &RecipeService, id: &str) -> Result<Uuid> {
    if let Ok(uuid) = Uuid::parse_str(id) {
        return Ok(uuid);
    }
    let results = recipes.search_recipes(id).await?;
    results
        .first()
        .map(|r| r.id)
        .ok_or_else(|| anyhow::anyhow!("Recipe '{}' not found", id))
}

fn run_countdown(seconds: u32) {
    for remaining in (0..=seconds).rev() {
        print!(
            "\r  ⏱ {:02}:{:02}",
            remaining / 60,
            remaining % 60
        );
        io::stdout().flush().ok();
        if remaining > 0 {
            std::thread::sleep(StdDuration::from_secs(1));
        }
    }
    println!("\n  Timer done!");
}

async fn print_meal_plan(
    recipes: &RecipeService,
    week_start: NaiveDate,
    meals: &[MealPlan],
) -> Result<()> {
    let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let slots = [
        MealType::Breakfast,
        MealType::Lunch,
        MealType::Dinner,
        MealType::Snack,
    ];

    println!(
        "Meal plan: {} — {}\n",
        week_start.format("%b %d"),
        (week_start + Duration::days(6)).format("%b %d, %Y")
    );

    for day in 0..7 {
        let date = week_start + Duration::days(day as i64);
        println!("{} {} ({})", day_names[day], date.day(), date.format("%Y-%m-%d"));
        for meal_type in slots {
            let entry = meals
                .iter()
                .find(|m| m.date == date && m.meal_type == meal_type);
            let label = match entry {
                Some(m) if m.recipe_id.is_some() => {
                    let id = m.recipe_id.unwrap();
                    recipes
                        .get_recipe(id)
                        .await?
                        .map(|r| r.name)
                        .unwrap_or_else(|| "(recipe)".to_string())
                }
                Some(m) => m.title.clone().unwrap_or_else(|| "(empty)".to_string()),
                None => "—".to_string(),
            };
            println!("  {:>10}: {}", meal_type, label);
        }
        println!();
    }
    Ok(())
}
