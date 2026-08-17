//! Co-op semantic colors for the TUI (DESIGN.md palette).
use larder_core::models::Difficulty;
use ratatui::style::Color;

/// Brand / role colors used across list, detail, editor, and status bar.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    pub money: Color,
    pub timer: Color,
    pub muted: Color,
    pub text: Color,
    pub highlight_fg: Color,
    pub highlight_bg: Color,
    pub easy: Color,
    pub medium: Color,
    pub hard: Color,
    pub danger: Color,
    pub border: Color,
}

impl Theme {
    /// Locked co-op light palette (kitchen fluorescent-friendly).
    pub const fn coop() -> Self {
        Self {
            accent: Color::Rgb(180, 35, 24),
            money: Color::Rgb(45, 106, 79),    // deli green — money reads calm
            timer: Color::Rgb(0, 180, 180),
            muted: Color::Rgb(107, 99, 90),   // #6b635a
            text: Color::Rgb(250, 248, 244),  // paper on dark terminal
            highlight_fg: Color::Rgb(250, 248, 244),
            highlight_bg: Color::Rgb(180, 35, 24), // brand reverse
            easy: Color::Rgb(45, 106, 79),
            medium: Color::Rgb(212, 160, 23),  // breakfast gold
            hard: Color::Rgb(180, 35, 24),
            danger: Color::Rgb(180, 35, 24),
            border: Color::Rgb(107, 99, 90),
        }
    }

    pub fn difficulty(self, d: Option<Difficulty>) -> Color {
        match d {
            Some(Difficulty::Easy) => self.easy,
            Some(Difficulty::Medium) => self.medium,
            Some(Difficulty::Hard) => self.hard,
            None => self.muted,
        }
    }

    /// Dept stripe from primary tag name (DESIGN.md).
    pub fn dept(tag: &str) -> Color {
        match tag.to_ascii_lowercase().as_str() {
            "bakery" => Color::Rgb(196, 122, 26),
            "deli" | "lunch" => Color::Rgb(45, 106, 79),
            "breakfast" => Color::Rgb(212, 160, 23),
            "dinner" => Color::Rgb(123, 45, 38),
            "snack" => Color::Rgb(107, 91, 149),
            _ => Color::Rgb(180, 35, 24),
        }
    }
}

pub const T: Theme = Theme::coop();
