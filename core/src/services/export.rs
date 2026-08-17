use anyhow::Result;
use uuid::Uuid;

use crate::models::{Recipe, RecipeIngredient, RecipeStep};
use crate::services::{
    BundleService, IngredientMasterService, LocationService, LarderBundle, RecipeService,
};

pub struct ExportService;

impl ExportService {
    pub async fn export_bundle(
        recipes: &RecipeService,
        ingredients: &IngredientMasterService,
        locations: &LocationService,
        user_id: Uuid,
    ) -> Result<LarderBundle> {
        BundleService::export(recipes, ingredients, locations, user_id).await
    }

    pub async fn export_bundle_filtered(
        recipes: &RecipeService,
        ingredients: &IngredientMasterService,
        locations: &LocationService,
        user_id: Uuid,
        tag: Option<&str>,
        cookbook_ids: Option<&[Uuid]>,
    ) -> Result<LarderBundle> {
        BundleService::export_filtered(recipes, ingredients, locations, user_id, tag, cookbook_ids)
            .await
    }

    pub fn bundle_to_json(bundle: &LarderBundle) -> Result<String> {
        BundleService::to_json(bundle)
    }

    /// Backward-compatible simple JSON (recipe cookbook only).
    pub fn to_json(
        recipes: &[Recipe],
        ingredients: &[(Uuid, Vec<RecipeIngredient>)],
        steps: &[(Uuid, Vec<RecipeStep>)],
    ) -> Result<String> {
        let mut output = Vec::new();
        for recipe in recipes {
            let ings = ingredients
                .iter()
                .find(|(id, _)| *id == recipe.id)
                .map(|(_, v)| v)
                .cloned()
                .unwrap_or_default();

            let stps = steps
                .iter()
                .find(|(id, _)| *id == recipe.id)
                .map(|(_, v)| v)
                .cloned()
                .unwrap_or_default();

            output.push(serde_json::json!({
                "name": recipe.name,
                "description": recipe.description,
                "servings": recipe.servings,
                "prep_time_minutes": recipe.prep_time_minutes,
                "cook_time_minutes": recipe.cook_time_minutes,
                "total_time_minutes": recipe.total_time(),
                "source_url": recipe.source_url,
                "ingredients": ings.iter().map(|i| i.display.clone()).collect::<Vec<_>>(),
                "steps": stps.iter().map(|s| &s.instruction).collect::<Vec<_>>(),
            }));
        }

        Ok(serde_json::to_string_pretty(&output)?)
    }

    pub fn to_markdown(
        recipes: &[Recipe],
        ingredients: &[(Uuid, Vec<RecipeIngredient>)],
        steps: &[(Uuid, Vec<RecipeStep>)],
        tags: &[(Uuid, Vec<String>)],
    ) -> Result<String> {
        let mut output = String::new();

        for recipe in recipes {
            output.push_str(&format!("# {}\n\n", recipe.name));

            if let Some(ref desc) = recipe.description {
                output.push_str(&format!("{}\n\n", desc));
            }

            let mut meta = Vec::new();
            if let Some(t) = recipe.total_time() {
                meta.push(format!("{} min", t));
            }
            meta.push(format!("{} servings", recipe.servings));
            if let Some(p) = recipe.menu_price {
                meta.push(format!("Menu ${}", p));
            }
            if let Some(ref url) = recipe.source_url {
                meta.push(format!("Source: {}", url));
            }
            if let Some(tags_for) = tags.iter().find(|(id, _)| *id == recipe.id) {
                if !tags_for.1.is_empty() {
                    meta.push(format!(
                        "Tags: {}",
                        tags_for
                            .1
                            .iter()
                            .map(|t| format!("#{}", t))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            output.push_str(&format!("{}\n\n", meta.join(" | ")));

            output.push_str("## Ingredients\n\n");
            if let Some((_, ings)) = ingredients.iter().find(|(id, _)| *id == recipe.id) {
                for ing in ings {
                    let mut line = format!("- {}", ing.display);
                    if let Some(cost) = ing.cost_per_unit {
                        line.push_str(&format!(" (${}/u)", cost));
                    }
                    output.push_str(&format!("{}\n", line));
                }
            }
            output.push('\n');

            output.push_str("## Steps\n\n");
            if let Some((_, stps)) = steps.iter().find(|(id, _)| *id == recipe.id) {
                for (i, step) in stps.iter().enumerate() {
                    output.push_str(&format!("{}. {}", i + 1, step.instruction));
                    if let Some(timer) = step.timer_seconds {
                        output.push_str(&format!(" [{}:{:02}]", timer / 60, timer % 60));
                    }
                    output.push('\n');
                }
            }

            output.push_str("\n---\n\n");
        }

        Ok(output)
    }
}
