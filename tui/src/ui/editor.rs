use larder_core::models::Recipe;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum EditField {
    Name,
    Description,
    Servings,
    PrepTime,
    CookTime,
    Rating,
}

pub struct EditorState {
    recipe: Recipe,
    field: EditField,
    name: String,
    description: String,
    servings: String,
    prep_time: String,
    cook_time: String,
    rating: String,
    status: String,
}

impl EditorState {
    pub fn new(recipe: Recipe) -> Self {
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
            field: EditField::Name,
            status: String::new(),
        }
    }

    pub fn recipe_id(&self) -> uuid::Uuid {
        self.recipe.id
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

    pub fn push_char(&mut self, c: char) {
        match self.field {
            EditField::Name => self.name.push(c),
            EditField::Description => self.description.push(c),
            EditField::Servings => self.servings.push(c),
            EditField::PrepTime => self.prep_time.push(c),
            EditField::CookTime => self.cook_time.push(c),
            EditField::Rating => self.rating.push(c),
        }
    }

    pub fn backspace(&mut self) {
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

pub fn render(frame: &mut Frame, state: &EditorState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let header = Paragraph::new("Edit Recipe")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title("Editor"));
    frame.render_widget(header, chunks[0]);

    let field_style = |field: EditField| {
        if field == state.field {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    };

    let cursor = if state.field == EditField::Name
        || state.field == EditField::Description
        || state.field == EditField::Servings
        || state.field == EditField::PrepTime
        || state.field == EditField::CookTime
    {
        "▌"
    } else {
        ""
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Name: ", field_style(EditField::Name)),
            Span::styled(
                format!(
                    "{}{}",
                    state.name,
                    if state.field == EditField::Name {
                        cursor
                    } else {
                        ""
                    }
                ),
                field_style(EditField::Name),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Description: ", field_style(EditField::Description)),
            Span::styled(
                format!(
                    "{}{}",
                    state.description,
                    if state.field == EditField::Description {
                        cursor
                    } else {
                        ""
                    }
                ),
                field_style(EditField::Description),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Servings: ", field_style(EditField::Servings)),
            Span::styled(
                format!(
                    "{}{}",
                    state.servings,
                    if state.field == EditField::Servings {
                        cursor
                    } else {
                        ""
                    }
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
                    if state.field == EditField::PrepTime {
                        cursor
                    } else {
                        ""
                    }
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
                    if state.field == EditField::CookTime {
                        cursor
                    } else {
                        ""
                    }
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
                    if state.field == EditField::Rating {
                        cursor
                    } else {
                        ""
                    }
                ),
                field_style(EditField::Rating),
            ),
        ]),
    ];

    if let Some(diff) = state.recipe.difficulty {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!("Difficulty: {:?}", diff),
            Style::default().fg(Color::DarkGray),
        )]));
    }

    if !state.status.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            &state.status,
            Style::default().fg(Color::Red),
        )]));
    }

    let form = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
    frame.render_widget(form, chunks[1]);

    let footer = Paragraph::new(
        "Tab/↓: next field | Shift+Tab/↑: prev | Enter: save | Esc: cancel",
    )
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
