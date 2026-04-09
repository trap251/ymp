// TODO: Make code modular; separate parts into their own files
// FIX: Screens and Tabs logic. Fix App::tabs_select(). Fix magic numbers.
use crate::player::Player;
use crate::search;
use crate::types::{Mode, Screen, Video};

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{DefaultTerminal, widgets::ListState};
use std::{env, fs, path::PathBuf, time::Duration};
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

/// The main application which holds the state and logic of the application.
#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    running: bool,
    pub player: Player,
    //menulist_state: ListState,
    pub resultlist: Vec<Video>,
    pub resultlist_state: ListState,
    pub queuelist: Vec<Video>,
    pub queuelist_state: ListState,

    pub mode: Mode,
    pub screen: Screen,
    pub search_query: String,
    pub tabs_titles: Vec<String>,

    // tokio  search-related stuff
    search_is_loading: bool, // In-case I want to add a leading screen
    search_rx: Option<mpsc::UnboundedReceiver<color_eyre::Result<Vec<Video>>>>, //receives search results
}

impl App {
    /// Construct a new instance of [`App`].
    fn default() -> Self {
        let running = true;
        let player = Player::new();
        let tabs_titles: Vec<String> = vec![
            String::from("     Queue     "),
            String::from("     Results     "),
        ];
        let resultlist = Vec::new();
        let resultlist_state = ListState::default().with_selected(Some(0));
        let queuelist = Vec::new();
        let queuelist_state = ListState::default().with_selected(Some(0));
        let mode = Mode::default();
        let search_query = String::default();
        let screen = Screen::Queue;
        let search_is_loading = false;
        let search_rx = None;

        Self {
            running,
            player,
            tabs_titles,
            //menulist_state,
            resultlist,
            resultlist_state,
            queuelist,
            queuelist_state,
            mode,
            search_query,
            screen,
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
            self.player.play_video_url(url)?;
        }
        self.retrieve_queue()?;

        while self.running {
            terminal.draw(|frame| self.render(frame))?;
            self.check_search_results()?;
            self.player.try_connect_mpv();
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
                            self.screen.next();
                        }
                        KeyCode::Char('L') => {
                            self.screen.previous();
                        }
                        KeyCode::Char('j') => self.resultlist_state.select_next(),
                        KeyCode::Char('k') => self.resultlist_state.select_previous(),
                        KeyCode::Enter => {
                            self.player
                                .play_video(&self.resultlist, &self.resultlist_state)?;
                            self.queuelist.push(self.player.now_playing().clone());
                            self.save_queue()?;
                            self.screen.select(0);
                        }
                        KeyCode::Char('/') => self.mode = Mode::Search,
                        KeyCode::Char('m') => self.player.playback_mode_switch(),
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
                            self.screen.next();
                        }
                        KeyCode::Char('L') => {
                            self.screen.previous();
                        }
                        KeyCode::Char('j') => self.queuelist_state.select_next(),
                        KeyCode::Char('k') => self.queuelist_state.select_previous(),
                        KeyCode::Enter => self
                            .player
                            .play_video(&self.queuelist, &self.queuelist_state)?,
                        KeyCode::Char('/') => self.mode = Mode::Search,
                        KeyCode::Char('m') => self.player.playback_mode_switch(),
                        KeyCode::Esc | KeyCode::Char('s') => {
                            self.player.stop()?;
                            self.mode = Mode::Default;
                        }
                        KeyCode::Char('9') => {
                            if *self.player.is_nowplaying() {
                                self.player.decrease_volume()?;
                                self.player.get_current_volume()?;
                            }
                        }
                        KeyCode::Char('0') => {
                            if *self.player.is_nowplaying() {
                                self.player.increase_volume()?;
                            }
                        }
                        KeyCode::Char(' ') => {
                            if *self.player.is_nowplaying() {
                                self.player.play_pause()?;
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
            let out = search::perform_search(query).await;
            let _ = tx.send(out);
        });

        // self.search_is_loading is set to false in check_search_results for obvious reasons. (Because
        // search_is_loading doesn't stop until check search results is completed)
    }

    fn check_search_results(&mut self) -> color_eyre::Result<()> {
        if let Some(rx) = &mut self.search_rx {
            match rx.try_recv() {
                Ok(Ok(videos)) => {
                    self.screen.select(1);
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
        self.player.kill_mpv();
        self.running = false;
    }
}
