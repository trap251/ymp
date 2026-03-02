// Fix: Screens and Tabs logic. fix App::tabs_choose().
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use derive_setters::Setters;
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{
        Color, Style, Stylize,
        palette::material::{self, AccentedPalette, BLUE},
    },
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Tabs,
        Widget, Wrap,
    },
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

// Color Scheme
const COLOR_SCHEME: AccentedPalette = BLUE;
const SUBTEXT_FG: Color = COLOR_SCHEME.c600;
const HIGHLIGHT_FG: Color = material::BLACK;
const HIGHLIGHT_BG: Color = COLOR_SCHEME.a100;
const BORDER_FG: Color = COLOR_SCHEME.a100;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = App::new().run(terminal).await;
    ratatui::restore();
    result
}

#[derive(Debug, Default, PartialEq)]
enum Mode {
    #[default]
    Default,
    Search,
}

#[derive(Debug, Default, PartialEq)]
enum Screen {
    #[default]
    //Menu,
    Queue,
    Results,
}

/// The main application which holds the state and logic of the application.
#[derive(Debug, Default)]
struct App {
    /// Is the application running?
    running: bool,
    //menulist_state: ListState,
    resultlist: Vec<Video>,
    resultlist_state: ListState,
    queuelist: Vec<Video>,
    queuelist_state: ListState,

    mode: Mode,
    screen: Screen,
    search_query: String,
    pub tabs_titles: Vec<&'static str>,
    tabs_current: usize,
    child_process: Option<Child>,
    mpv_stream: Option<UnixStream>,
    mpv_connect_attempts: i8,
    now_playing: Video,
    is_nowplaying: bool,

    // tokio  search-related stuff
    search_is_loading: bool, // In-case I want to add a leading screen
    search_rx: Option<mpsc::UnboundedReceiver<color_eyre::Result<Vec<Video>>>>, //receives search results
}

impl App {
    /// Construct a new instance of [`App`].
    fn default() -> Self {
        let running = true;
        let resultlist = Vec::new();
        let resultlist_state = ListState::default().with_selected(Some(0));
        let queuelist = Vec::new();
        let queuelist_state = ListState::default().with_selected(Some(0));
        let mode = Mode::default();
        let screen = Screen::default();
        let search_query = String::default();
        let tabs_titles = vec!["     Queue     ", "     Results     "];
        let tabs_current: usize = 0;
        let child_process: Option<Child> = None;
        let mpv_stream: Option<UnixStream> = None;
        let mpv_connect_attempts = 0;
        let now_playing: Video = Video::default();
        let is_nowplaying = false;
        let search_is_loading = false;
        let search_rx = None;

        Self {
            running,
            //menulist_state,
            mode,
            screen,
            search_query,
            resultlist,
            resultlist_state,
            queuelist,
            queuelist_state,
            tabs_titles,
            tabs_current,
            child_process,
            mpv_stream,
            mpv_connect_attempts,
            now_playing,
            is_nowplaying,
            search_is_loading,
            search_rx,
        }
    }
    fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        //self.check_dependency("yt-dlp");
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
        let [header_area, content_area, status_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        self.render_header(frame, header_area);

        self.render_status_bar(frame, status_area);

        match self.screen {
            Screen::Results => {
                self.render_content(frame, content_area, self.resultlist_state, &self.resultlist);
            }
            Screen::Queue => {
                self.render_content(frame, content_area, self.queuelist_state, &self.queuelist);
            }
        }

        match self.mode {
            Mode::Default => {}
            Mode::Search => {
                self.render_search(frame);
            }
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

    fn render_status_bar(&self, frame: &mut Frame<'_>, status_area: Rect) {
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
        frame.render_widget(
            Paragraph::new(" j/k: Scroll ")
                .left_aligned()
                .block(left_block),
            status_area_left,
        );
        frame.render_widget(
            Paragraph::new(" H/L: Switch Tab ")
                .left_aligned()
                .block(center_block.clone()),
            status_area_center,
        );
        frame.render_widget(
            Paragraph::new(" /: Search ")
                .right_aligned()
                .block(center_block),
            status_area_center,
        );
        frame.render_widget(
            Paragraph::new(" Enter: Play Video ")
                .right_aligned()
                .block(right_block),
            status_area_right,
        );

        // ---------- status_bar
    }
    fn render_content(
        &self,
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

    fn render_header(&self, frame: &mut Frame<'_>, header_area: Rect) {
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

        let tabs = Tabs::new(self.tabs_titles.clone())
            .padding("", "")
            .divider("")
            .block(left_block.clone())
            .highlight_style(Style::new().fg(HIGHLIGHT_FG).bg(HIGHLIGHT_BG).bold())
            .select(Some(self.tabs_current));

        let now_playing = if self.is_nowplaying {
            Paragraph::new(String::from(&self.now_playing.title)).block(right_block.clone())
        } else {
            Paragraph::new(String::from(&self.now_playing.title))
                .block(right_block)
                .italic()
        };

        frame.render_widget(tabs, left);
        frame.render_widget(now_playing, right);

        // ------------- header
    }

    fn render_search(&self, frame: &mut Frame<'_>) {
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

        let search = Popup::default()
            .content(format!(" {}", self.search_query))
            .title(" Search ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().dim())
            .padding(Padding::horizontal(1));
        frame.render_widget(search, search_area);
        //------------search
    }

    fn search(&mut self) {
        self.search_is_loading = true;

        self.resultlist.clear();
        let (tx, rx) = mpsc::unbounded_channel();
        self.search_rx = Some(rx);

        let query = self.search_query.clone();

        tokio::spawn(async move {
            let out = Self::perform_search(query).await;
            let _ = tx.send(out);
        });

        // self.search_is_loading is set to false in check_search_results for obvious reasons. (Because
        // search_is_loading doesn't stop until check search results is completed)
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
                    self.resultlist = videos;
                    if !self.resultlist.is_empty() {
                        self.resultlist_state.select(Some(0));
                    }
                    self.search_is_loading = false;
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
                    self.search_is_loading = false;
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

        let (state, list) = match screen {
            Screen::Results => (&self.resultlist_state, &self.resultlist),
            Screen::Queue => (&self.queuelist_state, &self.queuelist),
        };

        if let Some(index) = state.selected() {
            self.now_playing = list[index].clone();
        }

        let child = Command::new("mpv")
            .arg("--ytdl-format=bestaudio")
            .arg(format!(
                "https://www.youtube.com/watch?v={}",
                self.now_playing.id
            ))
            .arg("--no-video")
            .arg("--input-ipc-server=/tmp/mpv-socket")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()?;
        self.child_process = Some(child);

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

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.kill_mpv();
        self.running = false;
    }
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

//fn check_dependency(&mut self, dependency: &str) {
//    let dependency_version_latest_command = Command::new("pacman")
//        .arg("-Si")
//        .arg(dependency)
//        .output()
//        .unwrap()
//        .stdout;

//    let dependency_version_latest_string =
//        String::from_utf8_lossy(&dependency_version_latest_command);

//    let dependency_version_latest = dependency_version_latest_string
//        .lines()
//        .find(|line| line.contains("Version"))
//        .unwrap();

//    let dependency_version_current_command = Command::new("pacman")
//        .arg("-Qi")
//        .arg(dependency)
//        .output()
//        .unwrap()
//        .stdout;

//    let dependency_version_current_string =
//        String::from_utf8_lossy(&dependency_version_current_command);

//    let dependency_version_current = dependency_version_current_string
//        .lines()
//        .find(|line| line.contains("Version"))
//        .unwrap();
//    if dependency_version_current != dependency_version_latest {
//        eprintln!("{dependency} out of date. Update {dependency}")
//    }
//}
