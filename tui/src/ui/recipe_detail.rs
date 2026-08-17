use larder_core::{
    models::{Recipe, RecipeIngredient, RecipeStep, Tag},
    services::{
        cost::{format_money, ingredient_line_cost, recipe_ingredient_cost, food_cost_percent},
        scaling::{combined_scale_factor, format_quantity, scale_display_by_factor},
    },
};
use rust_decimal::Decimal;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::time::Instant;

/// Sub-batch sizes the stepper offers below 1 batch.
fn batch_fractions() -> [Decimal; 3] {
    [Decimal::new(25, 2), Decimal::new(5, 1), Decimal::new(75, 2)]
}

fn step_batch_up(current: Decimal) -> Decimal {
    let fractions = batch_fractions();
    if let Some(i) = fractions.iter().position(|&f| f == current) {
        if i + 1 < fractions.len() {
            return fractions[i + 1];
        }
    }
    if current < Decimal::ONE {
        return Decimal::ONE;
    }
    current + Decimal::ONE
}

fn step_batch_down(current: Decimal) -> Decimal {
    let fractions = batch_fractions();
    if current > Decimal::ONE {
        return current - Decimal::ONE;
    }
    if current == Decimal::ONE {
        return fractions[fractions.len() - 1];
    }
    if let Some(i) = fractions.iter().position(|&f| f == current) {
        if i > 0 {
            return fractions[i - 1];
        }
    }
    current
}

fn fmt_batch(batches: Decimal) -> String {
    let quarter = Decimal::new(25, 2);
    let half = Decimal::new(5, 1);
    let three_quarter = Decimal::new(75, 2);
    if batches == quarter {
        "¼".to_string()
    } else if batches == half {
        "½".to_string()
    } else if batches == three_quarter {
        "¾".to_string()
    } else {
        format_quantity(&batches)
    }
}

pub struct RecipeDetailState {
    recipe: Recipe,
    ingredients: Vec<RecipeIngredient>,
    steps: Vec<RecipeStep>,
    tags: Vec<Tag>,
    scroll: u16,
    cooking_mode: bool,
    current_step: usize,
    display_servings: u32,
    display_batches: Decimal,
    batch_locked: bool,
    timer_remaining: Option<u32>,
    timer_running: bool,
    last_tick: Option<Instant>,
}

impl RecipeDetailState {
    pub fn new(
        recipe: Recipe,
        ingredients: Vec<RecipeIngredient>,
        steps: Vec<RecipeStep>,
        tags: Vec<Tag>,
    ) -> Self {
        let display_servings = recipe.servings;
        let batch_locked = tags
            .iter()
            .any(|t| t.name.eq_ignore_ascii_case("sandwiches"));
        Self {
            recipe,
            ingredients,
            steps,
            tags,
            scroll: 0,
            cooking_mode: false,
            current_step: 0,
            display_servings,
            display_batches: Decimal::ONE,
            batch_locked,
            timer_remaining: None,
            timer_running: false,
            last_tick: None,
        }
    }

    pub fn recipe_id(&self) -> uuid::Uuid {
        self.recipe.id
    }

    pub fn recipe(&self) -> &Recipe {
        &self.recipe
    }

    pub fn ingredients(&self) -> &[RecipeIngredient] {
        &self.ingredients
    }

    pub fn steps(&self) -> &[RecipeStep] {
        &self.steps
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn page_down(&mut self) {
        self.scroll = self.scroll.saturating_add(10);
    }

    pub fn page_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(10);
    }

    pub fn scale_up(&mut self) {
        self.display_servings = self.display_servings.saturating_add(1);
    }

    pub fn scale_down(&mut self) {
        if self.display_servings > 1 {
            self.display_servings -= 1;
        }
    }

    pub fn toggle_cooking_mode(&mut self) {
        self.cooking_mode = !self.cooking_mode;
        if self.cooking_mode {
            self.current_step = 0;
            self.reset_step_timer();
        } else {
            self.timer_running = false;
            self.timer_remaining = None;
        }
    }

    pub fn cooking_mode(&self) -> bool {
        self.cooking_mode
    }

    pub fn next_step(&mut self) {
        if self.current_step + 1 < self.steps.len() {
            self.current_step += 1;
            self.reset_step_timer();
        }
    }

    pub fn prev_step(&mut self) {
        if self.current_step > 0 {
            self.current_step -= 1;
            self.reset_step_timer();
        }
    }

    pub fn toggle_timer(&mut self) {
        if self.timer_remaining.is_some() {
            self.timer_running = !self.timer_running;
            self.last_tick = Some(Instant::now());
        }
    }

    pub fn tick(&mut self) {
        if !self.timer_running {
            return;
        }
        let now = Instant::now();
        let elapsed = self
            .last_tick
            .map(|t| now.duration_since(t).as_secs() as u32)
            .unwrap_or(0);
        if elapsed == 0 {
            return;
        }
        self.last_tick = Some(now);
        if let Some(remaining) = self.timer_remaining.as_mut() {
            *remaining = remaining.saturating_sub(elapsed);
            if *remaining == 0 {
                self.timer_running = false;
            }
        }
    }

    fn reset_step_timer(&mut self) {
        self.timer_remaining = self
            .steps
            .get(self.current_step)
            .and_then(|s| s.timer_seconds);
        self.timer_running = false;
        self.last_tick = None;
    }

    fn scaled_ingredient_display(&self, ingredient: &RecipeIngredient) -> String {
        scale_display_by_factor(&ingredient.display, self.scale_factor())
    }

    pub fn batch_up(&mut self) {
        if self.batch_locked {
            return;
        }
        self.display_batches = step_batch_up(self.display_batches);
    }

    pub fn batch_down(&mut self) {
        if self.batch_locked {
            return;
        }
        self.display_batches = step_batch_down(self.display_batches);
    }

    fn scale_factor(&self) -> Decimal {
        combined_scale_factor(
            self.recipe.servings,
            self.display_servings,
            self.display_batches,
        )
    }

    pub fn total_cost(&self) -> Decimal {
        recipe_ingredient_cost(&self.ingredients, self.scale_factor())
    }

    fn ingredient_cost_label(&self, ingredient: &RecipeIngredient) -> Option<String> {
        ingredient_line_cost(ingredient, self.scale_factor()).map(format_money)
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &RecipeDetailState, status: &str) {
    use super::theme::T;
    use ratatui::widgets::BorderType;

    if state.cooking_mode() {
        render_cooking_mode(frame, area, state, status);
        return;
    }

    let has_tags = !state.tags.is_empty();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(if has_tags { 3 } else { 0 }),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let mut header_lines = vec![Line::from(vec![Span::styled(
        &state.recipe.name,
        Style::default()
            .fg(T.accent)
            .add_modifier(Modifier::BOLD),
    )])];

    if let Some(ref desc) = state.recipe.description {
        let clipped: String = {
            let chars: Vec<char> = desc.chars().collect();
            if chars.len() > 72 {
                chars.into_iter().take(69).collect::<String>() + "…"
            } else {
                desc.clone()
            }
        };
        header_lines.push(Line::from(vec![Span::styled(
            clipped,
            Style::default().fg(T.muted),
        )]));
    }

    let mut meta_spans: Vec<Span> = Vec::new();
    if let Some(t) = state.recipe.total_time() {
        meta_spans.push(Span::styled(
            format!("{} min", t),
            Style::default().fg(T.timer),
        ));
    }
    if let Some(d) = state.recipe.difficulty {
        let label = match d {
            larder_core::models::Difficulty::Easy => "Easy",
            larder_core::models::Difficulty::Medium => "Medium",
            larder_core::models::Difficulty::Hard => "Hard",
        };
        meta_spans.push(Span::styled(
            format!("[{}]", label),
            Style::default().fg(T.difficulty(Some(d))),
        ));
    }
    if let Some(r) = state.recipe.rating {
        meta_spans.push(Span::styled(
            "★".repeat(r as usize),
            Style::default().fg(T.medium),
        ));
    }
    let servings_label = if state.display_batches != Decimal::ONE
        || state.display_servings != state.recipe.servings
    {
        let total = Decimal::from(state.display_servings) * state.display_batches;
        format!(
            "{} servings × {} batch (base {} servings)",
            format_quantity(&total),
            fmt_batch(state.display_batches),
            state.recipe.servings
        )
    } else {
        format!("{} servings / batch", state.display_servings)
    };
    meta_spans.push(Span::styled(
        servings_label,
        Style::default().fg(T.timer),
    ));

    let mut meta_line = Vec::new();
    for (i, span) in meta_spans.into_iter().enumerate() {
        if i > 0 {
            meta_line.push(Span::styled(" | ", Style::default().fg(T.muted)));
        }
        meta_line.push(span);
    }
    header_lines.push(Line::from(meta_line));

    let allergen_list = state.recipe.allergen_list();
    if !allergen_list.is_empty() {
        let mut allergen_spans = vec![Span::styled(
            "Contains: ",
            Style::default()
                .fg(T.danger)
                .add_modifier(Modifier::BOLD),
        )];
        for (i, a) in allergen_list.iter().enumerate() {
            if i > 0 {
                allergen_spans.push(Span::raw(" "));
            }
            allergen_spans.push(Span::styled(
                format!("[{}]", a.to_uppercase()),
                Style::default()
                    .fg(T.medium)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        header_lines.push(Line::from(allergen_spans));
    }

    let header = Paragraph::new(header_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(T.border))
            .title("Recipe"),
    );
    frame.render_widget(header, chunks[0]);

    if has_tags {
        let mut tag_spans = Vec::new();
        for t in &state.tags {
            tag_spans.push(Span::styled(
                format!(" #{} ", t.name),
                Style::default().fg(super::theme::Theme::dept(&t.name)),
            ));
        }
        let tag_line = Line::from(tag_spans);
        let tags_widget = Paragraph::new(tag_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(T.border))
                .title("Tags"),
        );
        frame.render_widget(tags_widget, chunks[1]);
    }

    let mut body_lines: Vec<Line> = vec![Line::from(vec![Span::styled(
        "Ingredients",
        Style::default()
            .fg(T.medium)
            .add_modifier(Modifier::BOLD),
    )])];

    for i in &state.ingredients {
        let mut spans = vec![
            Span::raw("  "),
            Span::styled(
                format!("- {}", state.scaled_ingredient_display(i)),
                Style::default().fg(T.text),
            ),
        ];
        if let Some(cost) = state.ingredient_cost_label(i) {
            spans.push(Span::styled(
                format!("  ({})", cost),
                Style::default().fg(T.money),
            ));
        }
        body_lines.push(Line::from(spans));
    }

    let total_cost = state.total_cost();
    if total_cost > Decimal::ZERO {
        body_lines.push(Line::from(""));
        let mut cost_line = vec![Span::styled(
            format!("Est. food cost: {}", format_money(total_cost)),
            Style::default()
                .fg(T.money)
                .add_modifier(Modifier::BOLD),
        )];
        if let Some(menu) = state.recipe.menu_price {
            if let Some(pct) = food_cost_percent(total_cost, menu) {
                cost_line.push(Span::styled(
                    format!("  |  {:.1}% of menu {}", pct, format_money(menu)),
                    Style::default().fg(T.timer),
                ));
            }
        }
        body_lines.push(Line::from(cost_line));
    }

    body_lines.push(Line::from(""));
    body_lines.push(Line::from(vec![Span::styled(
        "Steps",
        Style::default()
            .fg(T.medium)
            .add_modifier(Modifier::BOLD),
    )]));

    for s in &state.steps {
        body_lines.push(Line::from(vec![Span::styled(
            format!("Step {}: ", s.position),
            Style::default()
                .fg(T.timer)
                .add_modifier(Modifier::BOLD),
        )]));
        body_lines.push(Line::from(Span::raw(&s.instruction)));
        if let Some(timer) = s.timer_seconds {
            let min = timer / 60;
            let sec = timer % 60;
            body_lines.push(Line::from(vec![Span::styled(
                format!("  ⏱ {}:{:02}", min, sec),
                Style::default().fg(T.timer),
            )]));
        }
    }

    let body = Paragraph::new(body_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(T.border)),
        )
        .wrap(ratatui::widgets::Wrap { trim: false })
        .scroll((state.scroll, 0));
    frame.render_widget(body, chunks[2]);

    let mut footer = if state.batch_locked {
        "Esc/b: back | j/k·PgUp/PgDn: scroll | +/-: servings | batches fixed (sandwich) | c: cook | e: edit"
            .to_string()
    } else {
        "Esc/b: back | j/k·PgUp/PgDn: scroll | +/-: servings | [/]: batches (¼–n) | c: cook | e: edit"
            .to_string()
    };
    if !status.is_empty() {
        footer = format!("{} | {}", status, footer);
    }
    let footer = Paragraph::new(footer).style(Style::default().fg(T.muted));
    frame.render_widget(footer, chunks[3]);
}

fn render_cooking_mode(frame: &mut Frame, area: Rect, state: &RecipeDetailState, status: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

    let total = state.steps.len();
    let current = state.current_step + 1;
    let counter = Paragraph::new(format!("Step {} of {}", current, total.max(1)))
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title("Cooking Mode"));
    frame.render_widget(counter, chunks[0]);

    if let Some(step) = state.steps.get(state.current_step) {
        let mut lines = vec![
            Line::from(vec![Span::styled(
                format!("Step {}", step.position),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(Span::styled(
                &step.instruction,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
        ];

        if let Some(remaining) = state.timer_remaining {
            let min = remaining / 60;
            let sec = remaining % 60;
            let timer_color = if remaining == 0 {
                Color::Red
            } else if state.timer_running {
                Color::Green
            } else {
                Color::Cyan
            };
            let status = if remaining == 0 {
                "DONE!"
            } else if state.timer_running {
                "RUNNING"
            } else {
                "PAUSED"
            };
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                format!("TIMER {}: {}:{:02}", status, min, sec),
                Style::default()
                    .fg(timer_color)
                    .add_modifier(Modifier::BOLD),
            )]));
        }

        let step_widget = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(step_widget, chunks[1]);
    }

    let mut footer_text = "j/k: steps | Space: timer | Esc: exit cook mode | ?: help".to_string();
    if !status.is_empty() {
        footer_text = format!("{} | {}", status, footer_text);
    }
    let footer = Paragraph::new(footer_text)
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: i64, s: u32) -> Decimal {
        Decimal::new(n, s)
    }

    #[test]
    fn steps_down_through_fractions() {
        assert_eq!(step_batch_down(Decimal::ONE), q(75, 2));
        assert_eq!(step_batch_down(q(75, 2)), q(5, 1));
        assert_eq!(step_batch_down(q(5, 1)), q(25, 2));
        assert_eq!(step_batch_down(q(25, 2)), q(25, 2)); // floor at ¼
    }

    #[test]
    fn steps_up_through_fractions_then_whole() {
        assert_eq!(step_batch_up(q(25, 2)), q(5, 1));
        assert_eq!(step_batch_up(q(5, 1)), q(75, 2));
        assert_eq!(step_batch_up(q(75, 2)), Decimal::ONE);
        assert_eq!(step_batch_up(Decimal::ONE), Decimal::from(2));
    }

    #[test]
    fn steps_whole_numbers_above_one() {
        assert_eq!(step_batch_up(Decimal::from(3)), Decimal::from(4));
        assert_eq!(step_batch_down(Decimal::from(3)), Decimal::from(2));
    }

    #[test]
    fn formats_fractions() {
        assert_eq!(fmt_batch(q(25, 2)), "¼");
        assert_eq!(fmt_batch(q(5, 1)), "½");
        assert_eq!(fmt_batch(q(75, 2)), "¾");
        assert_eq!(fmt_batch(Decimal::from(3)), "3");
    }
}
