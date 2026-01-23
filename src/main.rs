use std::fs;
use std::io::prelude::*;
use std::os::unix::net::UnixStream;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::{thread, time};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::widgets::Tabs;
use ratatui::widgets::Widget;
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
struct Video {
    id: String,
    title: String,
    uploader: String,
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
struct App {
    /// Is the application running?
    running: bool,
    //menulist_state: ListState,
    resultlist_state: ListState,

    mode: Mode,
    search_query: String,
    video_list: Vec<Video>,
    tabs_titles: Vec<&'static str>,
    tabs_current: usize,
    child_process: Option<Child>,
    mpv_stream: Option<UnixStream>,
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
        let tabs_titles = vec!["Search", "Now Playing"];
        let tabs_current: usize = 0;
        let child_process: Option<Child> = None;
        let mpv_stream: Option<UnixStream> = None;

        Self {
            running,
            //menulist_state,
            mode,
            search_query,
            video_list,
            resultlist_state,
            tabs_titles,
            tabs_current,
            child_process,
            mpv_stream,
        }
    }
    fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
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
        let [tabs_area, search_area, results_area, status_area] = Layout::vertical([
            Constraint::Length(1),
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

        let tabs = Tabs::new(self.tabs_titles.clone())
            .select(Some(self.tabs_current))
            .highlight_style(style::Color::Green);

        tabs.render(tabs_area, frame.buffer_mut());

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
            Paragraph::new(" H/L: Switch Tab ")
                .block(status_block.clone())
                .centered(),
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
                            format!("{:<40} uploader: {}", video.title, video.uploader),
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

    fn play_video(&mut self) {
        self.mode = Mode::NowPlaying;
        if fs::exists("/tmp/mpv-socket").expect("Can't check if /tmp/mpv-socket exists") {
            fs::remove_file("/tmp/mpv-socket").expect("could not remove /tmp/mpv-socket");
        }
        if let Some(index) = self.resultlist_state.selected() {
            let child = Command::new("mpv")
                .arg(format!(
                    "https://www.youtube.com/watch?v={}",
                    self.video_list[index].id
                ))
                .arg("--no-video")
                .arg("--input-ipc-server=/tmp/mpv-socket")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn()
                .expect("Error loading the video");
            self.child_process = Some(child);
        }
        let mut tries = 0;
        loop {
            if tries > 3 {
                break;
            }
            thread::sleep(time::Duration::from_millis(500));
            match UnixStream::connect("/tmp/mpv-socket") {
                Ok(o) => self.mpv_stream = Some(o),
                Err(e) => {
                    println!("Could not connect mpv-socket: {e}");
                }
            };
            tries += 1;
        }
    }

    pub fn send_mpv_command(&mut self, command: &str, args: Vec<&str>) {
        let mut vec_args: Vec<String> = Vec::new();
        for arg in args {
            vec_args.push(format!("\"{}\"", arg));
        }
        let json_args = vec_args.join(",");

        let message = format!("{{\"{command}\": [{json_args}]}}\n");
        if let Some(ref mut stream) = self.mpv_stream {
            stream
                .write_all(message.as_bytes())
                .expect("Could not write the command to mpv-socket");
        } else {
            println!("Mpv socket not connected.");
        }
    }

    fn tabs_next_index(&mut self) {
        self.tabs_current = (self.tabs_current + 1) % self.tabs_titles.len();
    }

    fn tabs_previous_index(&mut self) {
        if self.tabs_current > 0 {
            self.tabs_current -= 1;
        } else {
            self.tabs_current = self.tabs_titles.len() - 1;
        }
    }
    /// Reads the crossterm gevents and updates the state of [`App`].
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
        match key.code {
            KeyCode::Char('H') => self.tabs_next_index(),
            KeyCode::Char('L') => self.tabs_previous_index(),
            _ => {}
        }
        match self.mode {
            Mode::Search => match key.code {
                KeyCode::Char('q') => self.quit(),
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
                KeyCode::Char('q') => self.quit(),
                KeyCode::Char('j') => self.resultlist_state.select_next(),
                KeyCode::Char('k') => self.resultlist_state.select_previous(),
                KeyCode::Enter => self.play_video(),
                _ => {}
            },
            Mode::NowPlaying => match key.code {
                KeyCode::Esc | KeyCode::Char('s') => {
                    self.send_mpv_command("command", vec!["stop"]);
                    self.mode = Mode::Results;
                }
                KeyCode::Char(' ') => self.send_mpv_command("command", vec!["cycle", "pause"]),
                _ => {}
            },
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        if let Some(ref mut child) = self.child_process {
            child
                .kill()
                .expect("No Child getting killed today. Humanity");
        } else {
            println!("Could not kill child. Call IDF");
        }
        self.running = false;
    }
}
