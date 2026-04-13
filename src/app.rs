use crate::player::Player;
use crate::queue::Queue;
use crate::search;
use crate::types::{Mode, Screen, Video};

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{DefaultTerminal, widgets::ListState};
use std::{env, time::Duration};

/// The main application which holds the state and logic of the application.
#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    running: bool,
    pub player: Player,
    pub queue: Queue,
    search: search::Search,
    //menulist_state: ListState,
    pub resultlist: Vec<Video>,
    pub resultlist_state: ListState,

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
        let queue = Queue::new();
        let tabs_titles: Vec<String> = vec![
            String::from("     Queue     "),
            String::from("     Results     "),
        ];
        let resultlist = Vec::new();
        let resultlist_state = ListState::default().with_selected(Some(0));
        let mode = Mode::default();
        let search_query = String::default();
        let screen = Screen::Queue;

        Self {
            running,
            search,
            player,
            queue,
            tabs_titles,
            //menulist_state,
            resultlist,
            resultlist_state,
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
        self.queue.retrieve_queue()?;

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
                            if self.queue.queuelist().is_empty() {
                                self.queue
                                    .add_to_queue(&self.resultlist, &self.resultlist_state)?;
                                self.player.play_video(&mut self.queue)?;
                            } else {
                                self.queue
                                    .add_to_queue(&self.resultlist, &self.resultlist_state)?;
                            }
                            self.queue.save_queue()?;
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
                            self.queue.queuelist().clear();
                            self.queue.save_queue()?;
                        }
                        KeyCode::Char('H') => {
                            self.screen.next();
                        }
                        KeyCode::Char('L') => {
                            self.screen.previous();
                        }
                        KeyCode::Char('j') => self.queue.queuelist_state().select_next(),
                        KeyCode::Char('k') => self.queue.queuelist_state().select_previous(),
                        KeyCode::Enter => self.player.play_video(&mut self.queue)?,
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

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.player.kill_mpv();
        self.running = false;
    }
}
