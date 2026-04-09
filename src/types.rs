use derive_setters::Setters;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Text},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Widget, Wrap},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, PartialEq)]
pub enum Mode {
    #[default]
    Default,
    Search,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum PlaybackMode {
    #[default]
    Audio,
    Video,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Screen {
    #[default]
    //Menu,
    Queue,
    Results,
}

impl Screen {
    pub fn next(&mut self) {
        *self = match self {
            Screen::Queue => Screen::Results,
            Screen::Results => Screen::Queue,
        }
    }

    pub fn previous(&mut self) {
        *self = match self {
            Screen::Queue => Screen::Results,
            Screen::Results => Screen::Queue,
        }
    }

    pub fn select(&mut self, index: usize) {
        match index {
            0 => *self = Screen::Queue,
            1 => *self = Screen::Results,
            _ => {}
        }
    }

    pub fn current(&self) -> usize {
        match self {
            Screen::Queue => 0,
            Screen::Results => 1,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub uploader: String,
    // duration: f64,
}

#[derive(Debug, Default, Setters)]
pub struct Popup<'a> {
    #[setters(into)]
    title: Line<'a>,
    #[setters(into)]
    content: Text<'a>,
    borders: Borders,
    border_style: Style,
    border_type: BorderType,
    title_style: Style,
    style: Style,
    padding: Padding,
}

impl Widget for Popup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ensure that all cells under the popup are cleared to avoid leaking content
        Clear.render(area, buf);
        let block = Block::new()
            .title(self.title)
            .title_style(self.title_style)
            .borders(self.borders)
            .border_type(self.border_type)
            .border_style(self.border_style)
            .padding(self.padding);
        Paragraph::new(self.content)
            .wrap(Wrap { trim: true })
            .style(self.style)
            .block(block)
            .render(area, buf);
    }
}
