use anyhow::Result;
use uuid::Uuid;

use crate::models::{Recipe, RecipeIngredient, RecipeStep};

pub struct ExportService;

impl ExportService {
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
            if let Some(ref url) = recipe.source_url {
                meta.push(format!("Source: {}", url));
            }
            output.push_str(&format!("{}\n\n", meta.join(" | ")));

            output.push_str("## Ingredients\n\n");
            if let Some((_, ings)) = ingredients.iter().find(|(id, _)| *id == recipe.id) {
                for ing in ings {
                    output.push_str(&format!("- {}\n", ing.display));
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
