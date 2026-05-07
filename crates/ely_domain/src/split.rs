use crate::{SplitId, TabId};

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
}

impl SplitLayout {
    #[must_use]
    pub fn new(axis: SplitAxis, panes: Vec<SplitPane>) -> Self {
        Self { id: SplitId::new(), axis, panes }
    }

    #[must_use]
    pub fn id(&self) -> &SplitId {
        &self.id
    }

    #[must_use]
    pub fn axis(&self) -> &SplitAxis {
        &self.axis
    }

    #[must_use]
    pub fn panes(&self) -> &[SplitPane] {
        &self.panes
    }
}
