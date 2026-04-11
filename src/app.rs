use crate::player::Player;
use crate::search;
use crate::types::{Mode, Screen, Video};

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{DefaultTerminal, widgets::ListState};
use std::{env, fs, path::PathBuf, time::Duration};

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
    search: search::Search,
    //menulist_state: ListState,
    pub resultlist: Vec<Video>,
    pub resultlist_state: ListState,
    pub queuelist: Vec<Video>,
    pub queuelist_state: ListState,

    pub mode: Mode,
    pub screen: Screen,
    pub search_query: String,
    pub tabs_titles: Vec<String>,
}

impl App {
    /// Construct a new instance of [`App`].
    fn default() -> Self {
        let running = true;
        let search = search::Search::default();
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

        Self {
            running,
            search,
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
            match self.search.check_search_results() {
                Ok(videos) => {
                    self.screen.select(1);
                    self.resultlist = videos;
                    if !self.resultlist.is_empty() {
                        self.resultlist_state.select(Some(0));
                    }
                }
                Err(_) => {
                    // FIX add error handling
                }
            };

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
                    self.search
                        .search(&mut self.resultlist, self.search_query.to_owned());
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
