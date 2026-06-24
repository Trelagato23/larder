use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub mod editor;
pub mod help;
pub mod import;
pub mod meal_plan;
pub mod recipe_detail;
pub mod recipe_list;
pub mod shopping_list;
pub mod status_bar;

pub fn content_and_nav(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    (chunks[0], chunks[1])
}
