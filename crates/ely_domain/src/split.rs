use crate::{SplitId, TabId};

pub const MAX_SPLIT_PANES: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
    Grid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitPane {
    tab_id: TabId,
    weight: u16,
}

impl SplitPane {
    #[must_use]
    pub fn new(tab_id: TabId, weight: u16) -> Self {
        Self { tab_id, weight }
    }

    #[must_use]
    pub fn tab_id(&self) -> &TabId {
        &self.tab_id
    }

    #[must_use]
    pub fn weight(&self) -> u16 {
        self.weight
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitLayout {
    id: SplitId,
    axis: SplitAxis,
    panes: Vec<SplitPane>,
    title: String,
    saved: bool,
}

impl SplitLayout {
    #[must_use]
    pub fn new(axis: SplitAxis, panes: Vec<SplitPane>) -> Self {
        Self { id: SplitId::new(), axis, panes, title: "Split View".to_string(), saved: false }
    }

    #[must_use]
    pub fn id(&self) -> &SplitId {
        &self.id
    }

    #[must_use]
    pub fn axis(&self) -> &SplitAxis {
        &self.axis
    }

    pub fn set_axis(&mut self, axis: SplitAxis) {
        self.axis = axis;
    }

    #[must_use]
    pub fn panes(&self) -> &[SplitPane] {
        &self.panes
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn saved(&self) -> bool {
        self.saved
    }

    #[must_use]
    pub fn contains_tab(&self, tab_id: &TabId) -> bool {
        self.panes.iter().any(|pane| pane.tab_id() == tab_id)
    }

    #[must_use]
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    pub fn add_pane(&mut self, pane: SplitPane) -> bool {
        if self.panes.len() >= MAX_SPLIT_PANES || self.contains_tab(pane.tab_id()) {
            return false;
        }

        self.panes.push(pane);
        true
    }

    pub fn remove_tab(&mut self, tab_id: &TabId) -> bool {
        let original_len = self.panes.len();
        self.panes.retain(|pane| pane.tab_id() != tab_id);
        self.panes.len() != original_len
    }

    pub fn save(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.saved = true;
    }
}
