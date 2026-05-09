use ely_domain::{PluginContributionPoint, PluginPermission};

pub(crate) fn permission_scope_label(permission: &PluginPermission) -> &'static str {
    match permission {
        PluginPermission::TabsRead => "Reads tab metadata.",
        PluginPermission::TabsWrite => "Creates, moves, or closes tabs.",
        PluginPermission::SpacesRead => "Reads Space metadata.",
        PluginPermission::SpacesWrite => "Creates or changes Spaces.",
        PluginPermission::BookmarksRead => "Reads bookmarks.",
        PluginPermission::BookmarksWrite => "Writes bookmarks.",
        PluginPermission::HistoryRead => "Reads browsing history.",
        PluginPermission::DownloadsRead => "Reads download entries.",
        PluginPermission::DownloadsWrite => "Controls downloads.",
        PluginPermission::PageMetadata => "Reads active page metadata.",
        PluginPermission::PageScreenshot => "Captures page screenshots.",
        PluginPermission::PageScript => "Runs scoped page scripts.",
        PluginPermission::ClipboardRead => "Reads clipboard content.",
        PluginPermission::ClipboardWrite => "Writes clipboard content.",
        PluginPermission::FilesystemRead => "Reads user-selected files.",
        PluginPermission::FilesystemWrite => "Writes user-selected files.",
        PluginPermission::NetworkFetch => "Performs plugin network requests.",
        PluginPermission::SettingsRead => "Reads plugin settings.",
        PluginPermission::SettingsWrite => "Writes plugin settings.",
        PluginPermission::SyncPlugin => "Syncs plugin configuration.",
        PluginPermission::UiPanel => "Registers sidebar panels.",
        PluginPermission::UiCommand => "Registers command bar actions.",
        PluginPermission::UiContextMenu => "Registers context menu actions.",
    }
}

pub(crate) fn contribution_title(contribution: &PluginContributionPoint) -> &'static str {
    match contribution {
        PluginContributionPoint::CommandBarCommand => "Command bar action",
        PluginContributionPoint::TabContextMenu => "Tab context menu",
        PluginContributionPoint::PageContextMenu => "Page context menu",
        PluginContributionPoint::SidebarPanel => "Sidebar panel",
        PluginContributionPoint::SettingsPage => "Settings page",
        PluginContributionPoint::StatusBarIndicator => "Status bar indicator",
        PluginContributionPoint::DownloadAction => "Download action",
        PluginContributionPoint::BookmarkAction => "Bookmark action",
        PluginContributionPoint::ReadingModeExporter => "Reading mode exporter",
    }
}

pub(crate) fn contribution_detail(contribution: &PluginContributionPoint) -> &'static str {
    match contribution {
        PluginContributionPoint::CommandBarCommand => "Adds entries to the ⌘K command switcher.",
        PluginContributionPoint::TabContextMenu => "Adds tab right-click actions.",
        PluginContributionPoint::PageContextMenu => "Adds page right-click actions.",
        PluginContributionPoint::SidebarPanel => "Registers a custom sidebar panel.",
        PluginContributionPoint::SettingsPage => "Registers a settings page route.",
        PluginContributionPoint::StatusBarIndicator => "Renders an indicator in the status bar.",
        PluginContributionPoint::DownloadAction => "Adds actions to download entries.",
        PluginContributionPoint::BookmarkAction => "Adds actions to bookmarks.",
        PluginContributionPoint::ReadingModeExporter => "Exports reading mode content elsewhere.",
    }
}
