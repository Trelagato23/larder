use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Html,
};
use larder_core::services::scaling::scale_display_text;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(serde::Deserialize)]
pub struct PrepQuery {
    #[serde(default)]
    pub servings: Option<u32>,
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(params): Query<PrepQuery>,
) -> Result<Html<String>, (StatusCode, String)> {
    let recipe = state
        .recipes
        .get_recipe(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Recipe not found".to_string()))?;

    let ingredients = state
        .recipes
        .get_ingredients(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let steps = state
        .recipes
        .get_steps(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let servings = params.servings.unwrap_or(recipe.servings).max(1);
    let scaled = servings != recipe.servings;

    let mut meta = Vec::new();
    if let Some(p) = recipe.prep_time_minutes {
        meta.push(format!("Prep: {} min", p));
    }
    if let Some(c) = recipe.cook_time_minutes {
        meta.push(format!("Cook: {} min", c));
    }
    if let Some(t) = recipe.total_time() {
        meta.push(format!("Total: {} min", t));
    }
    let yield_line = if scaled {
        format!(
            "Yield: {} servings (scaled from {})",
            servings, recipe.servings
        )
    } else {
        format!("Yield: {} servings", servings)
    };

    let ingredient_rows: String = ingredients
        .iter()
        .map(|i| {
            let display = if scaled {
                scale_display_text(&i.display, recipe.servings, servings)
            } else {
                i.display.clone()
            };
            format!(
                "<li><span class=\"box\"></span> {}</li>",
                escape_html(&display)
            )
        })
        .collect();

    let step_rows: String = steps
        .iter()
        .map(|s| {
            let timer = s.timer_seconds.map(|t| {
                let m = t / 60;
                let sec = t % 60;
                format!("<span class=\"timer\">⏱ {}:{:02}</span>", m, sec)
            }).unwrap_or_default();
            format!(
                "<li><strong>{}.</strong> {} {}",
                s.position,
                escape_html(&s.instruction),
                timer
            )
        })
        .collect();

    let description = recipe
        .description
        .as_deref()
        .map(|d| format!("<p class=\"desc\">{}</p>", escape_html(d)))
        .unwrap_or_default();

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>{title} — Prep Sheet</title>
<style>
  @page {{ margin: 0.6in; }}
  body {{ font-family: Georgia, 'Times New Roman', serif; color: #111; margin: 0; padding: 0; }}
  h1 {{ font-size: 1.75rem; margin: 0 0 0.25rem; border-bottom: 2px solid #111; padding-bottom: 0.35rem; }}
  .meta {{ color: #444; font-size: 0.95rem; margin-bottom: 1rem; }}
  .meta span {{ margin-right: 1.25rem; }}
  .desc {{ color: #333; margin-bottom: 1rem; }}
  h2 {{ font-size: 1.1rem; text-transform: uppercase; letter-spacing: 0.05em; margin: 1.25rem 0 0.5rem; border-bottom: 1px solid #ccc; }}
  ul.ingredients {{ list-style: none; padding: 0; margin: 0; }}
  ul.ingredients li {{ padding: 0.35rem 0; font-size: 1.05rem; display: flex; align-items: baseline; gap: 0.5rem; }}
  .box {{ display: inline-block; width: 0.85rem; height: 0.85rem; border: 1.5px solid #111; flex-shrink: 0; }}
  ol.steps {{ padding-left: 1.25rem; margin: 0; }}
  ol.steps li {{ padding: 0.5rem 0; line-height: 1.5; font-size: 1.05rem; }}
  .timer {{ font-weight: bold; white-space: nowrap; }}
  .footer {{ margin-top: 1.5rem; padding-top: 0.5rem; border-top: 1px solid #ccc; font-size: 0.8rem; color: #666; }}
  .noprint {{ margin: 1rem; }}
  @media print {{ .noprint {{ display: none; }} }}
</style>
</head>
<body>
<div class="noprint"><button onclick="window.print()">Print</button></div>
<h1>{title}</h1>
<div class="meta"><span>{yield_line}</span>{meta}</div>
{description}
<h2>Ingredients</h2>
<ul class="ingredients">{ingredient_rows}</ul>
<h2>Method</h2>
<ol class="steps">{step_rows}</ol>
<div class="footer">Printed {date} · Larder prep sheet</div>
<script>window.onload = function() {{ if (new URLSearchParams(location.search).get('auto') === '1') window.print(); }};</script>
</body>
</html>"#,
        title = escape_html(&recipe.name),
        yield_line = yield_line,
        meta = meta
            .iter()
            .map(|m| format!("<span>{}</span>", escape_html(m)))
            .collect::<Vec<_>>()
            .join(""),
        description = description,
        ingredient_rows = ingredient_rows,
        step_rows = step_rows,
        date = chrono::Local::now().format("%Y-%m-%d"),
    );

    Ok(Html(html))
}
