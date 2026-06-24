use chrono::{Datelike, Duration, NaiveDate};
use larder_core::models::{MealPlan, MealType};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use std::collections::HashMap;
use uuid::Uuid;

const SLOT_TYPES: [MealType; 4] = [
    MealType::Breakfast,
    MealType::Lunch,
    MealType::Dinner,
    MealType::Snack,
];

pub struct MealPlanState {
    pub week_start: NaiveDate,
    meals: Vec<MealPlan>,
    recipe_names: HashMap<Uuid, String>,
    selected_day: usize,
    selected_slot: usize,
}

impl MealPlanState {
    pub fn new() -> Self {
        let today = chrono::Local::now().date_naive();
        let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        Self {
            week_start: monday,
            meals: Vec::new(),
            recipe_names: HashMap::new(),
            selected_day: 0,
            selected_slot: 0,
        }
    }

    pub fn set_meals(&mut self, meals: Vec<MealPlan>, recipe_names: HashMap<Uuid, String>) {
        self.meals = meals;
        self.recipe_names = recipe_names;
    }

    pub fn current_date(&self) -> NaiveDate {
        self.week_start + Duration::days(self.selected_day as i64)
    }

    pub fn current_meal_type(&self) -> MealType {
        SLOT_TYPES[self.selected_slot.min(SLOT_TYPES.len() - 1)]
    }

    pub fn meal_for_slot(&self, day_index: usize, slot: usize) -> Option<&MealPlan> {
        let date = self.week_start + Duration::days(day_index as i64);
        let meal_type = SLOT_TYPES[slot.min(SLOT_TYPES.len() - 1)];
        self.meals
            .iter()
            .find(|m| m.date == date && m.meal_type == meal_type)
    }

    pub fn navigate_next_day(&mut self) {
        self.selected_day = (self.selected_day + 1) % 7;
        self.selected_slot = 0;
    }

    pub fn navigate_prev_day(&mut self) {
        self.selected_day = (self.selected_day + 6) % 7;
        self.selected_slot = 0;
    }

    pub fn navigate_next_slot(&mut self) {
        self.selected_slot = (self.selected_slot + 1) % SLOT_TYPES.len();
    }

    pub fn navigate_prev_slot(&mut self) {
        self.selected_slot = (self.selected_slot + SLOT_TYPES.len() - 1) % SLOT_TYPES.len();
    }

    pub fn week_forward(&mut self) {
        self.week_start += Duration::days(7);
    }

    pub fn week_backward(&mut self) {
        self.week_start -= Duration::days(7);
    }

    fn label_for_slot(&self, day_index: usize, slot: usize) -> String {
        match self.meal_for_slot(day_index, slot) {
            Some(meal) => match (&meal.title, meal.recipe_id) {
                (Some(title), _) => title.clone(),
                (None, Some(id)) => self
                    .recipe_names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| "(recipe)".to_string()),
                (None, None) => "(empty)".to_string(),
            },
            None => "(empty)".to_string(),
        }
    }
}

pub fn render(frame: &mut Frame, state: &mut MealPlanState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let week_end = state.week_start + Duration::days(6);
    let header = Paragraph::new(format!(
        "Week of {} - {}",
        state.week_start.format("%b %d"),
        week_end.format("%b %d, %Y")
    ))
    .style(
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL).title("Meal Plan"));
    frame.render_widget(header, chunks[0]);

    let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let dates: Vec<NaiveDate> = (0..7)
        .map(|i| state.week_start + Duration::days(i as i64))
        .collect();
    let mut tab_spans = Vec::new();
    for (i, (name, date)) in day_names.iter().zip(dates.iter()).enumerate() {
        let style = if i == state.selected_day {
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        tab_spans.push(Span::styled(format!(" {} {} ", name, date.day()), style));
        if i < 6 {
            tab_spans.push(Span::raw(" | "));
        }
    }
    let tabs = Paragraph::new(Line::from(tab_spans))
        .block(Block::default().borders(Borders::ALL).title("Days"));
    frame.render_widget(tabs, chunks[1]);

    let meal_rows: Vec<Row> = SLOT_TYPES
        .iter()
        .enumerate()
        .map(|(i, meal_type)| {
            let recipe_name = state.label_for_slot(state.selected_day, i);
            let meal_type_color = match meal_type {
                MealType::Breakfast => Color::Yellow,
                MealType::Lunch => Color::Green,
                MealType::Dinner => Color::Cyan,
                MealType::Snack => Color::Magenta,
            };
            let style = if i == state.selected_slot {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![
                Span::styled(
                    format!("{:>10}", meal_type.to_string()),
                    Style::default().fg(meal_type_color),
                ),
                Span::raw(recipe_name),
            ])
            .style(style)
        })
        .collect();

    let selected_date = state.current_date();
    let meals_table = Table::new(meal_rows, [Constraint::Length(12), Constraint::Min(1)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Meals — {}", selected_date.format("%A %b %d"))),
        );
    frame.render_widget(meals_table, chunks[2]);

    let footer = Paragraph::new(
        "q: back | ←/→: day | ↑/↓: meal | [/]: week | a: assign recipe | d: clear | g: shopping list",
    )
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[3]);
}
