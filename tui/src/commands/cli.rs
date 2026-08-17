use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate};
use clap::{Parser, Subcommand};
use larder_core::{
    db::init_db,
    models::{MealPlan, MealType},
    services::{
        scaling::{combined_scale_factor, format_quantity, scale_display_by_factor},
        BundleService, ExportService, ImportService, MealPlanService, RecipeService,
        ShoppingListService,         TagService,
    },
};
use larder_core::services::IngredientMasterService;
use rust_decimal::Decimal;
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
        #[arg(value_name = "URL")]
        url: Option<String>,
        #[arg(short, long, value_name = "FILE")]
        file: Option<String>,
    },
    List,
    Random,
    Stats,
    Export {
        #[arg(short, long, default_value = "json")]
        format: String,
        /// Only recipes with this tag (e.g. work, coop, bakery)
        #[arg(long)]
        tag: Option<String>,
        /// Only recipes in this cookbook (id or name)
        #[arg(long)]
        cookbook: Option<String>,
        /// Write to file instead of stdout
        #[arg(short, long)]
        output: Option<String>,
    },
    Backup {
        #[arg(short, long)]
        output: Option<String>,
    },
    Cook {
        id: String,
    },
    Scale {
        id: String,
        /// Target number of servings
        #[arg(conflicts_with = "factor", required_unless_present = "factor")]
        servings: Option<u32>,
        /// Multiply quantities by this factor instead of scaling by servings
        #[arg(short, long)]
        factor: Option<Decimal>,
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
    Remove { recipe: String, name: String },
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

            let ingredients = recipes.get_ingredients(uuid, None).await?;
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
        Commands::Import { url, file } => {
            if let Some(path) = file {
                let json = std::fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path))?;
                let bundle = BundleService::parse(&json)?;
                let result = BundleService::import(
                    &recipes,
                    &IngredientMasterService::new(pool.clone()),
                    &tags,
                    &larder_core::services::LocationService::new(pool.clone()),
                    &bundle,
                    Uuid::nil(),
                )
                .await?;
                println!(
                    "Imported {} recipe(s), {} ingredient(s), {} location price(s)",
                    result.recipes_imported,
                    result.ingredients_upserted,
                    result.location_prices_set
                );
            } else if let Some(url) = url {
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
            } else {
                anyhow::bail!("import requires a URL or --file");
            }
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
        Commands::Export {
            format,
            tag,
            cookbook,
            output,
        } => {
            let user_id = Uuid::nil();
            let locations = larder_core::services::LocationService::new(pool.clone());
            let masters = IngredientMasterService::new(pool.clone());
            let cookbooks = larder_core::services::CookbookService::new(pool.clone());

            let filter_ids =
                resolve_export_recipe_ids(&recipes, &cookbooks, tag.as_deref(), cookbook.as_deref())
                    .await?;
            // Prefer tag path when only --tag; use id allow-list when --cookbook (or both).
            let (tag_for_bundle, id_allow) = if cookbook.is_some() {
                (None, filter_ids.as_deref())
            } else if tag.is_some() {
                (tag.as_deref(), None)
            } else {
                (None, None)
            };

            let output_text = match format.as_str() {
                "json" => {
                    let bundle = if tag.is_some() || cookbook.is_some() {
                        ExportService::export_bundle_filtered(
                            &recipes,
                            &masters,
                            &locations,
                            user_id,
                            tag_for_bundle,
                            id_allow,
                        )
                        .await?
                    } else {
                        ExportService::export_bundle(&recipes, &masters, &locations, user_id)
                            .await?
                    };
                    eprintln!("Exported {} recipe(s)", bundle.recipes.len());
                    ExportService::bundle_to_json(&bundle)?
                }
                "simple" => {
                    let all = filter_recipe_list(
                        recipes.list_recipes(user_id).await?,
                        filter_ids.as_ref(),
                    );
                    let mut ingredients_map = Vec::new();
                    let mut steps_map = Vec::new();
                    for recipe in &all {
                        ingredients_map.push((
                            recipe.id,
                            recipes.get_ingredients(recipe.id, None).await?,
                        ));
                        steps_map.push((recipe.id, recipes.get_steps(recipe.id).await?));
                    }
                    ExportService::to_json(&all, &ingredients_map, &steps_map)?
                }
                "markdown" | "md" => {
                    let all = filter_recipe_list(
                        recipes.list_recipes(user_id).await?,
                        filter_ids.as_ref(),
                    );
                    let mut ingredients_map = Vec::new();
                    let mut steps_map = Vec::new();
                    let mut tags_map = Vec::new();
                    for recipe in &all {
                        ingredients_map.push((
                            recipe.id,
                            recipes.get_ingredients(recipe.id, None).await?,
                        ));
                        steps_map.push((recipe.id, recipes.get_steps(recipe.id).await?));
                        let t = recipes.get_tags(recipe.id).await?;
                        tags_map.push((
                            recipe.id,
                            t.into_iter().map(|x| x.name).collect(),
                        ));
                    }
                    ExportService::to_markdown(&all, &ingredients_map, &steps_map, &tags_map)?
                }
                "csv-recipes" | "csv" => {
                    let bundle = ExportService::export_bundle_filtered(
                        &recipes,
                        &masters,
                        &locations,
                        user_id,
                        tag_for_bundle,
                        id_allow,
                    )
                    .await?;
                    BundleService::to_csv_recipes(&bundle)
                }
                "csv-ingredients" => {
                    let bundle = ExportService::export_bundle_filtered(
                        &recipes,
                        &masters,
                        &locations,
                        user_id,
                        tag_for_bundle,
                        id_allow,
                    )
                    .await?;
                    BundleService::to_csv_ingredients(&bundle)
                }
                _ => anyhow::bail!(
                    "Unknown export format: {}. Use json, simple, markdown, csv-recipes, or csv-ingredients.",
                    format
                ),
            };

            if let Some(path) = output {
                std::fs::write(&path, &output_text)
                    .with_context(|| format!("write {}", path))?;
                eprintln!("Wrote {}", path);
            } else {
                println!("{}", output_text);
            }
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
        Commands::Scale { id, servings, factor } => {
            let uuid = resolve_recipe_id(&recipes, &id).await?;
            let recipe = recipes
                .get_recipe(uuid)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Recipe not found"))?;
            let ingredients = recipes.get_ingredients(uuid, None).await?;

            let (scale, label) = match (servings, factor) {
                (Some(target), None) => (
                    combined_scale_factor(recipe.servings, target, Decimal::ONE),
                    format!("{} servings (base {})", target, recipe.servings),
                ),
                (None, Some(f)) => (
                    combined_scale_factor(recipe.servings, recipe.servings, f),
                    format!("×{} (base {} servings)", format_quantity(&f), recipe.servings),
                ),
                _ => unreachable!("clap enforces exactly one of servings/--factor"),
            };

            println!("\n{}", "=".repeat(50));
            println!("{}", recipe.name);
            println!("{}", "=".repeat(50));
            println!("\nScaled: {}", label);

            if ingredients.is_empty() {
                println!("\nNo ingredients.");
            } else {
                println!("\nIngredients:");
                for ing in &ingredients {
                    println!("  - {}", scale_display_by_factor(&ing.display, scale));
                }
            }
            println!();
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
            TagAction::Remove { recipe, name } => {
                let uuid = resolve_recipe_id(&recipes, &recipe).await?;
                let recipe_tags = recipes.get_tags(uuid).await?;
                let needle = name.trim().to_lowercase();
                let Some(tag) = recipe_tags
                    .into_iter()
                    .find(|t| t.name.to_lowercase() == needle)
                else {
                    anyhow::bail!("Tag #{} not on that recipe", name.trim());
                };
                tags.remove_from_recipe(uuid, tag.id).await?;
                println!("Removed #{} from recipe", tag.name);
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

async fn resolve_cookbook_id(
    cookbooks: &larder_core::services::CookbookService,
    key: &str,
) -> Result<Uuid> {
    if let Ok(uuid) = Uuid::parse_str(key) {
        return Ok(uuid);
    }
    let needle = key.trim().to_lowercase();
    let books = cookbooks.list_cookbooks(Uuid::nil()).await?;
    books
        .into_iter()
        .find(|b| b.name.to_lowercase() == needle)
        .map(|b| b.id)
        .ok_or_else(|| anyhow::anyhow!("Cookbook '{}' not found", key))
}

/// Resolve recipe ids for `--tag` / `--cookbook` export filters.
async fn resolve_export_recipe_ids(
    recipes: &RecipeService,
    cookbooks: &larder_core::services::CookbookService,
    tag: Option<&str>,
    cookbook: Option<&str>,
) -> Result<Option<Vec<Uuid>>> {
    if tag.is_none() && cookbook.is_none() {
        return Ok(None);
    }
    let mut ids: Option<std::collections::HashSet<Uuid>> = None;
    if let Some(t) = tag {
        let tagged = recipes.list_recipes_by_tag(t.trim()).await?;
        ids = Some(tagged.into_iter().map(|r| r.id).collect());
    }
    if let Some(c) = cookbook {
        let cid = resolve_cookbook_id(cookbooks, c).await?;
        let entries = cookbooks.get_recipes(cid).await?;
        let set: std::collections::HashSet<Uuid> =
            entries.into_iter().map(|e| e.recipe_id).collect();
        ids = Some(match ids {
            Some(prev) => prev.intersection(&set).copied().collect(),
            None => set,
        });
    }
    Ok(ids.map(|s| s.into_iter().collect()))
}

fn filter_recipe_list(
    all: Vec<larder_core::models::Recipe>,
    allow: Option<&Vec<Uuid>>,
) -> Vec<larder_core::models::Recipe> {
    match allow {
        Some(ids) => {
            let set: std::collections::HashSet<Uuid> = ids.iter().copied().collect();
            all.into_iter().filter(|r| set.contains(&r.id)).collect()
        }
        None => all,
    }
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
