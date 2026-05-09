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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DiagnosticsReportingPolicy {
    #[default]
    Minimal,
    Paused,
}

impl DiagnosticsReportingPolicy {
    pub const ALL: &[Self] = &[Self::Minimal, Self::Paused];

    #[must_use]
    pub fn reports_diagnostics(self) -> bool {
        matches!(self, Self::Minimal)
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Minimal => "Minimal Diagnostics",
            Self::Paused => "Paused Diagnostics",
        }
    }

    #[must_use]
    pub fn detail(self) -> &'static str {
        match self {
            Self::Minimal => {
                "Send startup, crash, WebView crash, sync error, plugin crash, and update result codes."
            }
            Self::Paused => "Keep diagnostics local for review and privacy exports.",
        }
    }

    #[must_use]
    pub fn status(self) -> &'static str {
        match self {
            Self::Minimal => "Diagnostics reporting is on",
            Self::Paused => "Diagnostics reporting is paused",
        }
    }
}
