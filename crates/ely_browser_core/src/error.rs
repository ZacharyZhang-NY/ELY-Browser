use ely_domain::{
    BookmarkId, DomainError, DownloadId, NoteId, PluginId, ProfileId, ReadingListId, SpaceId,
    SplitId, TabGroupId, TabId,
};
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CoreError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("tab not found: {id}")]
    TabNotFound { id: TabId },

    #[error("bookmark not found: {id}")]
    BookmarkNotFound { id: BookmarkId },

    #[error("space not found: {id}")]
    SpaceNotFound { id: SpaceId },

    #[error("split not found: {id}")]
    SplitNotFound { id: SplitId },

    #[error("tab group not found: {id}")]
    TabGroupNotFound { id: TabGroupId },

    #[error("profile not found: {id}")]
    ProfileNotFound { id: ProfileId },

    #[error("download not found: {id}")]
    DownloadNotFound { id: DownloadId },

    #[error("reading list entry not found: {id}")]
    ReadingListEntryNotFound { id: ReadingListId },

    #[error("note not found: {id}")]
    NoteNotFound { id: NoteId },

    #[error("download target path is unavailable: {id}")]
    DownloadTargetPathUnavailable { id: DownloadId },

    #[error("plugin already installed: {id}")]
    PluginAlreadyInstalled { id: PluginId },

    #[error("plugin requires high-risk permission confirmation: {id}")]
    PluginHighRiskConfirmationRequired { id: PluginId },

    #[error("plugin not found: {id}")]
    PluginNotFound { id: PluginId },

    #[error("favorite limit reached: {limit}")]
    FavoriteLimitReached { limit: usize },

    #[error("split pane limit reached: {limit}")]
    SplitPaneLimitReached { limit: usize },

    #[error("browser state has no archived tabs")]
    NoArchivedTabs,

    #[error("browser state has no active tab")]
    MissingActiveTab,
}
