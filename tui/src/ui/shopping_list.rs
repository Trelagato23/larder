use larder_core::models::ShoppingListItem;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

enum DisplayRow {
    Header(String),
    Item(ShoppingListItem),
}

pub struct ShoppingListState {
    items: Vec<ShoppingListItem>,
    list_state: ListState,
    show_checked: bool,
    rows: Vec<DisplayRow>,
    adding_item: bool,
    new_item_text: String,
}

impl ShoppingListState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            show_checked: false,
            rows: Vec::new(),
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

    pub fn toggle_show_checked(&mut self) {
        self.show_checked = !self.show_checked;
        self.rebuild_rows();
        self.ensure_selection();
    }

    pub fn show_checked(&self) -> bool {
        self.show_checked
    }

    pub fn set_items(&mut self, items: Vec<ShoppingListItem>) {
        self.items = items;
        self.rebuild_rows();
        let max = self.selectable_count();
        if max == 0 {
            self.list_state.select(None);
        } else if self.list_state.selected().unwrap_or(0) >= max {
            self.list_state.select(Some(0));
        }
    }

    fn rebuild_rows(&mut self) {
        let mut rows = Vec::new();
        let mut categories = std::collections::BTreeMap::<String, Vec<ShoppingListItem>>::new();

        for item in self.items.iter().filter(|i| !i.checked) {
            let cat = item.category.clone().unwrap_or_else(|| "Other".to_string());
            categories.entry(cat).or_default().push(item.clone());
        }

        for (cat, cat_items) in categories {
            rows.push(DisplayRow::Header(cat));
            for item in cat_items {
                rows.push(DisplayRow::Item(item));
            }
        }

        if self.show_checked {
            let checked: Vec<ShoppingListItem> =
                self.items.iter().filter(|i| i.checked).cloned().collect();
            if !checked.is_empty() {
                rows.push(DisplayRow::Header("Checked".to_string()));
                for item in checked {
                    rows.push(DisplayRow::Item(item));
                }
            }
        }

        self.rows = rows;
    }

    fn selectable_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| matches!(r, DisplayRow::Item(_)))
            .count()
    }

    fn item_index_for_selectable(&self, selectable_idx: usize) -> Option<usize> {
        let mut count = 0;
        for (i, row) in self.rows.iter().enumerate() {
            if matches!(row, DisplayRow::Item(_)) {
                if count == selectable_idx {
                    return Some(i);
                }
                count += 1;
            }
        }
        None
    }

    fn selectable_index(&self) -> Option<usize> {
        let row_idx = self.list_state.selected()?;
        if matches!(self.rows.get(row_idx)?, DisplayRow::Item(_)) {
            let mut count = 0;
            for (i, row) in self.rows.iter().enumerate() {
                if matches!(row, DisplayRow::Item(_)) {
                    if i == row_idx {
                        return Some(count);
                    }
                    count += 1;
                }
            }
        }
        None
    }

    pub fn select_next(&mut self) {
        let total = self.selectable_count();
        if total == 0 {
            return;
        }
        let next = self.selectable_index().map(|i| (i + 1) % total).unwrap_or(0);
        if let Some(row_idx) = self.item_index_for_selectable(next) {
            self.list_state.select(Some(row_idx));
        }
    }

    pub fn select_previous(&mut self) {
        let total = self.selectable_count();
        if total == 0 {
            return;
        }
        let prev = self
            .selectable_index()
            .map(|i| (i + total - 1) % total)
            .unwrap_or(0);
        if let Some(row_idx) = self.item_index_for_selectable(prev) {
            self.list_state.select(Some(row_idx));
        }
    }

    pub fn selected_item_id(&self) -> Option<uuid::Uuid> {
        let row_idx = self.list_state.selected()?;
        match self.rows.get(row_idx)? {
            DisplayRow::Item(item) => Some(item.id),
            DisplayRow::Header(_) => None,
        }
    }

    pub fn ensure_selection(&mut self) {
        if self.selectable_count() == 0 {
            self.list_state.select(None);
            return;
        }
        if self
            .list_state
            .selected()
            .and_then(|i| self.rows.get(i))
            .map(|r| matches!(r, DisplayRow::Item(_)))
            .unwrap_or(false)
        {
            return;
        }
        if let Some(row_idx) = self.item_index_for_selectable(0) {
            self.list_state.select(Some(row_idx));
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &mut ShoppingListState, status: &str) {
    state.ensure_selection();

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
    let checked_hint = if state.show_checked() {
        "hiding checked"
    } else {
        "v: show checked"
    };

    let header = Paragraph::new(format!(
        "Shopping List ({} open, {} done) — {}",
        total, checked, checked_hint
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
    for row in &state.rows {
        match row {
            DisplayRow::Header(category) => {
                items.push(ListItem::new(Line::from(vec![Span::styled(
                    format!("── {} ──", category),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )])));
            }
            DisplayRow::Item(item) => {
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
                items.push(ListItem::new(Line::from(spans)));
            }
        }
    }

    if items.is_empty() {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "  Empty — g: from meal plan | a: add item",
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

    let mut footer = "Space: check | a: add | Del: remove | v: toggle done | g: plan | x: clear done | ?: help".to_string();
    if !status.is_empty() {
        footer = format!("{} | {}", status, footer);
    }
    let footer = Paragraph::new(footer).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
