use crate::app::Video;
use std::path::PathBuf;
use yt_dlp::extractor::Youtube;

fn yt_dlp_path() -> PathBuf {
    match which::which("yt-dlp").map_err(|_| "can't find yt-dlp in PATH") {
        Ok(path) => path,
        Err(_) => PathBuf::from("/usr/bin/yt-dlp"),
    }
}

pub async fn perform_search(query: String) -> color_eyre::Result<Vec<Video>> {
    let extractor = Youtube::new(yt_dlp_path());
    let options = extractor.search(&query, 25).await?;
    let mut videos: Vec<Video> = Vec::new();
    let mut v: Video = Video::default();
    for entry in options.entries {
        v.id = entry.id;
        v.title = entry.title;
        if let Some(a) = entry.uploader {
            v.uploader = a;
        };
        videos.push(v.clone());
    }
    Ok(videos)
}
