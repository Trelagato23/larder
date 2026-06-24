use larder_core::models::ShoppingListItem;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

pub struct ShoppingListState {
    items: Vec<ShoppingListItem>,
    list_state: ListState,
    show_checked: bool,
    grouped: Vec<(String, Vec<ShoppingListItem>)>,
    adding_item: bool,
    new_item_text: String,
}

impl ShoppingListState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            show_checked: false,
            grouped: Vec::new(),
            adding_item: false,
            new_item_text: String::new(),
        }
    }

    pub fn start_add_item(&mut self) {
        self.adding_item = true;
        self.new_item_text.clear();
    }

    pub fn cancel_add_item(&mut self) {
        self.adding_item = false;
        self.new_item_text.clear();
    }

    pub fn adding_item(&self) -> bool {
        self.adding_item
    }

    pub fn new_item_text(&self) -> &str {
        &self.new_item_text
    }

    pub fn push_char(&mut self, c: char) {
        self.new_item_text.push(c);
    }

    pub fn pop_char(&mut self) {
        self.new_item_text.pop();
    }

    pub fn take_new_item(&mut self) -> String {
        self.adding_item = false;
        std::mem::take(&mut self.new_item_text)
    }

    pub fn set_items(&mut self, items: Vec<ShoppingListItem>) {
        let unchecked: Vec<ShoppingListItem> =
            items.iter().filter(|i| !i.checked).cloned().collect();
        let checked: Vec<ShoppingListItem> = items.iter().filter(|i| i.checked).cloned().collect();

        let mut grouped: Vec<(String, Vec<ShoppingListItem>)> = Vec::new();
        let mut categories = std::collections::BTreeMap::<String, Vec<ShoppingListItem>>::new();

        for item in &unchecked {
            let cat = item.category.clone().unwrap_or_else(|| "Other".to_string());
            categories.entry(cat).or_default().push(item.clone());
        }

        for (cat, items) in categories {
            grouped.push((cat, items));
        }

        if self.show_checked && !checked.is_empty() {
            grouped.push(("Checked".to_string(), checked));
        }

        self.items = items;
        self.grouped = grouped;
    }

    pub fn select_next(&mut self) {
        let total = self
            .grouped
            .iter()
            .map(|(_, items)| items.len())
            .sum::<usize>();
        if total == 0 {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % total));
    }

    pub fn select_previous(&mut self) {
        let total = self
            .grouped
            .iter()
            .map(|(_, items)| items.len())
            .sum::<usize>();
        if total == 0 {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + total - 1) % total));
    }

    pub fn selected_item_id(&self) -> Option<uuid::Uuid> {
        if let Some(idx) = self.list_state.selected() {
            let mut count = 0;
            for (_, items) in &self.grouped {
                for item in items {
                    if count == idx {
                        return Some(item.id);
                    }
                    count += 1;
                }
            }
        }
        None
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &mut ShoppingListState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let total = state.items.iter().filter(|i| !i.checked).count();
    let checked = state.items.iter().filter(|i| i.checked).count();

    let header = Paragraph::new(format!(
        "Shopping List ({} items, {} checked)",
        total, checked
    ))
    .style(
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Shopping List"),
    );
    frame.render_widget(header, chunks[0]);

    let mut items: Vec<ListItem> = Vec::new();
    for (category, cat_items) in &state.grouped {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            format!("── {} ──", category),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )])));

        for item in cat_items {
            let mut spans = vec![Span::raw("  ")];

            if item.checked {
                spans.push(Span::styled(
                    "[✓] ",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM),
                ));
            } else {
                spans.push(Span::raw("[ ] "));
            }

            let item_style = if item.checked {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(Color::White)
            };

            let display = if let (Some(qty), Some(unit)) = (&item.quantity, &item.unit) {
                format!("{} {} {}", qty, unit, item.item)
            } else if let Some(qty) = &item.quantity {
                format!("{} {}", qty, item.item)
            } else {
                item.item.clone()
            };

            spans.push(Span::styled(display, item_style));

            if let Some(ref note) = item.unit {
                spans.push(Span::styled(
                    format!(" ({})", note),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            items.push(ListItem::new(Line::from(spans)));
        }
    }

    if items.is_empty() {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "  Empty - add items from meal plan or recipes",
            Style::default().fg(Color::DarkGray),
        )])));
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    if state.adding_item() {
        let prompt = Paragraph::new(format!("Add item: {}▌", state.new_item_text()))
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("New Item"));
        frame.render_widget(prompt, chunks[1]);
    } else {
        frame.render_stateful_widget(list, chunks[1], &mut state.list_state);
    }

    let footer = Paragraph::new("Space: check | a: add | g: from plan | x: clear done | ?: help")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
