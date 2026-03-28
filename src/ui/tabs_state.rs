#[derive(Debug, Default)]
pub struct TabsState {
    titles: Vec<String>,
    selected: usize,
}

impl TabsState {
    pub fn new(titles: Vec<String>) -> Self {
        Self {
            titles,
            selected: 0,
        }
    }

    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % self.titles.len();
    }

    pub fn previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else {
            self.selected = self.titles.len() - 1;
        }
    }

    pub fn select(&mut self, index: usize) {
        self.selected = index;
    }

    pub fn selected(&self) -> usize {
        self.selected
    }
}
