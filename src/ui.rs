pub mod popup;
pub mod tabs_state;

pub use popup::Popup;
pub use tabs_state::TabsState;

use crate::app::{App, Mode, PlaybackMode, Screen, Video};

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize, palette::material},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph, Tabs},
};

// Color Scheme
const COLOR_SCHEME: material::AccentedPalette = material::BLUE;
const SUBTEXT_FG: Color = COLOR_SCHEME.c600;
const HIGHLIGHT_FG: Color = material::BLACK;
const HIGHLIGHT_BG: Color = COLOR_SCHEME.a100;
const BORDER_FG: Color = COLOR_SCHEME.a100;

impl App {
    /// Renders the user interface.
    pub fn render(&mut self, frame: &mut Frame) {
        let [header_area, content_area, status_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        render_header(self, frame, header_area);

        render_status_bar(self, frame, status_area);

        match self.screen {
            Screen::Results => {
                render_content(frame, content_area, self.resultlist_state, &self.resultlist);
            }
            Screen::Queue => {
                render_content(frame, content_area, self.queuelist_state, &self.queuelist);
            }
        }

        match self.mode {
            Mode::Default => {}
            Mode::Search => {
                render_search(self, frame);
            }
        }
    }
}
fn render_status_bar(app: &App, frame: &mut Frame<'_>, status_area: Rect) {
    // status_bar
    let [status_area_left, status_area_center, status_area_right] = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .areas(status_area);

    let block_border_type = BorderType::Rounded;
    let block_border_style = Style::new().fg(BORDER_FG);
    let (left_block, center_block, right_block) = (
        Block::new()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .border_type(block_border_type)
            .border_style(block_border_style),
        Block::new()
            .borders(Borders::TOP | Borders::BOTTOM)
            .border_type(block_border_type)
            .border_style(block_border_style),
        Block::new()
            .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
            .border_type(block_border_type)
            .border_style(block_border_style),
    );
    match app.playback_mode {
        PlaybackMode::Audio => {
            frame.render_widget(
                Paragraph::new(" Mode: [Audio] ")
                    .left_aligned()
                    .fg(BORDER_FG)
                    .block(left_block),
                status_area_left,
            );
        }
        PlaybackMode::Video => {
            frame.render_widget(
                Paragraph::new(" Mode: [Video] ")
                    .left_aligned()
                    .fg(BORDER_FG)
                    .block(left_block),
                status_area_left,
            );
        }
    }
    frame.render_widget(
        Paragraph::new("  ")
            .left_aligned()
            .fg(BORDER_FG)
            .block(center_block.clone()),
        status_area_center,
    );
    frame.render_widget(
        Paragraph::new("  ")
            .right_aligned()
            .fg(BORDER_FG)
            .block(center_block),
        status_area_center,
    );
    frame.render_widget(
        Paragraph::new("  ")
            .right_aligned()
            .fg(BORDER_FG)
            .block(right_block),
        status_area_right,
    );

    // ---------- status_bar
}
fn render_content(
    frame: &mut Frame<'_>,
    content_area: Rect,
    mut list_state: ListState,
    videolist: &[Video],
) {
    //content
    let content_block_type = BorderType::Rounded;
    let content_block_style = Style::new().fg(BORDER_FG);
    let [content_block] = [Block::bordered()
        .border_type(content_block_type)
        .border_style(content_block_style)
        .padding(Padding::horizontal(1))];

    let items: Vec<ListItem> = videolist
        .iter()
        .map(|video| {
            ListItem::new(Line::from(vec![
                Span::from(format!(
                    "{:<1$}",
                    video.title,
                    (content_area.width as usize).saturating_sub(30)
                )),
                Span::styled(" | ", Style::new().dim()),
                Span::styled(&video.uploader, Style::new().fg(SUBTEXT_FG)),
            ]))
        })
        .collect();

    frame.render_widget(content_block.clone(), content_area);

    frame.render_stateful_widget(
        List::new(items)
            .block(content_block)
            .highlight_style(Style::new().fg(HIGHLIGHT_FG).bg(HIGHLIGHT_BG))
            .highlight_symbol("> "),
        content_area,
        &mut list_state,
    );
    // ---------- content
}

fn render_header(app: &App, frame: &mut Frame<'_>, header_area: Rect) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(header_area);
    let block_type = BorderType::Rounded;
    let block_style = Style::new().fg(BORDER_FG);
    let left_block = Block::new()
        .borders(Borders::TOP | Borders::LEFT | Borders::BOTTOM)
        .border_type(block_type)
        .border_style(block_style);
    let right_block = Block::new()
        .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
        .border_type(block_type)
        .border_style(block_style);

    let tabs = Tabs::new(app.tabs_titles.clone())
        .padding("", "")
        .divider("")
        .block(left_block.clone())
        .highlight_style(Style::new().fg(HIGHLIGHT_FG).bg(HIGHLIGHT_BG).bold())
        .select(Some(app.tabs_state.selected()));

    let now_playing = if app.is_nowplaying {
        Paragraph::new(String::from(&app.now_playing.title)).block(right_block.clone())
    } else {
        Paragraph::new(String::from(&app.now_playing.title))
            .block(right_block)
            .italic()
    };

    frame.render_widget(tabs, left);
    frame.render_widget(now_playing, right);

    // ------------- header
}

fn render_search(app: &App, frame: &mut Frame<'_>) {
    // search
    let [_, search_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(3),
        Constraint::Fill(1),
    ])
    .areas(frame.area());
    let [_, search_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Percentage(50),
        Constraint::Fill(1),
    ])
    .areas(search_area);

    let border_type = BorderType::Rounded;
    let border_style = Style::new().fg(BORDER_FG).dim();
    let title_style = Style::new().fg(BORDER_FG).bold().dim();
    let search = Popup::default()
        .content(format!(" {}", app.search_query))
        .title(" Search ")
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(border_style)
        .padding(Padding::horizontal(1));
    frame.render_widget(search, search_area);
    //------------search
}
