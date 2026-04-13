use crate::types::Video;
use ratatui::widgets::ListState;
use std::fs;

#[derive(Default, Debug)]
pub struct Queue {
    queuelist_path: String,
    queuelist: Vec<Video>,
    queuelist_state: ListState,
}

impl Queue {
    pub fn default() -> Self {
        // queuelist is saved here
        let queuelist_path = Self::init_queuelist_path();
        let queuelist = Vec::new();
        let queuelist_state = ListState::default().with_selected(Some(0));
        Self {
            queuelist_path,
            queuelist,
            queuelist_state,
        }
    }
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_to_queue(
        &mut self,
        resultlist: &[Video],
        resultlist_state: &ListState,
    ) -> color_eyre::Result<()> {
        if let Some(index) = resultlist_state.selected() {
            if resultlist.len() <= index {
                return Err(color_eyre::eyre::eyre!(
                    "Index out of bounds in {} at {} ",
                    file!(),
                    line!()
                ));
            };
            self.queuelist.push(resultlist[index].clone());
        }
        Ok(())
    }
    pub fn retrieve_queue(&mut self) -> color_eyre::Result<()> {
        if fs::exists(&self.queuelist_path)? {
            let queuelist = fs::read_to_string(&self.queuelist_path)?;
            self.queuelist = serde_json::from_str(queuelist.as_str())?;
        }
        Ok(())
    }
    pub fn save_queue(&self) -> color_eyre::Result<()> {
        if let Some((path, _filename)) = self.queuelist_path.rsplit_once("/") {
            fs::DirBuilder::new().recursive(true).create(path)?;
        }
        let queuelist_json = serde_json::to_string_pretty(&self.queuelist)?;
        fs::write(self.queuelist_path.clone(), queuelist_json)?;
        Ok(())
    }

    pub fn queuelist(&mut self) -> &mut Vec<Video> {
        &mut self.queuelist
    }
    pub fn queuelist_state(&mut self) -> &mut ListState {
        &mut self.queuelist_state
    }
    fn init_queuelist_path() -> String {
        match dirs::data_local_dir() {
            Some(mut path) => {
                path.push("ymp");
                path.push("queuelist.json");
                path.to_string_lossy().into_owned()
            }
            None => {
                // TODO Add error handling for queuelist path not accessible.
                String::from("Placeholder")
            }
        }
    }
}
