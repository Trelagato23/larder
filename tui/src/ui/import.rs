use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub struct ImportState {
    pub url: String,
    pub cursor_pos: usize,
    pub status: String,
    pub importing: bool,
}

impl ImportState {
    pub fn new() -> Self {
        Self {
            url: String::new(),
            cursor_pos: 0,
            status: String::new(),
            importing: false,
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.url.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.url.remove(self.cursor_pos - 1);
            self.cursor_pos -= 1;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor_pos < self.url.len() {
            self.url.remove(self.cursor_pos);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_pos < self.url.len() {
            self.cursor_pos += 1;
        }
    }

    pub fn clear(&mut self) {
        self.url.clear();
        self.cursor_pos = 0;
        self.status.clear();
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ImportState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new("Import Recipe from URL")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title("Import"));
    frame.render_widget(header, chunks[0]);

    let cursor_char = if state.importing {
        " ".to_string()
    } else {
        "▌".to_string()
    };

    let url_display = if state.url.is_empty() {
        vec![
            Span::styled("Enter URL: ", Style::default().fg(Color::Cyan)),
            Span::styled(&cursor_char, Style::default().fg(Color::White)),
        ]
    } else {
        let before = &state.url[..state.cursor_pos];
        let after = &state.url[state.cursor_pos..];
        vec![
            Span::styled("Enter URL: ", Style::default().fg(Color::Cyan)),
            Span::raw(before),
            Span::styled(
                &cursor_char,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(after),
        ]
    };

    let url_input = Paragraph::new(Line::from(url_display))
        .block(Block::default().borders(Borders::ALL).title("URL"));
    frame.render_widget(url_input, chunks[1]);

    let status_text = if state.importing {
        "Fetching and parsing recipe..."
    } else if !state.status.is_empty() {
        &state.status
    } else {
        "Paste a recipe URL and press Enter to import"
    };

    let status_color = if state.importing {
        Color::Yellow
    } else if state.status.starts_with("Error") {
        Color::Red
    } else if state.status.starts_with("Imported") {
        Color::Green
    } else {
        Color::DarkGray
    };

    let status_widget = Paragraph::new(status_text).style(Style::default().fg(status_color));
    frame.render_widget(status_widget, chunks[2]);

    let footer = Paragraph::new("Enter: import | Esc: back | ?: help")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[3]);
}
