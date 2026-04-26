use std::fs;

use serde::Serialize;

#[derive(Serialize, Debug, Default)]
pub struct Settings {
    settings_path: String,
    browser: Option<String>,
}

impl Settings {
    pub fn default() -> Self {
        let settings_path = Self::init_settings_path();
        let browser = Option::default();
        Self {
            settings_path,
            browser,
        }
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn save(&self) -> color_eyre::Result<()> {
        if let Some((path, _filename)) = self.settings_path.rsplit_once("/") {
            fs::DirBuilder::new().recursive(true).create(path)?;
        }
        let settings_json = serde_json::to_string_pretty(&self)?;
        fs::write(self.settings_path.clone(), settings_json)?;
        Ok(())
    }
    fn init_settings_path() -> String {
        match dirs::data_local_dir() {
            Some(mut path) => {
                path.push("ymp");
                path.push("settings.json");
                path.to_string_lossy().into_owned()
            }
            None => {
                //TODO Add error handling for settings path not accessible
                String::from("Placeholder")
            }
        }
    }
}
