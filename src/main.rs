use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use derive_setters::Setters;
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Tabs, Widget, Wrap,
    },
};
use serde::Deserialize;
use std::{
    fs,
    io::Write,
    os::unix::net::UnixStream,
    process::{Child, Command, Stdio},
    thread, time,
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
    now_playing: Video,

    // tokio  search-related stuff
    is_loading: bool,
    search_rx: Option<mpsc::UnboundedReceiver<color_eyre::Result<Vec<Video>>>>, //receives search
                                                                                //results
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
    uploader: String,
    // #[serde(default)]
    // duration: f64,
}

impl Video {
    fn play_pause(app: &mut App) -> color_eyre::Result<()> {
        app.send_mpv_command(vec!["cycle", "pause"])
    }
    fn stop(app: &mut App) -> color_eyre::Result<()> {
        app.send_mpv_command(vec!["stop"])
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
    //Menu,
    Search,
    Results,
    NowPlaying,
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
        let tabs_titles = vec!["Search", "Queue"];
        let tabs_current: usize = 0;
        let child_process: Option<Child> = None;
        let mpv_stream: Option<UnixStream> = None;
        let now_playing: Video = Video::default();
        let is_loading = false;
        let search_rx = None;

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
            now_playing,
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
            self.handle_crossterm_events()?;
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
        let [results_block, status_block] =
            [Block::bordered().title(" Results "), Block::bordered()];

        let tabs = Tabs::new(self.tabs_titles.clone())
            .select(Some(self.tabs_current))
            .highlight_style(Color::Green);

        tabs.render(tabs_area, frame.buffer_mut());

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
            Mode::Search => {
                let search = Popup::default()
                    .content(format!(" {}", self.search_query))
                    .title(" Search ");
                frame.render_widget(search, search_area);
            }
            Mode::Results => {
                if self.is_loading {
                    frame.render_widget(
                        Paragraph::new("Results loading rn icl...").block(results_block.clone()),
                        results_area,
                    );
                } else {
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
                            .highlight_style(Color::Blue),
                        results_area,
                        &mut self.resultlist_state,
                    );
                    Gauge::default()
                        .block(status_block)
                        .percent(29)
                        .label(String::new())
                        .gauge_style(Color::Red)
                        .render(status_area, frame.buffer_mut());
                }
            }
            Mode::NowPlaying => {}
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

        // self.is_loading is set to false in check_search_results for obvious reasons. (Because is
        // loading doesn't stop until check search results is completed)
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

    fn play_video(&mut self) -> color_eyre::Result<()> {
        self.mode = Mode::NowPlaying;
        if fs::exists("/tmp/mpv-socket").unwrap()
            && let Err(e) = fs::remove_file("/tmp/mpv-socket")
        {
            eprintln!("Could not remove ipc file /tmp/mpv-socket: {e}");
        }

        if let Some(index) = self.resultlist_state.selected() {
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
        }

        //TEMP SOLUTION FIND BETTER WAY TO CHECK IF IPC LOADED
        for _ in 0..3 {
            thread::sleep(time::Duration::from_millis(500));
            match UnixStream::connect("/tmp/mpv-socket") {
                Ok(o) => {
                    self.mpv_stream = Some(o);
                    break;
                }
                Err(e) => {
                    eprintln!("Could not connect mpv-socket: {e}");
                }
            }
        }
        Ok(())
    }

    fn send_mpv_command(&mut self, args: Vec<&str>) -> color_eyre::Result<()> {
        let mut vec_args: Vec<String> = Vec::new();
        for arg in args {
            vec_args.push(format!("\"{}\"", arg));
        }
        let json_args = vec_args.join(",");
        let message = format!("{{\"command\": [{json_args}])\n");
        if let Some(ref mut stream) = self.mpv_stream
            && let Err(e) = stream.write_all(message.as_bytes())
        {
            println!("Could not write to UnixStream at send_mpv_command(): {e} ");
        }
        Ok(())
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
    /// If your application needs to perform work in between handling events, you can use the
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
                KeyCode::Char('q' | 'Q') => self.quit(),
                KeyCode::Char('c' | 'C') if key.modifiers == KeyModifiers::CONTROL => self.quit(),
                KeyCode::Char(ch) => self.search_query.push(ch),
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
                KeyCode::Char('H') => self.tabs_next_index(),
                KeyCode::Char('L') => self.tabs_previous_index(),
                KeyCode::Char('j') => self.resultlist_state.select_next(),
                KeyCode::Char('k') => self.resultlist_state.select_previous(),
                KeyCode::Enter => self.play_video()?,
                KeyCode::Char('/') => self.mode = Mode::Search,
                _ => {}
            },
            Mode::NowPlaying => match key.code {
                KeyCode::Char('H') => self.tabs_next_index(),
                KeyCode::Char('L') => self.tabs_previous_index(),
                KeyCode::Esc | KeyCode::Char('s') => {
                    Video::stop(self)?;
                    self.mode = Mode::Results;
                }
                KeyCode::Char('9') => {
                    Video::decrease_volume(self)?;
                    Video::get_current_volume(self)?;
                }
                KeyCode::Char('0') => {
                    Video::increase_volume(self)?;
                    Video::get_current_volume(self)?;
                }
                KeyCode::Char(' ') => Video::play_pause(self)?,
                _ => {}
            },
        }
        Ok(())
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        if let Some(ref mut child) = self.child_process
            && let Err(e) = child.kill()
        {
            eprintln!("Could not kill child: {e}"); /* Call IDF */
        }
        self.running = false;
    }
}
