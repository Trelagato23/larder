use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn render(frame: &mut Frame, area: Rect, active_tab: u8) {
    let tabs: [(u8, &str); 4] = [
        (1, "1 Recipes"),
        (2, "2 Import"),
        (3, "3 Plan"),
        (4, "4 Shop"),
    ];

    let mut spans = vec![Span::raw("  ")];
    for (id, label) in tabs {
        let style = if active_tab == id {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!(" {} ", label), style));
    }
    spans.push(Span::styled(
        "  |  1-4: jump  |  ?: help  |  q: quit",
        Style::default().fg(Color::DarkGray),
    ));

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Black));
    frame.render_widget(bar, area);
}
