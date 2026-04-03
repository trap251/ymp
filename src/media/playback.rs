use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{ErrorKind, Write},
    os::unix::net::UnixStream,
    process::Child,
};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub uploader: String,
    // duration: f64,
}

impl Video {
    pub fn play_pause(mpv_stream: &mut Option<UnixStream>) -> color_eyre::Result<()> {
        Video::send_mpv_command(mpv_stream, vec!["cycle", "pause"])
    }
    pub fn stop(
        is_nowplaying: &mut bool,
        mpv_stream: &mut Option<UnixStream>,
        mpv_process: &mut Option<Child>,
    ) -> color_eyre::Result<()> {
        *is_nowplaying = false;
        Video::kill_mpv(mpv_stream, mpv_process);
        Ok(())
    }
    pub fn increase_volume(mpv_stream: &mut Option<UnixStream>) -> color_eyre::Result<()> {
        Video::send_mpv_command(mpv_stream, vec!["add", "volume", "5"])
    }
    pub fn decrease_volume(mpv_stream: &mut Option<UnixStream>) -> color_eyre::Result<()> {
        Video::send_mpv_command(mpv_stream, vec!["add", "volume", "-5"])
    }
    pub fn get_current_volume(mpv_stream: &mut Option<UnixStream>) -> color_eyre::Result<()> {
        Video::send_mpv_command(mpv_stream, vec!["get_property", "volume"])
    }

    fn send_mpv_command(
        mpv_stream: &mut Option<UnixStream>,
        args: Vec<&str>,
    ) -> color_eyre::Result<()> {
        let mut vec_args: Vec<String> = Vec::new();
        for arg in args {
            vec_args.push(format!("\"{}\"", arg));
        }
        let json_args = vec_args.join(",");
        let message = format!("{{\"command\": [{json_args}]}}\n");
        if let Some(stream) = mpv_stream
            && let Err(e) = stream.write_all(message.as_bytes())
        {
            eprintln!("Could not write to UnixStream at send_mpv_command(): {e} ");
        }
        Ok(())
    }

    pub fn kill_mpv(mpv_stream: &mut Option<UnixStream>, mpv_process: &mut Option<Child>) {
        mpv_stream.take();

        if let Some(child) = mpv_process {
            if let Err(e) = child.kill() {
                eprintln!("Could not kill mpv child process, call idf: {e}");
            }
            if let Err(e) = child.wait() {
                eprintln!("Could not wait on mpv child process: {e}");
            }
        }
        *mpv_process = None;
        if let Err(e) = fs::remove_file("/tmp/mpv-socket")
            && e.kind() != ErrorKind::NotFound
        {
            eprintln!("Could not remove /tmp/mpv-socket file: {e}");
        }
    }
}
