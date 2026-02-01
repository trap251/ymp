use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{self},
    text::Span,
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use serde::Deserialize;
use std::error::Error;
use std::process::Command;

fn main() -> color_eyre::Result<(), Box<dyn Error>> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    App::new().run(&mut terminal)?;
    ratatui::restore();
    Ok(())
}

#[derive(Debug, Default, Clone, Deserialize)]
struct Video {
    id: String,
    title: String,
    uploader: String,
    // #[serde(default)]
    // duration: f64,
}

#[derive(Debug, Default, PartialEq)]
enum Mode {
    #[default]
    //Menu,
    Search,
    Results,
}

/// The main application which holds the state and logic of the application.
#[derive(Debug, Default)]
struct App {
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
    fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    fn run(mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        self.running = true;
        while self.running {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_crossterm_events(terminal)?;
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
                .block(status_block.clone())
                .right_aligned(),
            status_area,
        );

        frame.render_widget(
            Paragraph::new(" /: Search ").block(status_block).centered(),
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
                            format!("{:<40} uploader: {}", video.title, video.uploader,),
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
        }
    }

    fn search(&mut self) {
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

    fn play_video(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        if let Some(index) = self.resultlist_state.selected() {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            let _ = Command::new("mpv")
                .arg("--ytdl-format=bestaudio")
                .arg(format!(
                    "https://www.youtube.com/watch?v={}",
                    self.video_list[index].id
                ))
                .arg("--no-video")
                .status();
            enable_raw_mode()?;
            execute!(terminal.backend_mut(), EnterAlternateScreen)?;
            terminal.clear()?;
        }
        Ok(())
    }

    /// Reads the crossterm gevents and updates the state of [`App`].
    ///
    /// If your application needs to perform work in between handling events, you can use the
    /// [`event::poll`] function to check if there are any events available with a timeout.
    fn handle_crossterm_events(
        &mut self,
        terminal: &mut DefaultTerminal,
    ) -> color_eyre::Result<()> {
        match event::read()? {
            // it's important to check KeyEventKind::Press to avoid handling key release events
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key, terminal),
            Event::Mouse(_) => return Ok(()),
            Event::Resize(_, _) => return Ok(()),
            _ => return Ok(()),
        }?;
        Ok(())
    }

    fn on_key_event(
        &mut self,
        key: KeyEvent,
        terminal: &mut DefaultTerminal,
    ) -> color_eyre::Result<()> {
        match self.mode {
            Mode::Search => match key.code {
                KeyCode::Char('q' | 'Q') => self.quit(),
                KeyCode::Char('c' | 'C') if key.modifiers == KeyModifiers::CONTROL => self.quit(),
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
                KeyCode::Char('q' | 'Q') => self.quit(),
                KeyCode::Char('c' | 'C') if key.modifiers == KeyModifiers::CONTROL => self.quit(),
                KeyCode::Char('j') => self.resultlist_state.select_next(),
                KeyCode::Char('k') => self.resultlist_state.select_previous(),
                KeyCode::Enter => {
                    let _ = self.play_video(terminal);
                }
                KeyCode::Char('/') => self.mode = Mode::Search,
                _ => {}
            },
        }
        Ok(())
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}
