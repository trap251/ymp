// Fix: Screens and Tabs logic. fix App::tabs_choose().
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use derive_setters::Setters;
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Widget, Wrap},
};
use serde::Deserialize;
use std::{
    fs,
    io::{ErrorKind, Write},
    os::unix::net::UnixStream,
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = App::new().run(terminal).await;
    ratatui::restore();
    result
}

#[derive(Debug, Default, Setters)]
pub struct Popup<'a> {
    #[setters(into)]
    title: Line<'a>,
    #[setters(into)]
    content: Text<'a>,
    border_style: Style,
    title_style: Style,
    style: Style,
}

impl Widget for Popup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ensure that all cells under the popup are cleared to avoid leaking content
        Clear.render(area, buf);
        let block = Block::new()
            .title(self.title)
            .title_style(self.title_style)
            .borders(Borders::ALL)
            .border_style(self.border_style);
        Paragraph::new(self.content)
            .wrap(Wrap { trim: true })
            .style(self.style)
            .block(block)
            .render(area, buf);
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
struct Video {
    id: String,
    title: String,
    #[serde(default)]
    uploader: String,
    // duration: f64,
}

impl Video {
    fn play_pause(app: &mut App) -> color_eyre::Result<()> {
        app.send_mpv_command(vec!["cycle", "pause"])
    }
    fn stop(app: &mut App) -> color_eyre::Result<()> {
        app.is_nowplaying = false;
        app.kill_mpv();
        Ok(())
    }
    fn increase_volume(app: &mut App) -> color_eyre::Result<()> {
        app.send_mpv_command(vec!["add", "volume", "5"])
    }
    fn decrease_volume(app: &mut App) -> color_eyre::Result<()> {
        app.send_mpv_command(vec!["add", "volume", "-5"])
    }
    fn get_current_volume(app: &mut App) -> color_eyre::Result<()> {
        app.send_mpv_command(vec!["get_property", "volume"])
    }
}

#[derive(Debug, Default, PartialEq)]
enum Mode {
    #[default]
    Default,
    Search,
}

/// The main application which holds the state and logic of the application.
#[derive(Debug, Default)]
struct App {
    /// Is the application running?
    running: bool,
    //menulist_state: ListState,
    resultlist_state: ListState,
    queuelist: Vec<Video>,
    queuelist_state: ListState,

    mode: Mode,
    screen: Screen,
    search_query: String,
    video_list: Vec<Video>,
    pub tabs_titles: Vec<&'static str>,
    tabs_current: usize,
    child_process: Option<Child>,
    mpv_stream: Option<UnixStream>,
    mpv_connect_attempts: i8,
    now_playing: Video,
    is_nowplaying: bool,

    // tokio  search-related stuff
    is_loading: bool, // In-case I want to add a leading screen
    search_rx: Option<mpsc::UnboundedReceiver<color_eyre::Result<Vec<Video>>>>, //receives search results
}

#[derive(Debug, Default, PartialEq)]
enum Screen {
    #[default]
    //Menu,
    Queue,
    Results,
}

impl App {
    /// Construct a new instance of [`App`].
    fn default() -> Self {
        let running = true;
        //let menulist_state = ListState::default().with_selected(Some(0));
        let resultlist_state = ListState::default().with_selected(Some(0));
        let queuelist = Vec::new();
        let queuelist_state = ListState::default().with_selected(Some(0));
        let mode = Mode::default();
        let screen = Screen::default();
        let search_query = String::default();
        let video_list = Vec::new();
        let tabs_titles = vec!["Queue", "Results"];
        let tabs_current: usize = 0;
        let child_process: Option<Child> = None;
        let mpv_stream: Option<UnixStream> = None;
        let mpv_connect_attempts = 0;
        let now_playing: Video = Video::default();
        let is_nowplaying = false;
        let is_loading = false;
        let search_rx = None;

        Self {
            running,
            //menulist_state,
            mode,
            screen,
            search_query,
            video_list,
            resultlist_state,
            queuelist_state,
            queuelist,
            tabs_titles,
            tabs_current,
            child_process,
            mpv_stream,
            mpv_connect_attempts,
            now_playing,
            is_nowplaying,
            is_loading,
            search_rx,
        }
    }
    fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        self.check_dependency("yt-dlp");
        self.running = true;
        while self.running {
            terminal.draw(|frame| self.render(frame))?;
            self.check_search_results()?;
            self.try_connect_mpv();
            if event::poll(Duration::from_millis(50))? {
                self.handle_crossterm_events()?;
            }
        }
        Ok(())
    }

    /// Renders the user interface.
    fn render(&mut self, frame: &mut Frame) {
        let [tabs_area, results_area, status_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        let [search_area] = Layout::vertical([Constraint::Length(3)]).areas(frame.area());
        let [results_block, status_block] = [Block::bordered(), Block::bordered()];

        let tabs = Tabs::new(self.tabs_titles.clone())
            .select(Some(self.tabs_current))
            .highlight_style(Color::Green);

        tabs.render(tabs_area, frame.buffer_mut());
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
            Paragraph::new(" H/L: Switch Tab                    /: Search ")
                .block(status_block.clone())
                .centered(),
            status_area,
        );

        match self.screen {
            Screen::Results => {
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

                frame.render_widget(results_block.clone(), results_area);

                frame.render_stateful_widget(
                    List::new(items)
                        .block(results_block)
                        .highlight_style(Color::Blue),
                    results_area,
                    &mut self.resultlist_state,
                );
            }
            Screen::Queue => {
                let items: Vec<ListItem> = self
                    .queuelist
                    .iter()
                    .map(|video| {
                        ListItem::new(Span::styled(
                            format!("{:<40} uploader: {}", video.title, video.uploader,),
                            ratatui::style::Style::default().fg(ratatui::style::Color::Gray),
                        ))
                    })
                    .collect();

                frame.render_widget(results_block.clone(), results_area);

                frame.render_stateful_widget(
                    List::new(items)
                        .block(results_block)
                        .highlight_style(Color::Blue),
                    results_area,
                    &mut self.queuelist_state,
                );
            }
        }

        match self.mode {
            Mode::Default => {}
            Mode::Search => {
                let search = Popup::default()
                    .content(format!(" {}", self.search_query))
                    .title(" Search ");
                frame.render_widget(search, search_area);
            } //    Gauge::default()
              //        .block(status_block)
              //        .percent(29)
              //        .label(String::new())
              //        .gauge_style(Color::Red)
              //        .render(status_area, frame.buffer_mut());
        }
    }

    fn search(&mut self) {
        self.is_loading = true;

        self.video_list.clear();
        let (tx, rx) = mpsc::unbounded_channel();
        self.search_rx = Some(rx);

        let query = self.search_query.clone();

        tokio::spawn(async move {
            let out = Self::perform_search(query).await;
            let _ = tx.send(out);
        });

        // self.is_loading is set to false in check_search_results for obvious reasons. (Because
        // is_loading doesn't stop until check search results is completed)
    }
    async fn perform_search(query: String) -> color_eyre::Result<Vec<Video>> {
        let options = tokio::process::Command::new("yt-dlp")
            .arg(format!("ytsearch25:{}", query))
            .arg("--dump-json")
            .arg("--flat-playlist")
            .arg("--no-warnings")
            .output()
            .await?;

        if !options.status.success() {
            return Err(color_eyre::eyre::eyre!(
                "yt-dlp error: {} \nCheck if yt-dlp is latest.",
                String::from_utf8_lossy(&options.stderr)
            ));
        }

        let mut videos = Vec::new();
        let stdout = String::from_utf8_lossy(&options.stdout);

        for line in stdout.lines() {
            let video = serde_json::from_str::<Video>(line);
            match video {
                Ok(v) => {
                    videos.push(v);
                }
                Err(e) => {
                    eprintln!("video json parsing nothing working: {}", e);
                }
            }
        }

        Ok(videos)
    }

    fn check_search_results(&mut self) -> color_eyre::Result<()> {
        if let Some(rx) = &mut self.search_rx {
            match rx.try_recv() {
                Ok(Ok(videos)) => {
                    self.video_list = videos;
                    if !self.video_list.is_empty() {
                        self.resultlist_state.select(Some(0));
                    }
                    self.is_loading = false;
                    self.search_rx = None;
                }
                Ok(Err(e)) => {
                    return Err(color_eyre::eyre::eyre!("Videos not recived. Error: {}", e));
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // Might still be loading. Don't do anything here.
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Died unexpectedly
                    self.is_loading = false;
                    self.search_rx = None;
                }
            }
        }
        Ok(())
    }

    fn play_video(&mut self, screen: Screen) -> color_eyre::Result<()> {
        if self.is_nowplaying {
            self.kill_mpv();
        }
        self.is_nowplaying = true;

        if screen == Screen::Results
            && let Some(index) = self.resultlist_state.selected()
        {
            self.now_playing = self.video_list[index].clone();
            let child = Command::new("mpv")
                .arg("--ytdl-format=bestaudio")
                .arg(format!(
                    "https://www.youtube.com/watch?v={}",
                    self.video_list[index].id
                ))
                .arg("--no-video")
                .arg("--input-ipc-server=/tmp/mpv-socket")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn()?;
            self.child_process = Some(child);
        } else if screen == Screen::Queue
            && let Some(index) = self.queuelist_state.selected()
        {
            self.now_playing = self.queuelist[index].clone();
            let child = Command::new("mpv")
                .arg("--ytdl-format=bestaudio")
                .arg(format!(
                    "https://www.youtube.com/watch?v={}",
                    self.queuelist[index].id
                ))
                .arg("--no-video")
                .arg("--input-ipc-server=/tmp/mpv-socket")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn()?;
            self.child_process = Some(child);
        }

        self.mpv_connect_attempts = 10;
        //TEMP SOLUTION FIND BETTER WAY TO CHECK IF IPC LOADED
        Ok(())
    }

    fn try_connect_mpv(&mut self) {
        if self.mpv_connect_attempts == 0 {
            return;
        }
        match UnixStream::connect("/tmp/mpv-socket") {
            Ok(o) => {
                self.mpv_stream = Some(o);
                self.mpv_connect_attempts = 0;
            }
            Err(_) => {
                self.mpv_connect_attempts -= 1;
            }
        }
    }

    fn send_mpv_command(&mut self, args: Vec<&str>) -> color_eyre::Result<()> {
        let mut vec_args: Vec<String> = Vec::new();
        for arg in args {
            vec_args.push(format!("\"{}\"", arg));
        }
        let json_args = vec_args.join(",");
        let message = format!("{{\"command\": [{json_args}]}}\n");
        if let Some(ref mut stream) = self.mpv_stream
            && let Err(e) = stream.write_all(message.as_bytes())
        {
            eprintln!("Could not write to UnixStream at send_mpv_command(): {e} ");
        }
        Ok(())
    }

    fn kill_mpv(&mut self) {
        self.mpv_stream.take();

        if let Some(ref mut child) = self.child_process {
            if let Err(e) = child.kill() {
                eprintln!("Could not kill child, need idf: {e}");
            }
            if let Err(e) = child.wait() {
                eprintln!("Could not wait on child: {e}");
            }
        }
        self.child_process = None;
        if let Err(e) = fs::remove_file("/tmp/mpv-socket")
            && e.kind() != ErrorKind::NotFound
        {
            eprintln!("Could not remove /tmp/mpv-socket file: {e}");
        }
    }

    fn tabs_choose(&mut self, screen: Screen) {
        if screen == Screen::Queue {
            self.screen = Screen::Queue;
            self.tabs_current = 0;
        } else if screen == Screen::Results {
            self.screen = Screen::Results;
            self.tabs_current = 1;
        }
    }

    fn tabs_next(&mut self) {
        self.tabs_current = (self.tabs_current + 1) % self.tabs_titles.len();
    }

    fn tabs_previous(&mut self) {
        if self.tabs_current > 0 {
            self.tabs_current -= 1;
        } else {
            self.tabs_current = self.tabs_titles.len() - 1;
        }
    }

    fn check_dependency(&mut self, dependency: &str) {
        let dependency_version_latest_command = Command::new("pacman")
            .arg("-Si")
            .arg(dependency)
            .output()
            .unwrap()
            .stdout;

        let dependency_version_latest_string =
            String::from_utf8_lossy(&dependency_version_latest_command);

        let dependency_version_latest = dependency_version_latest_string
            .lines()
            .find(|line| line.contains("Version"))
            .unwrap();

        let dependency_version_current_command = Command::new("pacman")
            .arg("-Qi")
            .arg(dependency)
            .output()
            .unwrap()
            .stdout;

        let dependency_version_current_string =
            String::from_utf8_lossy(&dependency_version_current_command);

        let dependency_version_current = dependency_version_current_string
            .lines()
            .find(|line| line.contains("Version"))
            .unwrap();
        if dependency_version_current != dependency_version_latest {
            eprintln!("{dependency} out of date. Update {dependency}")
        }
    }

    /// Reads the crossterm gevents and updates the state of [`App`].
    ///
    /// If application needs to perform work in between handling events, use the
    /// [`event::poll`] function to check if there are any events available with a timeout.
    fn handle_crossterm_events(&mut self) -> color_eyre::Result<()> {
        match event::read()? {
            // it's important to check KeyEventKind::Press to avoid handling key release events
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key),
            Event::Mouse(_) => Ok(()),
            Event::Resize(_, _) => Ok(()),
            _ => Ok(()),
        }?;
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<()> {
        match self.mode {
            Mode::Search => match key.code {
                KeyCode::Char('c' | 'C') => {
                    if key.modifiers == KeyModifiers::CONTROL {
                        self.quit()
                    } else {
                        self.search_query.push('c');
                    }
                }
                KeyCode::Char(ch) => self.search_query.push(ch),
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Enter => {
                    self.search();
                    self.search_query = String::new();
                    self.mode = Mode::Default;
                    self.tabs_choose(Screen::Results);
                }
                KeyCode::Esc => {
                    self.search_query = String::new();
                    self.mode = Mode::Default;
                }
                _ => {}
            },
            Mode::Default => {
                if self.screen == Screen::Results {
                    match key.code {
                        KeyCode::Char('q' | 'Q') => self.quit(),
                        KeyCode::Char('c' | 'C') if key.modifiers == KeyModifiers::CONTROL => {
                            self.quit()
                        }
                        KeyCode::Char('H') => {
                            self.tabs_next();
                            self.screen = Screen::Queue
                        }
                        KeyCode::Char('L') => {
                            self.tabs_previous();
                            self.screen = Screen::Queue
                        }
                        KeyCode::Char('j') => self.resultlist_state.select_next(),
                        KeyCode::Char('k') => self.resultlist_state.select_previous(),
                        KeyCode::Enter => {
                            self.play_video(Screen::Results)?;
                            self.queuelist.push(self.now_playing.clone());
                            self.tabs_choose(Screen::Queue);
                        }
                        KeyCode::Char('/') => self.mode = Mode::Search,
                        _ => {}
                    }
                } else if self.screen == Screen::Queue {
                    match key.code {
                        KeyCode::Char('q' | 'Q') => self.quit(),
                        KeyCode::Char('c' | 'C') if key.modifiers == KeyModifiers::CONTROL => {
                            self.quit()
                        }
                        KeyCode::Char('H') => {
                            self.tabs_next();
                            self.screen = Screen::Results;
                        }
                        KeyCode::Char('L') => {
                            self.tabs_previous();
                            self.screen = Screen::Results;
                        }
                        KeyCode::Char('j') => self.queuelist_state.select_next(),
                        KeyCode::Char('k') => self.queuelist_state.select_previous(),
                        KeyCode::Enter => self.play_video(Screen::Queue)?,
                        KeyCode::Char('/') => self.mode = Mode::Search,
                        KeyCode::Esc | KeyCode::Char('s') => {
                            Video::stop(self)?;
                            self.mode = Mode::Default;
                        }
                        KeyCode::Char('9') => {
                            if self.is_nowplaying {
                                Video::decrease_volume(self)?;
                                Video::get_current_volume(self)?;
                            }
                        }
                        KeyCode::Char('0') => {
                            if self.is_nowplaying {
                                Video::increase_volume(self)?;
                            }
                        }
                        KeyCode::Char(' ') => {
                            if self.is_nowplaying {
                                Video::play_pause(self)?;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.kill_mpv();
        self.running = false;
    }
}
