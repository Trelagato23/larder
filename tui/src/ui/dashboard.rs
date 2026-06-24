use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

pub struct DashboardState {
    selected: usize,
    list_state: ListState,
}

impl DashboardState {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            selected: 0,
            list_state,
        }
    }

    pub fn menu_items(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Recipes", "Browse and manage your recipes"),
            ("Meal Plan", "Plan your weekly meals"),
            ("Shopping List", "Auto-generated from recipes"),
            ("Cookbooks", "Organize recipe collections"),
            ("Search", "Find recipes by name or ingredient"),
            ("Statistics", "View your cooking stats"),
        ]
    }

    pub fn select_next(&mut self) {
        let len = self.menu_items().len();
        self.selected = (self.selected + 1) % len;
        self.list_state.select(Some(self.selected));
    }

    pub fn select_previous(&mut self) {
        let len = self.menu_items().len();
        self.selected = (self.selected + len - 1) % len;
        self.list_state.select(Some(self.selected));
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }
}

pub fn render(frame: &mut Frame, state: &DashboardState) {
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
        .block(Block::default().borders(Borders::ALL).title("Menu"));
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
    frame.render_stateful_widget(list, chunks[1], &mut state.list_state.clone());

    let footer = Paragraph::new("q: quit | j/k or ↑↓: navigate | Enter: select")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
