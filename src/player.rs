use std::{
    fs,
    io::{ErrorKind, Write},
    os::unix::net::UnixStream,
    process::{Child, Command, Stdio},
};

use crate::queue::Queue;
use crate::types::{PlaybackMode, Video};

#[derive(Default, Debug)]
pub struct Player {
    playback_mode: PlaybackMode,
    mpv_process: Option<Child>,
    mpv_stream: Option<UnixStream>,
    mpv_connect_attempts: i8,
    now_playing: Video,
    is_nowplaying: bool,
}

impl Player {
    pub fn default() -> Self {
        let playback_mode = PlaybackMode::Audio;
        let mpv_process: Option<Child> = None;
        let mpv_stream: Option<UnixStream> = None;
        let mpv_connect_attempts = 0;
        let now_playing: Video = Video::default();
        let is_nowplaying = false;
        Self {
            playback_mode,
            mpv_process,
            mpv_stream,
            mpv_connect_attempts,
            now_playing,
            is_nowplaying,
        }
    }
    pub fn new() -> Self {
        Self::default()
    }

    pub fn now_playing(&self) -> &Video {
        &self.now_playing
    }

    pub fn is_nowplaying(&self) -> &bool {
        &self.is_nowplaying
    }

    pub fn playback_mode(&self) -> &PlaybackMode {
        &self.playback_mode
    }

    pub fn playback_mode_switch(&mut self) {
        if self.playback_mode == PlaybackMode::Audio {
            self.playback_mode = PlaybackMode::Video;
        } else {
            self.playback_mode = PlaybackMode::Audio;
        }
    }

    pub fn play_pause(&mut self) -> color_eyre::Result<()> {
        self.send_mpv_command(vec!["cycle", "pause"])
    }

    pub fn stop(&mut self) -> color_eyre::Result<()> {
        self.kill_mpv();
        self.is_nowplaying = false;
        Ok(())
    }

    pub fn increase_volume(&mut self) -> color_eyre::Result<()> {
        self.send_mpv_command(vec!["add", "volume", "5"])
    }

    pub fn decrease_volume(&mut self) -> color_eyre::Result<()> {
        self.send_mpv_command(vec!["add", "volume", "-5"])
    }

    pub fn get_current_volume(&mut self) -> color_eyre::Result<()> {
        self.send_mpv_command(vec!["get_property", "volume"])
    }

    pub fn play_video(&mut self, queue: &mut Queue) -> color_eyre::Result<()> {
        if *self.is_nowplaying() {
            self.kill_mpv();
        }

        self.is_nowplaying = true;

        if let Some(index) = queue.queuelist_state().selected() {
            if queue.queuelist().len() <= index {
                return Err(color_eyre::eyre::eyre!(
                    "Index out of bounds in {} at {} ",
                    file!(),
                    line!()
                ));
            };
            self.now_playing = queue.queuelist()[index].clone();
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

        // amount of times it tries to connect to mpv socket.
        self.mpv_connect_attempts = 10;

        Ok(())
    }

    pub fn play_video_url(&mut self, url: String) -> color_eyre::Result<()> {
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

    pub fn try_connect_mpv(&mut self) {
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
    pub fn kill_mpv(&mut self) {
        self.mpv_stream.take();

        if let Some(mut child) = self.mpv_process.take() {
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
    fn send_mpv_command(&mut self, args: Vec<&str>) -> color_eyre::Result<()> {
        if self.mpv_stream.is_none() {
            eprintln!("Can't send controls mpv stream is not even connected yet. wait a bit.");
            return Ok(());
        }
        let mut vec_args: Vec<String> = Vec::new();
        for arg in args {
            vec_args.push(format!("\"{}\"", arg));
        }
        let json_args = vec_args.join(",");
        let message = format!("{{\"command\": [{json_args}]}}\n");
        if let Some(mut stream) = self.mpv_stream.take()
            && let Err(e) = stream.write_all(message.as_bytes())
        {
            return Err(color_eyre::eyre::eyre!(
                "Could not write to UnixStream at send_mpv_command(): {e} "
            ));
        }
        Ok(())
    }
}
