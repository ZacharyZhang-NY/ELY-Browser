#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HistoryRecordingPolicy {
    #[default]
    Record,
    Pause,
}

impl HistoryRecordingPolicy {
    pub const ALL: &[Self] = &[Self::Record, Self::Pause];

    #[must_use]
    pub fn records_history(self) -> bool {
        matches!(self, Self::Record)
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Record => "Record History",
            Self::Pause => "Pause History",
        }
    }

    #[must_use]
    pub fn detail(self) -> &'static str {
        match self {
            Self::Record => "Save new web visits for Standard Profiles.",
            Self::Pause => "Skip new history entries for Standard Profiles.",
        }
    }

    #[must_use]
    pub fn status(self) -> &'static str {
        match self {
            Self::Record => "History recording is on",
            Self::Pause => "History recording is paused",
        }
    }
}
