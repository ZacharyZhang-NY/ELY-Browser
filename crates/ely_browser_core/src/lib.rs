mod error;
mod navigation;
mod state;

pub use error::CoreError;
pub use state::{
    BrowserCore, BrowserSnapshot, InitialBrowserConfig, InstalledPlugin, PluginAuditAction,
    PluginAuditEvent,
};
