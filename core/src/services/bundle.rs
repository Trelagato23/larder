use anyhow::{Context, Result};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{
    Difficulty, Recipe, RecipeIngredient, RecipeStep, name_key,
};
use crate::services::{
    import::ImportService, IngredientMasterService, LocationService, RecipeService, TagService,
};

pub const BUNDLE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LarderBundle {
    pub larder_version: u32,
    pub exported_at: String,
    #[serde(default)]
    pub ingredients: Vec<BundleIngredient>,
    #[serde(default)]
    pub location_prices: Vec<BundleLocationPrice>,
    pub recipes: Vec<BundleRecipe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleIngredient {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_per_unit: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_size: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleLocationPrice {
    pub location_slug: String,
    pub ingredient_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_per_unit: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRecipe {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default = "default_servings")]
    pub servings: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prep_time_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cook_time_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_time_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_price: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yield_quantity: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yield_unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waste_percent: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_calories: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allergens: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub ingredients: Vec<BundleRecipeIngredient>,
    #[serde(default)]
    pub steps: Vec<BundleStep>,
}

fn default_servings() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRecipeIngredient {
    pub ingredient: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub display: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_per_unit: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_cost: Option<Decimal>,
    #[serde(default, alias = "master_ingredient", skip_serializing_if = "Option::is_none")]
    pub master_ingredient_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prep_yield_percent: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleStep {
    pub position: u32,
    pub instruction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_seconds: Option<u32>,
}

/// Legacy export: array of recipes with string ingredient/step lines.
#[derive(Debug, Deserialize)]
struct LegacyRecipe {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    servings: Option<u32>,
    #[serde(default)]
    prep_time_minutes: Option<u32>,
    #[serde(default)]
    cook_time_minutes: Option<u32>,
    #[serde(default)]
    total_time_minutes: Option<u32>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    yield_quantity: Option<serde_json::Value>,
    #[serde(default)]
    yield_unit: Option<String>,
    #[serde(default)]
    ingredients: Vec<serde_json::Value>,
    #[serde(default)]
    steps: Vec<serde_json::Value>,
}

#[derive(Debug, Default)]
pub struct ImportBundleResult {
    pub recipes_imported: usize,
    pub ingredients_upserted: usize,
    pub location_prices_set: usize,
}

pub struct BundleService;

impl BundleService {
    pub async fn export(
        recipes: &RecipeService,
        ingredients: &IngredientMasterService,
        locations: &LocationService,
        user_id: Uuid,
    ) -> Result<LarderBundle> {
        Self::export_filtered(recipes, ingredients, locations, user_id, None, None).await
    }

    /// Export a subset for work deploy: filter by tag and/or cookbook.
    /// When both are set, recipes must match the tag *and* be in the cookbook.
    pub async fn export_filtered(
        recipes: &RecipeService,
        ingredients: &IngredientMasterService,
        locations: &LocationService,
        user_id: Uuid,
        tag: Option<&str>,
        cookbook_ids: Option<&[Uuid]>,
    ) -> Result<LarderBundle> {
        let masters = ingredients.list(None).await?;
        let location_prices_all: Vec<BundleLocationPrice> = locations
            .list_prices_for_export()
            .await?
            .into_iter()
            .map(|row| BundleLocationPrice {
                location_slug: row.location_slug,
                ingredient_name: row.ingredient_name,
                cost_per_unit: row.cost_per_unit,
            })
            .collect();

        let mut all = if let Some(t) = tag.filter(|s| !s.trim().is_empty()) {
            recipes.list_recipes_by_tag(t.trim()).await?
        } else {
            recipes.list_recipes(user_id).await?
        };

        if let Some(ids) = cookbook_ids {
            if !ids.is_empty() {
                let allow: std::collections::HashSet<Uuid> = ids.iter().copied().collect();
                all.retain(|r| allow.contains(&r.id));
            }
        }

        let mut bundle_recipes = Vec::new();
        let mut used_masters: std::collections::HashSet<String> = std::collections::HashSet::new();

        for recipe in &all {
            let ings = recipes.get_ingredients(recipe.id, None).await?;
            let steps = recipes.get_steps(recipe.id).await?;
            let tags = recipes.get_tags(recipe.id).await?;

            for i in &ings {
                if let Some(ref m) = i.master_name {
                    used_masters.insert(name_key(m));
                }
                used_masters.insert(name_key(&i.ingredient));
            }

            bundle_recipes.push(BundleRecipe {
                name: recipe.name.clone(),
                description: recipe.description.clone(),
                image_url: recipe.image_url.clone(),
                servings: recipe.servings,
                prep_time_minutes: recipe.prep_time_minutes,
                cook_time_minutes: recipe.cook_time_minutes,
                total_time_minutes: recipe.total_time_minutes,
                source_url: recipe.source_url.clone(),
                rating: recipe.rating,
                difficulty: recipe.difficulty.map(|d| match d {
                    Difficulty::Easy => "easy".into(),
                    Difficulty::Medium => "medium".into(),
                    Difficulty::Hard => "hard".into(),
                }),
                menu_price: recipe.menu_price,
                yield_quantity: recipe.yield_quantity,
                yield_unit: recipe.yield_unit.clone(),
                waste_percent: recipe.waste_percent,
                author: recipe.author.clone(),
                estimated_calories: recipe.estimated_calories,
                allergens: recipe.allergens.clone(),
                tags: tags.into_iter().map(|t| t.name).collect(),
                ingredients: ings
                    .iter()
                    .map(|i| BundleRecipeIngredient {
                        ingredient: i.ingredient.clone(),
                        quantity: i.quantity,
                        unit: i.unit.clone(),
                        note: i.note.clone(),
                        display: i.display.clone(),
                        category: i.category.clone(),
                        cost_per_unit: i.cost_per_unit,
                        line_cost: i.line_cost,
                        master_ingredient_name: i.master_name.clone(),
                        prep_yield_percent: i.prep_yield_percent,
                    })
                    .collect(),
                steps: steps
                    .iter()
                    .map(|s| BundleStep {
                        position: s.position,
                        instruction: s.instruction.clone(),
                        timer_seconds: s.timer_seconds,
                    })
                    .collect(),
            });
        }

        let filtered = tag.is_some() || cookbook_ids.map(|c| !c.is_empty()).unwrap_or(false);
        let bundle_ingredients: Vec<BundleIngredient> = masters
            .iter()
            .filter(|m| !filtered || used_masters.contains(&name_key(&m.name)))
            .map(|m| BundleIngredient {
                name: m.name.clone(),
                default_unit: m.default_unit.clone(),
                cost_per_unit: m.cost_per_unit,
                pack_size: m.pack_size,
                pack_unit: m.pack_unit.clone(),
                notes: m.notes.clone(),
            })
            .collect();

        let location_prices: Vec<BundleLocationPrice> = if filtered {
            location_prices_all
                .into_iter()
                .filter(|p| used_masters.contains(&name_key(&p.ingredient_name)))
                .collect()
        } else {
            location_prices_all
        };

        Ok(LarderBundle {
            larder_version: BUNDLE_VERSION,
            exported_at: Utc::now().to_rfc3339(),
            ingredients: bundle_ingredients,
            location_prices,
            recipes: bundle_recipes,
        })
    }

    pub fn to_json(bundle: &LarderBundle) -> Result<String> {
        Ok(serde_json::to_string_pretty(bundle)?)
    }

    pub fn parse(json: &str) -> Result<LarderBundle> {
        let trimmed = json.trim();
        if trimmed.starts_with('[') {
            let legacy: Vec<LegacyRecipe> =
                serde_json::from_str(trimmed).context("parse legacy recipe array")?;
            return Ok(LarderBundle {
                larder_version: 0,
                exported_at: String::new(),
                ingredients: vec![],
                location_prices: vec![],
                recipes: legacy.into_iter().map(legacy_to_bundle).collect(),
            });
        }
        serde_json::from_str(trimmed).context("parse larder bundle JSON")
    }

    pub async fn import(
        recipes: &RecipeService,
        ingredients: &IngredientMasterService,
        tags: &TagService,
        locations: &LocationService,
        bundle: &LarderBundle,
        user_id: Uuid,
    ) -> Result<ImportBundleResult> {
        let mut result = ImportBundleResult::default();
        let mut master_by_key: std::collections::HashMap<String, Uuid> =
            std::collections::HashMap::new();

        for ing in &bundle.ingredients {
            let master = ingredients
                .find_or_create(
                    &ing.name,
                    ing.default_unit.as_deref(),
                    ing.cost_per_unit,
                )
                .await?;
            if ing.cost_per_unit.is_some()
                || ing.pack_size.is_some()
                || ing.notes.is_some()
                || ing.default_unit.is_some()
            {
                ingredients
                    .update(
                        master.id,
                        Some(&ing.name),
                        Some(ing.default_unit.as_deref()),
                        Some(ing.cost_per_unit),
                        Some(ing.pack_size),
                        Some(ing.pack_unit.as_deref()),
                        Some(ing.notes.as_deref()),
                        None,
                    )
                    .await?;
            }
            master_by_key.insert(name_key(&ing.name), master.id);
            result.ingredients_upserted += 1;
        }

        for lp in &bundle.location_prices {
            let loc = locations
                .get_by_slug(&lp.location_slug)
                .await?
                .with_context(|| format!("unknown location slug: {}", lp.location_slug))?;
            let key = name_key(&lp.ingredient_name);
            let ing_id = if let Some(id) = master_by_key.get(&key) {
                *id
            } else if let Some(m) = ingredients.get_by_name_key(&key).await? {
                m.id
            } else {
                let m = ingredients
                    .find_or_create(&lp.ingredient_name, None, lp.cost_per_unit)
                    .await?;
                master_by_key.insert(key, m.id);
                result.ingredients_upserted += 1;
                m.id
            };
            locations
                .set_ingredient_price(loc.id, ing_id, lp.cost_per_unit)
                .await?;
            result.location_prices_set += 1;
        }

        for br in &bundle.recipes {
            let difficulty = br.difficulty.as_deref().and_then(parse_difficulty);
            let recipe = Recipe {
                id: Uuid::new_v4(),
                name: br.name.clone(),
                description: br.description.clone(),
                image_url: br.image_url.clone(),
                servings: br.servings.max(1),
                prep_time_minutes: br.prep_time_minutes,
                cook_time_minutes: br.cook_time_minutes,
                total_time_minutes: br
                    .total_time_minutes
                    .or_else(|| match (br.prep_time_minutes, br.cook_time_minutes) {
                        (Some(p), Some(c)) => Some(p + c),
                        (Some(p), None) => Some(p),
                        (None, Some(c)) => Some(c),
                        _ => None,
                    }),
                source_url: br.source_url.clone(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                user_id,
                rating: br.rating,
                difficulty,
                menu_price: br.menu_price,
                yield_quantity: br.yield_quantity,
                yield_unit: br.yield_unit.clone(),
                waste_percent: br.waste_percent,
                author: br.author.clone(),
                estimated_calories: br.estimated_calories,
                allergens: crate::models::normalize_allergens(br.allergens.as_deref()),
            last_opened_at: None,
            open_count: None,
            };

            let recipe_id = recipes.create_recipe(&recipe).await?;

            for bi in &br.ingredients {
                let master_id = if let Some(ref name) = bi.master_ingredient_name {
                    let key = name_key(name);
                    if let Some(id) = master_by_key.get(&key) {
                        Some(*id)
                    } else if let Some(m) = ingredients.get_by_name_key(&key).await? {
                        master_by_key.insert(key, m.id);
                        Some(m.id)
                    } else {
                        let m = ingredients
                            .find_or_create(name, bi.unit.as_deref(), bi.cost_per_unit)
                            .await?;
                        master_by_key.insert(key, m.id);
                        result.ingredients_upserted += 1;
                        Some(m.id)
                    }
                } else {
                    None
                };

                let line = RecipeIngredient {
                    id: Uuid::new_v4(),
                    recipe_id,
                    ingredient: bi.ingredient.clone(),
                    quantity: bi.quantity,
                    unit: bi.unit.clone(),
                    note: bi.note.clone(),
                    display: if bi.display.is_empty() {
                        bi.ingredient.clone()
                    } else {
                        bi.display.clone()
                    },
                    category: bi.category.clone(),
                    cost_per_unit: bi.cost_per_unit,
                    line_cost: bi.line_cost,
                    master_ingredient_id: master_id,
                    prep_yield_percent: bi.prep_yield_percent,
                    master_name: bi.master_ingredient_name.clone(),
                    master_g_per_cup: None,
                };
                recipes.add_ingredient(&line).await?;
            }

            for bs in &br.steps {
                recipes
                    .add_step(&RecipeStep {
                        id: Uuid::new_v4(),
                        recipe_id,
                        position: bs.position,
                        instruction: bs.instruction.clone(),
                        timer_seconds: bs.timer_seconds,
                    })
                    .await?;
            }

            for tag_name in &br.tags {
                let tag = tags.add_to_recipe(recipe_id, tag_name).await?;
                let _ = tag;
            }

            result.recipes_imported += 1;
        }

        Ok(result)
    }

    pub fn to_csv_recipes(bundle: &LarderBundle) -> String {
        let mut out = String::from(
            "recipe_name,servings,tag,ingredient,quantity,unit,display,category,cost_per_unit,line_cost,master_ingredient,prep_yield_percent,step_position,step_instruction,timer_seconds\n",
        );
        for recipe in &bundle.recipes {
            let tags = if recipe.tags.is_empty() {
                String::new()
            } else {
                recipe.tags.join(";")
            };
            if recipe.ingredients.is_empty() && recipe.steps.is_empty() {
                csv_row(
                    &mut out,
                    &[
                        &recipe.name,
                        &recipe.servings.to_string(),
                        &tags,
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                    ],
                );
                continue;
            }
            let max = recipe.ingredients.len().max(recipe.steps.len()).max(1);
            for i in 0..max {
                let ing = recipe.ingredients.get(i);
                let step = recipe.steps.get(i);
                csv_row(
                    &mut out,
                    &[
                        &recipe.name,
                        &recipe.servings.to_string(),
                        &tags,
                        ing.map(|x| x.ingredient.as_str()).unwrap_or(""),
                        ing.and_then(|x| x.quantity.map(|q| q.to_string()))
                            .unwrap_or_default()
                            .as_str(),
                        ing.and_then(|x| x.unit.as_deref()).unwrap_or(""),
                        ing.map(|x| x.display.as_str()).unwrap_or(""),
                        ing.and_then(|x| x.category.as_deref()).unwrap_or(""),
                        ing.and_then(|x| x.cost_per_unit.map(|q| q.to_string()))
                            .unwrap_or_default()
                            .as_str(),
                        ing.and_then(|x| x.line_cost.map(|q| q.to_string()))
                            .unwrap_or_default()
                            .as_str(),
                        ing.and_then(|x| x.master_ingredient_name.as_deref())
                            .unwrap_or(""),
                        ing.and_then(|x| x.prep_yield_percent.map(|q| q.to_string()))
                            .unwrap_or_default()
                            .as_str(),
                        &step.map(|s| s.position.to_string()).unwrap_or_default(),
                        step.map(|s| s.instruction.as_str()).unwrap_or(""),
                        &step
                            .and_then(|s| s.timer_seconds.map(|t| t.to_string()))
                            .unwrap_or_default(),
                    ],
                );
            }
        }
        out
    }

    pub fn to_csv_ingredients(bundle: &LarderBundle) -> String {
        let mut out = String::from(
            "name,default_unit,cost_per_unit,pack_size,pack_unit,notes,location_slug,location_cost\n",
        );
        for ing in &bundle.ingredients {
            csv_row(
                &mut out,
                &[
                    &ing.name,
                    ing.default_unit.as_deref().unwrap_or(""),
                    &ing
                        .cost_per_unit
                        .map(|q| q.to_string())
                        .unwrap_or_default(),
                    &ing.pack_size.map(|q| q.to_string()).unwrap_or_default(),
                    ing.pack_unit.as_deref().unwrap_or(""),
                    ing.notes.as_deref().unwrap_or(""),
                    "",
                    "",
                ],
            );
        }
        for lp in &bundle.location_prices {
            csv_row(
                &mut out,
                &[
                    &lp.ingredient_name,
                    "",
                    "",
                    "",
                    "",
                    "",
                    &lp.location_slug,
                    &lp
                        .cost_per_unit
                        .map(|q| q.to_string())
                        .unwrap_or_default(),
                ],
            );
        }
        out
    }
}

fn json_to_decimal(v: &Option<serde_json::Value>) -> Option<Decimal> {
    match v {
        Some(serde_json::Value::Number(n)) => n.to_string().parse().ok(),
        Some(serde_json::Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                t.parse().ok()
            }
        }
        _ => None,
    }
}

fn ingredient_from_display_line(text: String) -> Option<BundleRecipeIngredient> {
    if text.trim().is_empty() {
        return None;
    }
    let (quantity, unit, ingredient, note) = ImportService::parse_ingredient_line(&text);
    Some(BundleRecipeIngredient {
        ingredient: if ingredient.is_empty() {
            text.clone()
        } else {
            ingredient
        },
        quantity,
        unit,
        note,
        display: text,
        category: None,
        cost_per_unit: None,
        line_cost: None,
        master_ingredient_name: None,
        prep_yield_percent: None,
    })
}

fn legacy_to_bundle(legacy: LegacyRecipe) -> BundleRecipe {
    let ingredients = legacy
        .ingredients
        .iter()
        .filter_map(|v| {
            match v {
                serde_json::Value::String(s) => ingredient_from_display_line(s.clone()),
                serde_json::Value::Object(obj) => {
                    let mut display = obj
                        .get("display")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mut ingredient = obj
                        .get("ingredient")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mut quantity = obj
                        .get("quantity")
                        .and_then(|x| x.as_str())
                        .and_then(|s| s.parse().ok())
                        .or_else(|| {
                            obj.get("quantity")
                                .and_then(|x| x.as_f64())
                                .and_then(Decimal::from_f64_retain)
                        });
                    let mut unit = obj
                        .get("unit")
                        .and_then(|x| x.as_str())
                        .map(String::from);
                    let mut note = obj.get("note").and_then(|x| x.as_str()).map(String::from);
                    if quantity.is_none() {
                        let parse_from = if !display.is_empty() {
                            display.clone()
                        } else {
                            ingredient.clone()
                        };
                        if !parse_from.is_empty() {
                            let (q, u, name, n) = ImportService::parse_ingredient_line(&parse_from);
                            quantity = q;
                            if unit.is_none() {
                                unit = u;
                            }
                            if ingredient.is_empty() {
                                ingredient = name;
                            }
                            if note.is_none() {
                                note = n;
                            }
                            if display.is_empty() {
                                display = parse_from;
                            }
                        }
                    }
                    if display.is_empty() {
                        display = ingredient.clone();
                    }
                    if display.trim().is_empty() && ingredient.trim().is_empty() {
                        return None;
                    }
                    Some(BundleRecipeIngredient {
                        ingredient: if ingredient.is_empty() {
                            display.clone()
                        } else {
                            ingredient
                        },
                        quantity,
                        unit,
                        note,
                        display,
                        category: obj
                            .get("category")
                            .and_then(|x| x.as_str())
                            .map(String::from),
                        cost_per_unit: None,
                        line_cost: None,
                        master_ingredient_name: None,
                        prep_yield_percent: None,
                    })
                }
                _ => None,
            }
        })
        .collect();

    let steps = legacy
        .steps
        .iter()
        .enumerate()
        .filter_map(|(i, v)| {
            let instruction = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Object(obj) => obj
                    .get("instruction")
                    .or_else(|| obj.get("text"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                _ => return None,
            };
            if instruction.trim().is_empty() {
                return None;
            }
            Some(BundleStep {
                position: i as u32,
                instruction,
                timer_seconds: None,
            })
        })
        .collect();

    BundleRecipe {
        name: legacy.name,
        description: legacy.description,
        image_url: None,
        servings: legacy.servings.unwrap_or(1),
        prep_time_minutes: legacy.prep_time_minutes,
        cook_time_minutes: legacy.cook_time_minutes,
        total_time_minutes: legacy.total_time_minutes,
        source_url: legacy.source_url,
        rating: None,
        difficulty: None,
        menu_price: None,
        yield_quantity: json_to_decimal(&legacy.yield_quantity),
        yield_unit: legacy
            .yield_unit
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        waste_percent: None,
        author: None,
        estimated_calories: None,
        allergens: None,
        tags: vec![],
        ingredients,
        steps,
    }
}

fn parse_difficulty(s: &str) -> Option<Difficulty> {
    match s.to_lowercase().as_str() {
        "easy" => Some(Difficulty::Easy),
        "medium" => Some(Difficulty::Medium),
        "hard" => Some(Difficulty::Hard),
        _ => None,
    }
}

fn csv_row(out: &mut String, fields: &[&str]) {
    let escaped: Vec<String> = fields
        .iter()
        .map(|f| {
            if f.contains(',') || f.contains('"') || f.contains('\n') {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                (*f).to_string()
            }
        })
        .collect();
    out.push_str(&escaped.join(","));
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_legacy_array() {
        let json = r#"[
          {"name":"Toast","servings":2,"ingredients":["2 slices bread"],"steps":["Toast it"]}
        ]"#;
        let bundle = BundleService::parse(json).unwrap();
        assert_eq!(bundle.recipes.len(), 1);
        assert_eq!(bundle.recipes[0].name, "Toast");
        assert_eq!(bundle.recipes[0].ingredients[0].display, "2 slices bread");
        assert_eq!(
            bundle.recipes[0].ingredients[0].quantity,
            Some(Decimal::from(2))
        );
        assert_eq!(
            bundle.recipes[0].ingredients[0].unit.as_deref(),
            Some("slices")
        );
    }

    #[test]
    fn parse_legacy_keeps_yield_for_batch_scale() {
        let json = r#"[
          {"name":"Rolls","servings":12,"yield_quantity":24,"yield_unit":"rolls","ingredients":["4 cups flour"]}
        ]"#;
        let bundle = BundleService::parse(json).unwrap();
        assert_eq!(bundle.recipes[0].yield_quantity, Some(Decimal::from(24)));
        assert_eq!(bundle.recipes[0].yield_unit.as_deref(), Some("rolls"));
        assert_eq!(
            bundle.recipes[0].ingredients[0].quantity,
            Some(Decimal::from(4))
        );
        assert_eq!(
            bundle.recipes[0].ingredients[0].unit.as_deref(),
            Some("cups")
        );
    }

    #[test]
    fn csv_escapes_commas() {
        let bundle = LarderBundle {
            larder_version: 1,
            exported_at: String::new(),
            ingredients: vec![],
            location_prices: vec![],
            recipes: vec![BundleRecipe {
                name: "Soup".into(),
                description: None,
                image_url: None,
                servings: 4,
                prep_time_minutes: None,
                cook_time_minutes: None,
                total_time_minutes: None,
                source_url: None,
                rating: None,
                difficulty: None,
                menu_price: None,
                yield_quantity: None,
                yield_unit: None,
                waste_percent: None,
                author: None,
                estimated_calories: None,
                allergens: None,
                tags: vec![],
                ingredients: vec![BundleRecipeIngredient {
                    ingredient: "onion, diced".into(),
                    quantity: None,
                    unit: None,
                    note: None,
                    display: "1 onion, diced".into(),
                    category: None,
                    cost_per_unit: None,
                    line_cost: None,
                    master_ingredient_name: None,
                    prep_yield_percent: None,
                }],
                steps: vec![],
            }],
        };
        let csv = BundleService::to_csv_recipes(&bundle);
        assert!(csv.contains("\"onion, diced\""));
    }
}
