mod error;
mod navigation;
mod state;
mod sync_engine;

pub use error::CoreError;
pub use state::{
    BookmarkImportSummary, BrowserCore, BrowserSnapshot, ELYBOOKMARKS_FILE_EXTENSION,
    ELYBOOKMARKS_SCHEMA_VERSION, ELYDATA_FILE_EXTENSION, ELYDATA_SCHEMA_VERSION,
    ELYSPACE_FILE_EXTENSION, ELYSPACE_SCHEMA_VERSION, ElyBookmarksPackage, ElyLocalDataPackage,
    ElySpacePackage, InitialBrowserConfig, InstalledPlugin, LocalDataInventory, PluginAuditAction,
    PluginAuditEvent, SiteDataClearance, SpaceImportProfileMapping, TrashedSpace,
};
pub use sync_engine::{SyncEngine, SyncEngineBuilder, SyncOutcome};
