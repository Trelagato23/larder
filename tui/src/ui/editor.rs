use larder_core::models::{Recipe, RecipeIngredient, RecipeStep};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditField {
    Name,
    Description,
    Servings,
    PrepTime,
    CookTime,
    Rating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorPanel {
    Meta,
    Ingredients,
    Steps,
}

#[derive(Debug, Clone)]
struct StepDraft {
    instruction: String,
    timer_minutes: String,
}

pub struct EditorState {
    recipe: Recipe,
    panel: EditorPanel,
    field: EditField,
    name: String,
    description: String,
    servings: String,
    prep_time: String,
    cook_time: String,
    rating: String,
    ingredients: Vec<String>,
    steps: Vec<StepDraft>,
    list_state: ListState,
    editing_line: bool,
    line_buffer: String,
    status: String,
}

impl EditorState {
    pub fn new(
        recipe: Recipe,
        ingredients: Vec<RecipeIngredient>,
        steps: Vec<RecipeStep>,
    ) -> Self {
        let ingredient_lines: Vec<String> = ingredients.into_iter().map(|i| i.display).collect();
        let step_lines: Vec<StepDraft> = steps
            .into_iter()
            .map(|s| StepDraft {
                instruction: s.instruction,
                timer_minutes: s
                    .timer_seconds
                    .map(|t| (t / 60).to_string())
                    .unwrap_or_default(),
            })
            .collect();

        Self {
            name: recipe.name.clone(),
            description: recipe.description.clone().unwrap_or_default(),
            servings: recipe.servings.to_string(),
            prep_time: recipe
                .prep_time_minutes
                .map(|v| v.to_string())
                .unwrap_or_default(),
            cook_time: recipe
                .cook_time_minutes
                .map(|v| v.to_string())
                .unwrap_or_default(),
            rating: recipe.rating.map(|v| v.to_string()).unwrap_or_default(),
            recipe,
            panel: EditorPanel::Meta,
            field: EditField::Name,
            ingredients: ingredient_lines,
            steps: step_lines,
            list_state: ListState::default().with_selected(Some(0)),
            editing_line: false,
            line_buffer: String::new(),
            status: String::new(),
        }
    }

    pub fn recipe_id(&self) -> uuid::Uuid {
        self.recipe.id
    }

    pub fn panel(&self) -> EditorPanel {
        self.panel
    }

    pub fn in_line_edit(&self) -> bool {
        self.editing_line
    }

    pub fn in_meta_field_edit(&self) -> bool {
        self.panel == EditorPanel::Meta && !self.editing_line
    }

    pub fn set_panel(&mut self, panel: EditorPanel) {
        self.panel = panel;
        self.editing_line = false;
        self.line_buffer.clear();
        self.status.clear();
        if panel != EditorPanel::Meta {
            self.list_state.select(Some(0));
        }
    }

    pub fn build_recipe(&self) -> Result<Recipe, String> {
        let servings: u32 = self
            .servings
            .parse()
            .map_err(|_| "Servings must be a positive number".to_string())?;
        if servings == 0 {
            return Err("Servings must be at least 1".to_string());
        }

        let prep_time_minutes = if self.prep_time.trim().is_empty() {
            None
        } else {
            Some(
                self.prep_time
                    .parse()
                    .map_err(|_| "Prep time must be minutes".to_string())?,
            )
        };

        let cook_time_minutes = if self.cook_time.trim().is_empty() {
            None
        } else {
            Some(
                self.cook_time
                    .parse()
                    .map_err(|_| "Cook time must be minutes".to_string())?,
            )
        };

        let total_time_minutes = match (prep_time_minutes, cook_time_minutes) {
            (Some(p), Some(c)) => Some(p + c),
            (Some(p), None) => Some(p),
            (None, Some(c)) => Some(c),
            (None, None) => None,
        };

        let rating = if self.rating.trim().is_empty() {
            None
        } else {
            let value: u8 = self
                .rating
                .parse()
                .map_err(|_| "Rating must be 1-5".to_string())?;
            if !(1..=5).contains(&value) {
                return Err("Rating must be 1-5".to_string());
            }
            Some(value)
        };

        Ok(Recipe {
            id: self.recipe.id,
            name: self.name.trim().to_string(),
            description: if self.description.trim().is_empty() {
                None
            } else {
                Some(self.description.trim().to_string())
            },
            image_url: self.recipe.image_url.clone(),
            servings,
            prep_time_minutes,
            cook_time_minutes,
            total_time_minutes,
            source_url: self.recipe.source_url.clone(),
            created_at: self.recipe.created_at,
            updated_at: chrono::Utc::now(),
            user_id: self.recipe.user_id,
            rating,
            difficulty: self.recipe.difficulty,
        })
    }

    pub fn build_ingredients(&self, recipe_id: uuid::Uuid) -> Vec<RecipeIngredient> {
        self.ingredients
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let display = line.trim().to_string();
                RecipeIngredient {
                    id: uuid::Uuid::new_v4(),
                    recipe_id,
                    ingredient: display.clone(),
                    quantity: None,
                    unit: None,
                    note: None,
                    display,
                    category: None,
                }
            })
            .collect()
    }

    pub fn build_steps(&self, recipe_id: uuid::Uuid) -> Result<Vec<RecipeStep>, String> {
        let mut out = Vec::new();
        for (idx, step) in self
            .steps
            .iter()
            .filter(|s| !s.instruction.trim().is_empty())
            .enumerate()
        {
            let timer_seconds = if step.timer_minutes.trim().is_empty() {
                None
            } else {
                let mins: u32 = step
                    .timer_minutes
                    .parse()
                    .map_err(|_| "Step timer must be minutes".to_string())?;
                Some(mins * 60)
            };
            out.push(RecipeStep {
                id: uuid::Uuid::new_v4(),
                recipe_id,
                position: (idx + 1) as u32,
                instruction: step.instruction.trim().to_string(),
                timer_seconds,
            });
        }
        Ok(out)
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
    }

    pub fn next_field(&mut self) {
        self.field = match self.field {
            EditField::Name => EditField::Description,
            EditField::Description => EditField::Servings,
            EditField::Servings => EditField::PrepTime,
            EditField::PrepTime => EditField::CookTime,
            EditField::CookTime => EditField::Rating,
            EditField::Rating => EditField::Name,
        };
    }

    pub fn prev_field(&mut self) {
        self.field = match self.field {
            EditField::Name => EditField::Rating,
            EditField::Description => EditField::Name,
            EditField::Servings => EditField::Description,
            EditField::PrepTime => EditField::Servings,
            EditField::CookTime => EditField::PrepTime,
            EditField::Rating => EditField::CookTime,
        };
    }

    pub fn select_next(&mut self) {
        let len = self.list_len();
        if len == 0 {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % len));
    }

    pub fn select_previous(&mut self) {
        let len = self.list_len();
        if len == 0 {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + len - 1) % len));
    }

    fn list_len(&self) -> usize {
        match self.panel {
            EditorPanel::Ingredients => self.ingredients.len().max(1),
            EditorPanel::Steps => self.steps.len().max(1),
            EditorPanel::Meta => 0,
        }
    }

    pub fn start_edit_line(&mut self) {
        if self.panel == EditorPanel::Meta {
            return;
        }
        self.editing_line = true;
        self.status.clear();
        self.line_buffer = match self.panel {
            EditorPanel::Ingredients => self
                .ingredients
                .get(self.list_state.selected().unwrap_or(0))
                .cloned()
                .unwrap_or_default(),
            EditorPanel::Steps => self
                .steps
                .get(self.list_state.selected().unwrap_or(0))
                .map(|s| s.instruction.clone())
                .unwrap_or_default(),
            EditorPanel::Meta => String::new(),
        };
    }

    pub fn commit_line(&mut self) {
        if !self.editing_line {
            return;
        }
        if self.status == "Timer (minutes):" {
            self.commit_timer();
            return;
        }
        let idx = self.list_state.selected().unwrap_or(0);
        match self.panel {
            EditorPanel::Ingredients => {
                if idx >= self.ingredients.len() {
                    self.ingredients.push(self.line_buffer.trim().to_string());
                } else {
                    self.ingredients[idx] = self.line_buffer.trim().to_string();
                }
            }
            EditorPanel::Steps => {
                if idx >= self.steps.len() {
                    self.steps.push(StepDraft {
                        instruction: self.line_buffer.trim().to_string(),
                        timer_minutes: String::new(),
                    });
                } else {
                    self.steps[idx].instruction = self.line_buffer.trim().to_string();
                }
            }
            EditorPanel::Meta => {}
        }
        self.editing_line = false;
        self.line_buffer.clear();
    }

    pub fn cancel_edit_line(&mut self) {
        self.editing_line = false;
        self.line_buffer.clear();
        self.status.clear();
    }

    pub fn add_line(&mut self) {
        match self.panel {
            EditorPanel::Ingredients => {
                self.ingredients.push(String::new());
                self.list_state.select(Some(self.ingredients.len() - 1));
            }
            EditorPanel::Steps => {
                self.steps.push(StepDraft {
                    instruction: String::new(),
                    timer_minutes: String::new(),
                });
                self.list_state.select(Some(self.steps.len() - 1));
            }
            EditorPanel::Meta => {}
        }
        self.start_edit_line();
    }

    pub fn delete_selected(&mut self) {
        let idx = self.list_state.selected().unwrap_or(0);
        match self.panel {
            EditorPanel::Ingredients if idx < self.ingredients.len() => {
                self.ingredients.remove(idx);
            }
            EditorPanel::Steps if idx < self.steps.len() => {
                self.steps.remove(idx);
            }
            _ => {}
        }
        if self.list_len() > 0 {
            self.list_state.select(Some(idx.min(self.list_len() - 1)));
        }
    }

    pub fn edit_step_timer(&mut self) {
        if self.panel != EditorPanel::Steps {
            return;
        }
        let idx = self.list_state.selected().unwrap_or(0);
        if let Some(step) = self.steps.get(idx) {
            self.editing_line = true;
            self.line_buffer = step.timer_minutes.clone();
            self.status = "Timer (minutes):".to_string();
        }
    }

    pub fn commit_timer(&mut self) {
        let idx = self.list_state.selected().unwrap_or(0);
        if let Some(step) = self.steps.get_mut(idx) {
            step.timer_minutes = self.line_buffer.trim().to_string();
        }
        self.editing_line = false;
        self.line_buffer.clear();
        self.status.clear();
    }

    pub fn push_char(&mut self, c: char) {
        if self.editing_line {
            self.line_buffer.push(c);
            return;
        }
        if self.panel != EditorPanel::Meta {
            return;
        }
        match self.field {
            EditField::Name => self.name.push(c),
            EditField::Description => self.description.push(c),
            EditField::Servings => self.servings.push(c),
            EditField::PrepTime => self.prep_time.push(c),
            EditField::CookTime => self.cook_time.push(c),
            EditField::Rating => self.rating.push(c),
        }
    }

    pub fn push_str(&mut self, s: &str) {
        for c in s.chars() {
            self.push_char(c);
        }
    }

    pub fn backspace(&mut self) {
        if self.editing_line {
            self.line_buffer.pop();
            return;
        }
        if self.panel != EditorPanel::Meta {
            return;
        }
        match self.field {
            EditField::Name => {
                self.name.pop();
            }
            EditField::Description => {
                self.description.pop();
            }
            EditField::Servings => {
                self.servings.pop();
            }
            EditField::PrepTime => {
                self.prep_time.pop();
            }
            EditField::CookTime => {
                self.cook_time.pop();
            }
            EditField::Rating => {
                self.rating.pop();
            }
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &mut EditorState, status: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);

    let panel_label = match state.panel {
        EditorPanel::Meta => "1 Meta",
        EditorPanel::Ingredients => "2 Ingredients",
        EditorPanel::Steps => "3 Steps",
    };

    let header = Paragraph::new(format!("Edit Recipe — {}", panel_label))
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title("Editor"));
    frame.render_widget(header, chunks[0]);

    match state.panel {
        EditorPanel::Meta => render_meta(frame, chunks[1], state),
        EditorPanel::Ingredients => render_ingredient_list(frame, chunks[1], state),
        EditorPanel::Steps => render_step_list(frame, chunks[1], state),
    }

    let mut footer = match state.panel {
        EditorPanel::Meta => "1/2/3: panel | Tab: field | Enter: save recipe | Esc: cancel",
        EditorPanel::Ingredients => "j/k: select | Enter: edit | a: add | d: delete | 1/2/3: panel",
        EditorPanel::Steps => "j/k: select | Enter: edit | t: timer | a: add | d: delete | 1/2/3: panel",
    };
    if state.editing_line {
        footer = "Enter: save line | Esc: cancel edit";
    }
    let mut text = footer.to_string();
    if !state.status.is_empty() && state.status != "Timer (minutes):" {
        text = format!("{} | {}", state.status, text);
    } else if !status.is_empty() {
        text = format!("{} | {}", status, text);
    }
    let footer = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}

fn render_meta(frame: &mut Frame, area: Rect, state: &EditorState) {
    let field_style = |field: EditField| {
        if field == state.field {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    };

    let cursor = "▌";
    let lines = vec![
        Line::from(vec![
            Span::styled("Name: ", field_style(EditField::Name)),
            Span::styled(
                format!(
                    "{}{}",
                    state.name,
                    if state.field == EditField::Name { cursor } else { "" }
                ),
                field_style(EditField::Name),
            ),
        ]),
        Line::from(vec![
            Span::styled("Description: ", field_style(EditField::Description)),
            Span::styled(
                format!(
                    "{}{}",
                    state.description,
                    if state.field == EditField::Description { cursor } else { "" }
                ),
                field_style(EditField::Description),
            ),
        ]),
        Line::from(vec![
            Span::styled("Servings: ", field_style(EditField::Servings)),
            Span::styled(
                format!(
                    "{}{}",
                    state.servings,
                    if state.field == EditField::Servings { cursor } else { "" }
                ),
                field_style(EditField::Servings),
            ),
        ]),
        Line::from(vec![
            Span::styled("Prep (min): ", field_style(EditField::PrepTime)),
            Span::styled(
                format!(
                    "{}{}",
                    state.prep_time,
                    if state.field == EditField::PrepTime { cursor } else { "" }
                ),
                field_style(EditField::PrepTime),
            ),
        ]),
        Line::from(vec![
            Span::styled("Cook (min): ", field_style(EditField::CookTime)),
            Span::styled(
                format!(
                    "{}{}",
                    state.cook_time,
                    if state.field == EditField::CookTime { cursor } else { "" }
                ),
                field_style(EditField::CookTime),
            ),
        ]),
        Line::from(vec![
            Span::styled("Rating (1-5): ", field_style(EditField::Rating)),
            Span::styled(
                format!(
                    "{}{}",
                    state.rating,
                    if state.field == EditField::Rating { cursor } else { "" }
                ),
                field_style(EditField::Rating),
            ),
        ]),
    ];

    let form = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
    frame.render_widget(form, area);
}

fn render_ingredient_list(frame: &mut Frame, area: Rect, state: &mut EditorState) {
    if state.editing_line {
        let prompt = Paragraph::new(format!("Ingredient: {}▌", state.line_buffer))
            .block(Block::default().borders(Borders::ALL).title("Edit line"));
        frame.render_widget(prompt, area);
        return;
    }

    let items: Vec<ListItem> = if state.ingredients.is_empty() {
        vec![ListItem::new("  (empty — press a to add)")]
    } else {
        state
            .ingredients
            .iter()
            .map(|line| ListItem::new(format!("  {}", line)))
            .collect()
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Ingredients"))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, area, &mut state.list_state);
}

fn render_step_list(frame: &mut Frame, area: Rect, state: &mut EditorState) {
    if state.editing_line {
        let title = if state.status == "Timer (minutes):" {
            format!("Timer (min): {}▌", state.line_buffer)
        } else {
            format!("Step: {}▌", state.line_buffer)
        };
        let prompt = Paragraph::new(title).block(Block::default().borders(Borders::ALL).title("Edit"));
        frame.render_widget(prompt, area);
        return;
    }

    let items: Vec<ListItem> = if state.steps.is_empty() {
        vec![ListItem::new("  (empty — press a to add)")]
    } else {
        state
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let timer = if s.timer_minutes.is_empty() {
                    String::new()
                } else {
                    format!(" [{}m]", s.timer_minutes)
                };
                ListItem::new(format!(
                    "  {}. {}{}",
                    i + 1,
                    if s.instruction.is_empty() {
                        "(empty)"
                    } else {
                        &s.instruction
                    },
                    timer
                ))
            })
            .collect()
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Steps"))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, area, &mut state.list_state);
}
