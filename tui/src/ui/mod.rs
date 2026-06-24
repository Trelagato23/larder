use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub mod dashboard;
pub mod editor;
pub mod import;
pub mod meal_plan;
pub mod recipe_detail;
pub mod recipe_list;
pub mod shopping_list;

pub use dashboard::DashboardState;

pub fn render_dashboard(frame: &mut Frame, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Paragraph::new("Recipes")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = state
        .menu_items()
        .iter()
        .map(|(label, desc)| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    *label,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" - {}", desc), Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Main Menu"))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_widget(list, chunks[1]);

    let footer = Paragraph::new(
        "q: quit | j/k: navigate | Enter: select | r: recipes | m: meal plan | s: shopping",
    )
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
