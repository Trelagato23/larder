use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use rust_decimal::Decimal;
use scraper::{Html, Selector};
use serde_json::Value;
use tracing::{info, warn};
use uuid::Uuid;

use crate::models::{Recipe, RecipeIngredient, RecipeStep};

pub struct ImportService {
    client: Client,
}

pub struct ImportedRecipe {
    pub recipe: Recipe,
    pub ingredients: Vec<RecipeIngredient>,
    pub steps: Vec<RecipeStep>,
}

impl ImportService {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (compatible; RecipeBox/1.0)")
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn import_from_url(&self, url: &str) -> Result<ImportedRecipe> {
        info!("Importing recipe from URL: {}", url);

        let html = self.client.get(url).send().await?.text().await?;
        let document = Html::parse_document(&html);

        if let Some(result) = self.parse_json_ld(&document, url) {
            return Ok(result);
        }

        warn!("JSON-LD parsing failed, falling back to heuristics");
        self.parse_heuristics(&document, url)
    }

    fn parse_json_ld(&self, document: &Html, url: &str) -> Option<ImportedRecipe> {
        let script_selector = Selector::parse("script[type=\"application/ld+json\"]").ok()?;

        for element in document.select(&script_selector) {
            let text = element.inner_html();
            let cleaned = Self::clean_json(&text);

            if let Ok(json) = serde_json::from_str::<Value>(&cleaned) {
                if let Some(result) = self.extract_recipe_from_json(json, url) {
                    return Some(result);
                }
            }
        }
        None
    }

    fn clean_json(text: &str) -> String {
        let text = text.trim();
        if text.starts_with("/*") {
            if let Some(end) = text.find("*/") {
                return text[end + 2..].trim().to_string();
            }
        }
        text.to_string()
    }

    fn extract_recipe_from_json(&self, json: Value, url: &str) -> Option<ImportedRecipe> {
        let recipe_obj = self.find_recipe_object(json)?;
        let recipe = recipe_obj.as_object()?;

        let name = recipe.get("name")?.as_str()?.trim().to_string();
        if name.is_empty() {
            return None;
        }

        let description = recipe
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string());

        let image_url = self.extract_image(recipe);

        let prep_time = recipe
            .get("prepTime")
            .and_then(|v| Self::parse_duration(v.as_str()?));

        let cook_time = recipe
            .get("cookTime")
            .and_then(|v| Self::parse_duration(v.as_str()?));

        let total_time = recipe
            .get("totalTime")
            .and_then(|v| Self::parse_duration(v.as_str()?));

        let servings = recipe
            .get("recipeYield")
            .and_then(|v| Self::parse_servings(v));

        let ingredients = self.extract_ingredients(recipe);
        let steps = self.extract_steps(recipe);

        let recipe_model = Recipe {
            id: Uuid::new_v4(),
            name,
            description,
            image_url,
            servings: servings.unwrap_or(1),
            prep_time_minutes: prep_time,
            cook_time_minutes: cook_time,
            total_time_minutes: total_time,
            source_url: Some(url.to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: Uuid::nil(),
            rating: None,
            difficulty: None,
            menu_price: None,
        };

        Some(ImportedRecipe {
            recipe: recipe_model,
            ingredients,
            steps,
        })
    }

    fn find_recipe_object(&self, json: Value) -> Option<Value> {
        if let Some(obj) = json.as_object() {
            if let Some(type_val) = obj.get("@type") {
                if self.is_recipe_type(type_val) {
                    return Some(json);
                }
            }

            if let Some(graph) = obj.get("@graph").and_then(|v| v.as_array()) {
                for item in graph {
                    if let Some(obj) = item.as_object() {
                        if let Some(type_val) = obj.get("@type") {
                            if self.is_recipe_type(type_val) {
                                return Some(item.clone());
                            }
                        }
                    }
                }
            }

            for (_, value) in obj {
                if let Some(found) = self.find_recipe_object(value.clone()) {
                    return Some(found);
                }
            }
        }

        if let Some(arr) = json.as_array() {
            for item in arr {
                if let Some(found) = self.find_recipe_object(item.clone()) {
                    return Some(found);
                }
            }
        }

        None
    }

    fn is_recipe_type(&self, type_val: &Value) -> bool {
        match type_val {
            Value::String(s) => s.contains("Recipe"),
            Value::Array(arr) => arr
                .iter()
                .any(|v| v.as_str().map(|s| s.contains("Recipe")).unwrap_or(false)),
            _ => false,
        }
    }

    fn extract_image(&self, recipe: &serde_json::Map<String, Value>) -> Option<String> {
        recipe.get("image").and_then(|img| match img {
            Value::String(s) => Some(s.clone()),
            Value::Array(arr) => arr.first().and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Object(obj) => obj.get("url").and_then(|u| u.as_str()).map(String::from),
                _ => None,
            }),
            Value::Object(obj) => obj.get("url").and_then(|u| u.as_str()).map(String::from),
            _ => None,
        })
    }

    fn extract_ingredients(
        &self,
        recipe: &serde_json::Map<String, Value>,
    ) -> Vec<RecipeIngredient> {
        let ingredients = match recipe.get("recipeIngredient") {
            Some(Value::Array(arr)) => arr,
            Some(Value::String(s)) => {
                return vec![RecipeIngredient {
                    id: Uuid::new_v4(),
                    recipe_id: Uuid::nil(),
                    ingredient: s.clone(),
                    quantity: None,
                    unit: None,
                    note: None,
                    display: s.clone(),
                    category: None,
                    cost_per_unit: None,
                    line_cost: None,
                }];
            }
            _ => return vec![],
        };

        ingredients
            .iter()
            .filter_map(|ing| {
                let text = ing.as_str()?.trim().to_string();
                if text.is_empty() {
                    return None;
                }

                let (quantity, unit, ingredient, note) = Self::parse_ingredient(&text);

                Some(RecipeIngredient {
                    id: Uuid::new_v4(),
                    recipe_id: Uuid::nil(),
                    ingredient,
                    quantity,
                    unit,
                    note,
                    display: text.clone(),
                    category: None,
                    cost_per_unit: None,
                    line_cost: None,
                })
            })
            .collect()
    }

    fn parse_ingredient(text: &str) -> (Option<Decimal>, Option<String>, String, Option<String>) {
        let text = text.trim();

        if let Some((qty, rest)) = Self::extract_quantity(text) {
            let (unit, ingredient_with_note) = Self::extract_unit(rest.trim());
            let (ingredient, note) = Self::extract_note(&ingredient_with_note);

            (Some(qty), unit, ingredient, note)
        } else {
            (None, None, text.to_string(), None)
        }
    }

    fn extract_quantity(text: &str) -> Option<(Decimal, &str)> {
        if let Some((whole, frac, rest)) = Self::parse_fraction(text) {
            let qty = if let Some(frac) = frac {
                whole + frac
            } else {
                whole
            };
            return Some((qty, rest));
        }

        if let Some((num, rest)) = Self::parse_decimal(text) {
            return Some((num, rest));
        }

        None
    }

    fn parse_fraction(text: &str) -> Option<(Decimal, Option<Decimal>, &str)> {
        let parts: Vec<&str> = text.splitn(2, char::is_whitespace).collect();

        if parts.len() == 2 {
            if let Some(frac) = Self::parse_simple_fraction(parts[0]) {
                let rest = parts[1];
                if let Ok(whole) = parts[0].parse::<i64>() {
                    return Some((Decimal::from(whole), Some(frac), rest));
                }
            }
        }

        if let Some(frac) = Self::parse_simple_fraction(parts[0]) {
            let rest = if parts.len() > 1 { parts[1] } else { "" };
            return Some((Decimal::ZERO, Some(frac), rest));
        }

        None
    }

    fn parse_simple_fraction(s: &str) -> Option<Decimal> {
        match s {
            "1/2" => Some(Decimal::new(1, 1) / Decimal::new(2, 0)),
            "1/3" => Some(Decimal::new(1, 1) / Decimal::new(3, 0)),
            "2/3" => Some(Decimal::new(2, 1) / Decimal::new(3, 0)),
            "1/4" => Some(Decimal::new(1, 1) / Decimal::new(4, 0)),
            "3/4" => Some(Decimal::new(3, 1) / Decimal::new(4, 0)),
            "1/5" => Some(Decimal::new(1, 1) / Decimal::new(5, 0)),
            "1/8" => Some(Decimal::new(1, 1) / Decimal::new(8, 0)),
            "3/8" => Some(Decimal::new(3, 1) / Decimal::new(8, 0)),
            "5/8" => Some(Decimal::new(5, 1) / Decimal::new(8, 0)),
            "7/8" => Some(Decimal::new(7, 1) / Decimal::new(8, 0)),
            _ => None,
        }
    }

    fn parse_decimal(text: &str) -> Option<(Decimal, &str)> {
        let mut num_str = String::new();
        let mut found_dot = false;

        for (i, c) in text.char_indices() {
            if c.is_ascii_digit() {
                num_str.push(c);
            } else if c == '.' && !found_dot {
                found_dot = true;
                num_str.push(c);
            } else {
                if !num_str.is_empty() && num_str != "." {
                    if let Ok(num) = num_str.parse::<Decimal>() {
                        return Some((num, &text[i..]));
                    }
                }
                return None;
            }
        }

        if !num_str.is_empty() && num_str != "." {
            if let Ok(num) = num_str.parse::<Decimal>() {
                return Some((num, ""));
            }
        }

        None
    }

    fn extract_unit(text: &str) -> (Option<String>, String) {
        let common_units = [
            "cup",
            "cups",
            "tbsp",
            "tablespoon",
            "tablespoons",
            "tsp",
            "teaspoon",
            "teaspoons",
            "oz",
            "ounce",
            "ounces",
            "lb",
            "lbs",
            "pound",
            "pounds",
            "g",
            "kg",
            "ml",
            "l",
            "liter",
            "liters",
            "pint",
            "pints",
            "quart",
            "quarts",
            "gallon",
            "gallons",
            "pinch",
            "dash",
            "can",
            "cans",
            "bottle",
            "bottles",
            "piece",
            "pieces",
            "slice",
            "slices",
            "clove",
            "cloves",
            "bunch",
            "bunches",
            "sprig",
            "sprigs",
            "whole",
            "large",
            "medium",
            "small",
        ];

        let parts: Vec<&str> = text.splitn(2, char::is_whitespace).collect();

        if parts.is_empty() {
            return (None, text.to_string());
        }

        let first = parts[0].to_lowercase();
        if common_units.contains(&first.as_str()) {
            let rest = if parts.len() > 1 { parts[1] } else { "" };
            return (Some(parts[0].to_string()), rest.to_string());
        }

        (None, text.to_string())
    }

    fn extract_note(text: &str) -> (String, Option<String>) {
        let text = text.trim();

        if let Some(pos) = text.find(',') {
            let ingredient = text[..pos].trim().to_string();
            let note = text[pos + 1..].trim().to_string();
            if !note.is_empty() {
                return (ingredient, Some(note));
            }
        }

        for prefix in ["-", "–", "—"] {
            if let Some(pos) = text.find(prefix) {
                let ingredient = text[..pos].trim().to_string();
                let note = text[pos + prefix.len()..].trim().to_string();
                if !note.is_empty() {
                    return (ingredient, Some(note));
                }
            }
        }

        (text.to_string(), None)
    }

    fn extract_steps(&self, recipe: &serde_json::Map<String, Value>) -> Vec<RecipeStep> {
        let steps_value = match recipe.get("recipeInstructions") {
            Some(v) => v,
            None => return vec![],
        };

        let mut steps = Vec::new();
        let mut position = 0;

        match steps_value {
            Value::Array(arr) => {
                for item in arr {
                    if let Some(section) = item.as_object() {
                        if section.contains_key("@type") {
                            if let Some(type_val) = section.get("@type") {
                                if let Some(s) = type_val.as_str() {
                                    if s.contains("HowToSection") || s.contains("HowToStep") {
                                        if s.contains("HowToSection") {
                                            if let Some(sub_steps) = section.get("itemListElement")
                                            {
                                                if let Some(sub_arr) = sub_steps.as_array() {
                                                    for sub in sub_arr {
                                                        if let Some(step) =
                                                            self.parse_step_object(sub, position)
                                                        {
                                                            steps.push(step);
                                                            position += 1;
                                                        }
                                                    }
                                                }
                                            }
                                            continue;
                                        } else if s.contains("HowToStep") {
                                            if let Some(step) =
                                                self.parse_step_object(item, position)
                                            {
                                                steps.push(step);
                                                position += 1;
                                            }
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(step) = self.parse_step_object(item, position) {
                        steps.push(step);
                        position += 1;
                    }
                }
            }
            Value::String(s) => {
                let html = Html::parse_document(s);
                let p_selector = Selector::parse("p").ok();
                let li_selector = Selector::parse("li").ok();
                let br_selector = Selector::parse("br").ok();

                let text_parts: Vec<String> = if let Some(ref p_sel) = p_selector {
                    html.select(p_sel).map(|el| el.inner_html()).collect()
                } else if let Some(ref li_sel) = li_selector {
                    html.select(li_sel).map(|el| el.inner_html()).collect()
                } else if br_selector.is_some() {
                    let full_text = html.root_element().inner_html();
                    full_text
                        .split("<br>")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                } else {
                    vec![s.clone()]
                };

                for text in text_parts {
                    let clean = Self::strip_html(&text);
                    if !clean.trim().is_empty() {
                        steps.push(RecipeStep {
                            id: Uuid::new_v4(),
                            recipe_id: Uuid::nil(),
                            position: position as u32,
                            instruction: clean.trim().to_string(),
                            timer_seconds: None,
                        });
                        position += 1;
                    }
                }
            }
            _ => {}
        }

        steps
    }

    fn parse_step_object(&self, value: &Value, position: usize) -> Option<RecipeStep> {
        match value {
            Value::String(s) => {
                let clean = Self::strip_html(s);
                if clean.trim().is_empty() {
                    return None;
                }
                Some(RecipeStep {
                    id: Uuid::new_v4(),
                    recipe_id: Uuid::nil(),
                    position: position as u32,
                    instruction: clean.trim().to_string(),
                    timer_seconds: None,
                })
            }
            Value::Object(obj) => {
                let instruction = obj
                    .get("text")
                    .or_else(|| obj.get("name"))
                    .and_then(|v| v.as_str())?;

                let timer = obj
                    .get("timer")
                    .and_then(|v| Self::parse_duration(v.as_str()?));

                Some(RecipeStep {
                    id: Uuid::new_v4(),
                    recipe_id: Uuid::nil(),
                    position: position as u32,
                    instruction: instruction.trim().to_string(),
                    timer_seconds: timer,
                })
            }
            _ => None,
        }
    }

    fn strip_html(html: &str) -> String {
        let document = Html::parse_document(html);
        document.root_element().text().collect::<Vec<_>>().join(" ")
    }

    fn parse_duration(s: &str) -> Option<u32> {
        let s = s.trim();

        if s.starts_with("PT") {
            let s = &s[2..];
            let mut total_minutes = 0;

            let mut num_str = String::new();
            for c in s.chars() {
                if c.is_ascii_digit() {
                    num_str.push(c);
                } else if c == 'H' {
                    if let Ok(hours) = num_str.parse::<u32>() {
                        total_minutes += hours * 60;
                    }
                    num_str.clear();
                } else if c == 'M' {
                    if let Ok(minutes) = num_str.parse::<u32>() {
                        total_minutes += minutes;
                    }
                    num_str.clear();
                } else if c == 'S' {
                    if let Ok(seconds) = num_str.parse::<u32>() {
                        total_minutes += seconds / 60;
                    }
                    num_str.clear();
                }
            }

            if total_minutes > 0 {
                return Some(total_minutes);
            }
        }

        if let Ok(minutes) = s
            .trim_end_matches(" minutes")
            .trim_end_matches(" mins")
            .trim_end_matches(" min")
            .parse::<u32>()
        {
            return Some(minutes);
        }

        if let Ok(hours) = s
            .trim_end_matches(" hours")
            .trim_end_matches(" hrs")
            .trim_end_matches(" hr")
            .parse::<u32>()
        {
            return Some(hours * 60);
        }

        None
    }

    fn parse_servings(value: &Value) -> Option<u32> {
        match value {
            Value::Number(n) => n.as_u64().map(|v| v as u32),
            Value::String(s) => {
                if let Ok(n) = s.parse::<u32>() {
                    return Some(n);
                }
                for word in s.split_whitespace() {
                    if let Ok(n) = word.parse::<u32>() {
                        return Some(n);
                    }
                }
                None
            }
            Value::Array(arr) => arr.first().and_then(Self::parse_servings),
            _ => None,
        }
    }

    fn parse_heuristics(&self, document: &Html, url: &str) -> Result<ImportedRecipe> {
        let title_selector = Selector::parse("h1").ok();
        let name = title_selector
            .and_then(|sel| document.select(&sel).next())
            .map(|el| el.inner_html())
            .unwrap_or_else(|| "Imported Recipe".to_string());

        info!("Using heuristic parsing for recipe: {}", name);

        Ok(ImportedRecipe {
            recipe: Recipe {
                id: Uuid::new_v4(),
                name,
                description: None,
                image_url: None,
                servings: 1,
                prep_time_minutes: None,
                cook_time_minutes: None,
                total_time_minutes: None,
                source_url: Some(url.to_string()),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                user_id: Uuid::nil(),
                rating: None,
                difficulty: None,
                menu_price: None,
            },
            ingredients: vec![],
            steps: vec![],
        })
    }
}
