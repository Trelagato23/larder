use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);

    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(area);

    let h = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(10), Constraint::Percentage(80), Constraint::Percentage(10)])
        .split(popup[1]);

    let box_area = h[1];

    let lines = vec![
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("  1 / r     Recipes list"),
        Line::from("  2 / i     Import from URL"),
        Line::from("  3 / m     Meal plan"),
        Line::from("  4 / s     Shopping list"),
        Line::from("  j / k     Move selection up/down"),
        Line::from("  Enter     Open / confirm"),
        Line::from("  Esc / b   Go back"),
        Line::from("  ?         This help"),
        Line::from("  q         Quit"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Recipes",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("  /         Search"),
        Line::from("  n         New sample recipe"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Recipe view",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("  c         Cooking mode"),
        Line::from("  e         Edit"),
        Line::from("  d         Delete"),
        Line::from("  + / -     Scale servings"),
        Line::from("  g         Add ingredients to shopping list"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Meal plan",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("  h / l     Previous / next day"),
        Line::from("  j / k     Previous / next meal slot"),
        Line::from("  [ / ]     Previous / next week"),
        Line::from("  a         Assign recipe to slot"),
        Line::from("  d         Clear slot"),
        Line::from("  g         Build shopping list from plan"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Editor",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("  1 / 2 / 3   Meta / ingredients / steps panels"),
        Line::from("  j / k       Select line in list"),
        Line::from("  Enter       Save recipe (meta) or edit line"),
        Line::from("  a           Add ingredient or step"),
        Line::from("  d           Delete selected line"),
        Line::from("  t           Set step timer (minutes)"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Shopping",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("  Space/c   Check / uncheck item"),
        Line::from("  a         Add item manually"),
        Line::from("  v         Toggle show checked items"),
        Line::from("  x         Remove checked items"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press Esc or ? to close",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, box_area);
}
