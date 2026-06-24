use larder_core::models::Recipe;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use uuid::Uuid;

pub struct RecipeListState {
    recipes: Vec<Recipe>,
    filtered: Vec<Recipe>,
    list_state: ListState,
    search_active: bool,
    search_query: String,
    pick_mode: bool,
}

impl RecipeListState {
    pub fn new() -> Self {
        Self {
            recipes: Vec::new(),
            filtered: Vec::new(),
            list_state: ListState::default(),
            search_active: false,
            search_query: String::new(),
            pick_mode: false,
        }
    }

    pub fn set_pick_mode(&mut self, pick: bool) {
        self.pick_mode = pick;
    }

    pub fn pick_mode(&self) -> bool {
        self.pick_mode
    }

    pub fn set_recipes(&mut self, recipes: Vec<Recipe>) {
        self.recipes = recipes;
        self.apply_filter();
    }

    pub fn selected_id(&self) -> Option<Uuid> {
        self.list_state
            .selected()
            .and_then(|i| self.filtered.get(i).map(|r| r.id))
    }

    pub fn select_next(&mut self) {
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % len));
    }

    pub fn select_previous(&mut self) {
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + len - 1) % len));
    }

    pub fn toggle_search(&mut self) {
        self.search_active = !self.search_active;
        if !self.search_active {
            self.search_query.clear();
            self.apply_filter();
        }
    }

    pub fn push_search(&mut self, c: char) {
        self.search_query.push(c);
        self.apply_filter();
    }

    pub fn pop_search(&mut self) {
        self.search_query.pop();
        self.apply_filter();
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.apply_filter();
        self.search_active = false;
    }

    pub fn search_active(&self) -> bool {
        self.search_active
    }

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered = self.recipes.clone();
        } else {
            let q = self.search_query.to_lowercase();
            self.filtered = self
                .recipes
                .iter()
                .filter(|r| {
                    r.name.to_lowercase().contains(&q)
                        || r.description
                            .as_ref()
                            .map(|d| d.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .cloned()
                .collect();
        }
        if self.list_state.selected().unwrap_or(0) >= self.filtered.len() {
            self.list_state.select(self.filtered.first().map(|_| 0));
        }
    }
}

pub fn render(frame: &mut Frame, state: &mut RecipeListState, status: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let title = if state.pick_mode() {
        "Pick a recipe for meal plan".to_string()
    } else if state.search_active {
        format!("Recipes [search: {}▌]", state.search_query())
    } else {
        format!("Recipes ({})", state.filtered.len())
    };

    let header = Paragraph::new(title.as_str())
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title("Recipes"));
    frame.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .map(|r| {
            let time = r
                .total_time()
                .map(|t| format!("{}m", t))
                .unwrap_or_else(|| "?".to_string());
            let difficulty = r
                .difficulty
                .map(|d| match d {
                    larder_core::models::Difficulty::Easy => "E",
                    larder_core::models::Difficulty::Medium => "M",
                    larder_core::models::Difficulty::Hard => "H",
                })
                .unwrap_or("?");
            let rating = r.rating.map(|r| "★".repeat(r as usize)).unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[{}] ", difficulty),
                    Style::default().fg(match r.difficulty {
                        Some(larder_core::models::Difficulty::Easy) => Color::Green,
                        Some(larder_core::models::Difficulty::Medium) => Color::Yellow,
                        Some(larder_core::models::Difficulty::Hard) => Color::Red,
                        None => Color::DarkGray,
                    }),
                ),
                Span::raw(&r.name),
                Span::styled(
                    format!(" ({}) ", time),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(rating, Style::default().fg(Color::Yellow)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Recipes"))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, chunks[1], &mut state.list_state);

    let help = if state.pick_mode() {
        "Esc: cancel | Enter: assign to meal slot"
    } else if state.search_active {
        "Esc: cancel search | Type to search"
    } else {
        "q: back | j/k or ↑↓: navigate | Enter: view | /: search | n: new recipe"
    };
    let mut footer_text = help.to_string();
    if !status.is_empty() {
        footer_text = format!("{} | {}", status, footer_text);
    }
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
