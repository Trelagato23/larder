use larder_core::{
    models::{Recipe, RecipeIngredient, RecipeStep, Tag},
    services::scaling::scale_display_text,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::time::Instant;

pub struct RecipeDetailState {
    recipe: Recipe,
    ingredients: Vec<RecipeIngredient>,
    steps: Vec<RecipeStep>,
    tags: Vec<Tag>,
    scroll: u16,
    cooking_mode: bool,
    current_step: usize,
    display_servings: u32,
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
        Self {
            recipe,
            ingredients,
            steps,
            tags,
            scroll: 0,
            cooking_mode: false,
            current_step: 0,
            display_servings,
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

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
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
        if self.display_servings == self.recipe.servings {
            ingredient.display.clone()
        } else {
            scale_display_text(
                &ingredient.display,
                self.recipe.servings,
                self.display_servings,
            )
        }
    }
}

pub fn render(frame: &mut Frame, state: &RecipeDetailState) {
    if state.cooking_mode {
        render_cooking_mode(frame, state);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let mut header_lines = vec![Line::from(vec![Span::styled(
        &state.recipe.name,
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )])];

    if let Some(ref desc) = state.recipe.description {
        header_lines.push(Line::from(vec![Span::styled(
            desc,
            Style::default().fg(Color::DarkGray),
        )]));
    }

    let mut meta_spans: Vec<Span> = Vec::new();
    if let Some(t) = state.recipe.total_time() {
        meta_spans.push(Span::styled(
            format!("{} min", t),
            Style::default().fg(Color::Cyan),
        ));
    }
    if let Some(d) = &state.recipe.difficulty {
        let (color, label) = match d {
            larder_core::models::Difficulty::Easy => (Color::Green, "Easy"),
            larder_core::models::Difficulty::Medium => (Color::Yellow, "Medium"),
            larder_core::models::Difficulty::Hard => (Color::Red, "Hard"),
        };
        meta_spans.push(Span::styled(
            format!("[{}]", label),
            Style::default().fg(color),
        ));
    }
    if let Some(r) = state.recipe.rating {
        meta_spans.push(Span::styled(
            "★".repeat(r as usize),
            Style::default().fg(Color::Yellow),
        ));
    }
    let servings_label = if state.display_servings != state.recipe.servings {
        format!(
            "{} servings (scaled from {})",
            state.display_servings, state.recipe.servings
        )
    } else {
        format!("{} servings", state.display_servings)
    };
    meta_spans.push(Span::styled(
        servings_label,
        Style::default().fg(Color::Cyan),
    ));

    let mut meta_line = Vec::new();
    for (i, span) in meta_spans.into_iter().enumerate() {
        if i > 0 {
            meta_line.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        }
        meta_line.push(span);
    }
    header_lines.push(Line::from(meta_line));

    let header =
        Paragraph::new(header_lines).block(Block::default().borders(Borders::ALL).title("Recipe"));
    frame.render_widget(header, chunks[0]);

    if !state.tags.is_empty() {
        let mut tag_spans = Vec::new();
        for t in &state.tags {
            tag_spans.push(Span::styled(
                format!(" #{} ", t.name),
                Style::default().fg(Color::Magenta),
            ));
        }
        let tag_line = Line::from(tag_spans);
        let tags_widget =
            Paragraph::new(tag_line).block(Block::default().borders(Borders::ALL).title("Tags"));
        frame.render_widget(tags_widget, chunks[1]);
    }

    let ingredient_lines: Vec<Line> = state
        .ingredients
        .iter()
        .map(|i| {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("- {}", state.scaled_ingredient_display(i)),
                    Style::default().fg(Color::White),
                ),
            ])
        })
        .collect();

    let ingredients = Paragraph::new(ingredient_lines)
        .block(Block::default().borders(Borders::ALL).title("Ingredients"));
    frame.render_widget(ingredients, chunks[2]);

    let step_lines: Vec<Line> = state
        .steps
        .iter()
        .flat_map(|s| {
            let mut lines = vec![Line::from(vec![Span::styled(
                format!("Step {}: ", s.position),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )])];
            lines.push(Line::from(Span::raw(&s.instruction)));
            if let Some(timer) = s.timer_seconds {
                let min = timer / 60;
                let sec = timer % 60;
                lines.push(Line::from(vec![Span::styled(
                    format!("  [timer: {}:{:02}]", min, sec),
                    Style::default().fg(Color::Cyan),
                )]));
            }
            lines.push(Line::from(""));
            lines
        })
        .collect();

    let steps =
        Paragraph::new(step_lines).block(Block::default().borders(Borders::ALL).title("Steps"));
    frame.render_widget(steps, chunks[3]);

    let footer = Paragraph::new(
        "q: back | c: cook | e: edit | d: delete | +/-: scale | g: shopping list | j/k: scroll",
    )
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[4]);
}

fn render_cooking_mode(frame: &mut Frame, state: &RecipeDetailState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

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

        let step_widget = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
        frame.render_widget(step_widget, chunks[1]);
    }

    let footer = Paragraph::new(
        "j/→: next | k/←: prev | Space: timer | Esc/q: exit cooking mode",
    )
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
