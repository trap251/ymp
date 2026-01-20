use std::io;
use std::process::Command;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{self},
    text::Span,
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use serde::Deserialize;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();
    result
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Video {
    id: String,
    title: String,
    //uploader: String,
    //#[serde(default)]
    //duration: String,
}

#[derive(Debug, Default, PartialEq)]
enum Mode {
    #[default]
    //Menu,
    Search,
    Results,
    NowPlaying,
}

/// The main application which holds the state and logic of the application.
#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    running: bool,
    //menulist_state: ListState,
    resultlist_state: ListState,

    mode: Mode,
    search_query: String,
    video_list: Vec<Video>,
}

impl App {
    /// Construct a new instance of [`App`].
    fn default() -> Self {
        let running = true;
        //let menulist_state = ListState::default().with_selected(Some(0));
        let resultlist_state = ListState::default().with_selected(Some(0));
        let mode = Mode::default();
        let search_query = String::default();
        let video_list = Vec::new();

        Self {
            running,
            //menulist_state,
            mode,
            search_query,
            video_list,
            resultlist_state,
        }
    }
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    pub fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        self.running = true;
        while self.running {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_crossterm_events()?;
        }
        Ok(())
    }

    /// Renders the user interface.
    ///
    /// This is where you add new widgets. See the following resources for more information:
    ///
    /// - <https://docs.rs/ratatui/latest/ratatui/widgets/index.html>
    /// - <https://github.com/ratatui/ratatui/tree/main/ratatui-widgets/examples>
    fn render(&mut self, frame: &mut Frame) {
        let [search_area, results_area, status_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        let [search_block, results_block, status_block] = [
            Block::bordered().title(" Search "),
            Block::bordered().title(" Results "),
            Block::bordered(),
        ];

        frame.render_widget(
            Paragraph::new(format!(" {}", self.search_query)).block(search_block),
            search_area,
        );
        frame.render_widget(results_block.clone(), results_area);

        frame.render_widget(
            Paragraph::new(" j/k: Scroll ")
                .block(status_block.clone())
                .left_aligned(),
            status_area,
        );
        frame.render_widget(
            Paragraph::new(" Enter: Play Video ")
                .block(status_block)
                .right_aligned(),
            status_area,
        );
        match self.mode {
            Mode::Search => {}
            Mode::Results => {
                let items: Vec<ListItem> = self
                    .video_list
                    .iter()
                    .map(|video| {
                        ListItem::new(Span::styled(
                            format!("{:<40}", video.title),
                            ratatui::style::Style::default().fg(ratatui::style::Color::Gray),
                        ))
                    })
                    .collect();

                frame.render_stateful_widget(
                    List::new(items)
                        .block(results_block)
                        .highlight_style(style::Color::Blue),
                    results_area,
                    &mut self.resultlist_state,
                );
            }
            Mode::NowPlaying => {}
        }
    }

    pub fn search(&mut self) {
        let options = Command::new("yt-dlp")
            .arg(format!("ytsearch25:{}", self.search_query))
            .arg("--dump-json")
            .arg("--flat-playlist")
            .arg("--no-warnings")
            .output();

        self.video_list.clear();

        match options {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);

                for line in stdout.lines() {
                    let video = serde_json::from_str::<Video>(line);
                    match video {
                        Ok(v) => {
                            self.video_list.push(v);
                        }
                        Err(e) => {
                            eprintln!("video json parsing nothing working: {}", e);
                        }
                    }
                }

                if !self.video_list.is_empty() {
                    self.resultlist_state.select(Some(0));
                }
            }

            Err(e) => {
                eprintln!(
                    "Could not parse json for Search Results obtained from yt-dlp. You might not have yt-dlp installed. Try sudo pacman -S yt-dlp.\nError:{} ",
                    e
                )
            }
        }
    }

    pub fn play_video(&mut self) {
        self.mode = Mode::NowPlaying;
        if let Some(index) = self.resultlist_state.selected() {
            let mut child = Command::new("mpv")
                .arg(format!(
                    "https://www.youtube.com/watch?v={}",
                    self.video_list[index].id
                ))
                .arg("--no-video")
                .arg("--input-ipc-server=/tmp/mpv-socket")
                .spawn()
                .expect("Error loading the video");
        }
    }

    /// Reads the crossterm events and updates the state of [`App`].
    ///
    /// If your application needs to perform work in between handling events, you can use the
    /// [`event::poll`] function to check if there are any events available with a timeout.
    fn handle_crossterm_events(&mut self) -> color_eyre::Result<()> {
        match event::read()? {
            // it's important to check KeyEventKind::Press to avoid handling key release events
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key),
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    /// Handles the key events and updates the state of [`App`].
    fn on_key_event(&mut self, key: KeyEvent) {
        match self.mode {
            Mode::Search => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.quit(),
                KeyCode::Char(c) => self.search_query.push(c),
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Enter => {
                    self.search();
                    self.mode = Mode::Results;
                }
                _ => {}
            },
            Mode::Results => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.quit(),
                KeyCode::Char('j') => self.resultlist_state.select_next(),
                KeyCode::Char('k') => self.resultlist_state.select_previous(),
                KeyCode::Enter => self.play_video(),
                _ => {}
            },
            Mode::NowPlaying => match key.code {
                KeyCode::Esc => self.mode = Mode::Results,
                KeyCode::Char('j') => self.resultlist_state.select_next(),
                KeyCode::Char('k') => self.resultlist_state.select_previous(),
                _ => {}
            },
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}
