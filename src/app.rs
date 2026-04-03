// TODO: Make code modular; separate parts into their own files
// FIX: Screens and Tabs logic. Fix App::tabs_select(). Fix magic numbers.
use crate::media::search::Search;
use crate::ui::TabsState;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, widgets::ListState};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{ErrorKind, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::sync::mpsc;

// Paths
// tries to find yt-dlp path e.g. /usr/bin/yt-dlp
// FIX: Install yt_dlp if path not found. HINT: Change ERR()

// queuelist is saved here
fn queuelist_path() -> String {
    match dirs::data_local_dir() {
        Some(mut path) => {
            path.push("ymp");
            path.push("queuelist.json");
            path.to_string_lossy().into_owned()
        }
        None => {
            let mut path = PathBuf::from(".");
            path.push("ymp");
            path.push("queuelist.json");
            path.to_string_lossy().into_owned()
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub enum Mode {
    #[default]
    Default,
    Search,
}

#[derive(Debug, Default, PartialEq)]
pub enum PlaybackMode {
    #[default]
    Audio,
    Video,
}

#[derive(Debug, Default, PartialEq)]
pub enum Screen {
    #[default]
    //Menu,
    Queue,
    Results,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub uploader: String,
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

/// The main application which holds the state and logic of the application.
#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    running: bool,
    //menulist_state: ListState,
    pub resultlist: Vec<Video>,
    pub resultlist_state: ListState,
    pub queuelist: Vec<Video>,
    pub queuelist_state: ListState,
    pub tabs_state: TabsState,

    pub mode: Mode,
    pub playback_mode: PlaybackMode,
    pub screen: Screen,
    pub search_query: String,
    pub tabs_titles: Vec<String>,
    mpv_process: Option<Child>,
    mpv_stream: Option<UnixStream>,
    mpv_connect_attempts: i8,
    pub now_playing: Video,
    pub is_nowplaying: bool,

    // tokio  search-related stuff
    search_is_loading: bool, // In-case I want to add a leading screen
    search_rx: Option<mpsc::UnboundedReceiver<color_eyre::Result<Vec<Video>>>>, //receives search results
}

impl App {
    /// Construct a new instance of [`App`].
    fn default() -> Self {
        let running = true;
        let tabs_titles: Vec<String> = vec![
            String::from("     Queue     "),
            String::from("     Results     "),
        ];
        let resultlist = Vec::new();
        let resultlist_state = ListState::default().with_selected(Some(0));
        let queuelist = Vec::new();
        let queuelist_state = ListState::default().with_selected(Some(0));
        let tabs_state = TabsState::new(tabs_titles.clone());
        let mode = Mode::default();
        let playback_mode = PlaybackMode::Audio;
        let search_query = String::default();
        //FIX::Could potentially index into a place that doesn't exist
        let screen = Screen::Queue;
        let mpv_process: Option<Child> = None;
        let mpv_stream: Option<UnixStream> = None;
        let mpv_connect_attempts = 0;
        let now_playing: Video = Video::default();
        let is_nowplaying = false;
        let search_is_loading = false;
        let search_rx = None;

        Self {
            running,
            tabs_titles,
            //menulist_state,
            resultlist,
            resultlist_state,
            queuelist,
            queuelist_state,
            tabs_state,
            mode,
            playback_mode,
            search_query,
            screen,
            mpv_process,
            mpv_stream,
            mpv_connect_attempts,
            now_playing,
            is_nowplaying,
            search_is_loading,
            search_rx,
        }
    }
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        //self.check_dependency("yt-dlp");
        self.running = true;
        if let Some(url) = env::args().nth(1) {
            self.play_video_url(url)?;
        }
        self.retrieve_queue()?;

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
                    self.screen = Screen::Results;
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
                            self.tabs_state.next();
                            self.screen = Screen::Queue
                        }
                        KeyCode::Char('L') => {
                            self.tabs_state.previous();
                            self.screen = Screen::Queue
                        }
                        KeyCode::Char('j') => self.resultlist_state.select_next(),
                        KeyCode::Char('k') => self.resultlist_state.select_previous(),
                        KeyCode::Enter => {
                            self.play_video(Screen::Results)?;
                            self.queuelist.push(self.now_playing.clone());
                            self.save_queue()?;
                            self.screen = Screen::Queue;
                            self.tabs_state.select(0);
                        }
                        KeyCode::Char('/') => self.mode = Mode::Search,
                        KeyCode::Char('m') => self.playback_mode_switch(),
                        _ => {}
                    }
                } else if self.screen == Screen::Queue {
                    match key.code {
                        KeyCode::Char('q' | 'Q') => self.quit(),
                        KeyCode::Char('c' | 'C') if key.modifiers == KeyModifiers::CONTROL => {
                            self.quit()
                        }
                        KeyCode::Char('C') => {
                            self.queuelist.clear();
                            self.save_queue()?;
                        }
                        KeyCode::Char('H') => {
                            self.tabs_state.next();
                            self.screen = Screen::Results;
                        }
                        KeyCode::Char('L') => {
                            self.tabs_state.previous();
                            self.screen = Screen::Results;
                        }
                        KeyCode::Char('j') => self.queuelist_state.select_next(),
                        KeyCode::Char('k') => self.queuelist_state.select_previous(),
                        KeyCode::Enter => self.play_video(Screen::Queue)?,
                        KeyCode::Char('/') => self.mode = Mode::Search,
                        KeyCode::Char('m') => self.playback_mode_switch(),
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

    fn search(&mut self) {
        self.search_is_loading = true;

        self.resultlist.clear();
        let (tx, rx) = mpsc::unbounded_channel();
        self.search_rx = Some(rx);

        let query = self.search_query.clone();

        tokio::spawn(async move {
            let out = Search::perform_search(query).await;
            let _ = tx.send(out);
        });

        // self.search_is_loading is set to false in check_search_results for obvious reasons. (Because
        // search_is_loading doesn't stop until check search results is completed)
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

        match self.playback_mode {
            PlaybackMode::Audio => {
                let child = Command::new("mpv")
                    .arg("--ytdl-format=bestaudio")
                    .arg(format!(
                        "https://www.youtube.com/watch?v={}",
                        self.now_playing.id
                    ))
                    .arg("--input-ipc-server=/tmp/mpv-socket")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .stdin(Stdio::null())
                    .spawn()?;
                self.mpv_process = Some(child);
            }
            PlaybackMode::Video => {
                let child = Command::new("mpv")
                    .arg(format!(
                        "https://www.youtube.com/watch?v={}",
                        self.now_playing.id
                    ))
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?;
                self.mpv_process = Some(child);
            }
        }

        self.mpv_connect_attempts = 10;
        Ok(())
    }

    fn play_video_url(&mut self, url: String) -> color_eyre::Result<()> {
        if self.is_nowplaying {
            self.kill_mpv();
        }
        self.is_nowplaying = true;
        match self.playback_mode {
            PlaybackMode::Audio => {
                let child = Command::new("mpv")
                    .arg("--ytdl-format=bestaudio")
                    .arg(url)
                    .arg("--input-ipc-server=/tmp/mpv-socket")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .stdin(Stdio::null())
                    .spawn()?;
                self.mpv_process = Some(child);
            }
            PlaybackMode::Video => {
                let child = Command::new("mpv")
                    .arg(url)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?;
                self.mpv_process = Some(child);
            }
        }
        self.mpv_connect_attempts = 10;
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

        if let Some(ref mut child) = self.mpv_process {
            if let Err(e) = child.kill() {
                eprintln!("Could not kill mpv child process, call idf: {e}");
            }
            if let Err(e) = child.wait() {
                eprintln!("Could not wait on mpv child process: {e}");
            }
        }
        self.mpv_process = None;
        if let Err(e) = fs::remove_file("/tmp/mpv-socket")
            && e.kind() != ErrorKind::NotFound
        {
            eprintln!("Could not remove /tmp/mpv-socket file: {e}");
        }
    }

    fn playback_mode_switch(&mut self) {
        if self.playback_mode == PlaybackMode::Audio {
            self.playback_mode = PlaybackMode::Video;
        } else {
            self.playback_mode = PlaybackMode::Audio;
        }
    }

    fn save_queue(&self) -> color_eyre::Result<()> {
        if let Some((path, _filename)) = queuelist_path().rsplit_once("/") {
            fs::DirBuilder::new().recursive(true).create(path)?;
        }
        let queuelist_json = serde_json::to_string_pretty(&self.queuelist)?;
        fs::write(queuelist_path(), queuelist_json)?;
        Ok(())
    }

    fn retrieve_queue(&mut self) -> color_eyre::Result<()> {
        let queuelist_path_string = queuelist_path();
        if fs::exists(&queuelist_path_string)? {
            let queuelist = fs::read_to_string(&queuelist_path_string)?;
            self.queuelist = serde_json::from_str(queuelist.as_str())?;
        }
        Ok(())
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.kill_mpv();
        self.running = false;
    }
}
