use crate::types::Video;
use std::path::PathBuf;
use tokio::sync::mpsc;
use yt_dlp::extractor::Youtube;

#[derive(Default, Debug)]
pub struct Search {
    // tokio  search-related stuff
    search_is_loading: bool, // In-case I want to add a leading screen
    search_rx: Option<mpsc::UnboundedReceiver<color_eyre::Result<Vec<Video>>>>, //receives search results
    yt_dlp_path: PathBuf,
    search_query: String,
}

impl Search {
    pub fn default() -> Self {
        let search_is_loading = false;
        let search_rx = None;
        // Paths
        // tries to find yt-dlp path e.g. /usr/bin/yt-dlp
        // FIX: Install yt_dlp if path not found. HINT: Change ERR()

        let yt_dlp_path = match which::which("yt-dlp").map_err(|_| "can't find yt-dlp in PATH") {
            Ok(path) => path,
            Err(_) => PathBuf::from("/usr/bin/yt-dlp"),
        };
        let search_query = String::new();
        Self {
            search_is_loading,
            search_rx,
            yt_dlp_path,
            search_query,
        }
    }

    pub fn search(&mut self, resultlist: &mut Vec<Video>, search_query: String) {
        self.search_is_loading = true;

        resultlist.clear();
        let (tx, rx) = mpsc::unbounded_channel();
        self.search_rx = Some(rx);

        self.search_query = search_query.clone();
        let yt_dlp_path = self.yt_dlp_path.clone();

        tokio::spawn(async move {
            let out = perform_search(yt_dlp_path, search_query).await;
            let _ = tx.send(out);
        });

        // self.search_is_loading is set to false in check_search_results for obvious reasons. (Because
        // search_is_loading doesn't stop until check search results is completed)
    }

    pub fn check_search_results(&mut self) -> std::io::Result<Vec<Video>> {
        if let Some(rx) = &mut self.search_rx {
            match rx.try_recv() {
                Ok(Ok(videos)) => {
                    self.search_is_loading = false;
                    self.search_rx = None;
                    return Ok(videos);
                }
                Ok(Err(e)) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Search returned no videos. Error: {}", e),
                    ));
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // Might still be loading. Don't do anything here.
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Search came back Empty. Not an issue.",
                    ));
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Died unexpectedly
                    self.search_is_loading = false;
                    self.search_rx = None;
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "Search failed. Receiver Disconnected.",
                    ));
                }
            }
        }
        Err(std::io::Error::other(format!(
            "Unidentified Error: {}: line: {}",
            file!(),
            line!()
        )))
    }
}
async fn perform_search(
    yt_dlp_path: PathBuf,
    search_query: String,
) -> color_eyre::Result<Vec<Video>> {
    let extractor = Youtube::new(yt_dlp_path);
    let options = extractor.search(&search_query, 25).await?;
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
