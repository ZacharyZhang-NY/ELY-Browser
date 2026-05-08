use std::collections::BTreeSet;

use semver::Version;
use serde::Deserialize;
use url::Url;

use crate::DomainError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginId(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginManifest {
    id: PluginId,
    name: String,
    description: String,
    author: String,
    homepage: String,
    permissions: Vec<PluginPermission>,
    contributes: Vec<PluginContributionPoint>,
    min_ely_build: Version,
    checksum: String,
    signature: PluginSignature,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginPermission {
    TabsRead,
    TabsWrite,
    SpacesRead,
    SpacesWrite,
    BookmarksRead,
    BookmarksWrite,
    HistoryRead,
    DownloadsRead,
    DownloadsWrite,
    PageMetadata,
    PageScreenshot,
    PageScript,
    ClipboardRead,
    ClipboardWrite,
    FilesystemRead,
    FilesystemWrite,
    NetworkFetch,
    SettingsRead,
    SettingsWrite,
    SyncPlugin,
    UiPanel,
    UiCommand,
    UiContextMenu,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginContributionPoint {
    CommandBarCommand,
    TabContextMenu,
    PageContextMenu,
    SidebarPanel,
    SettingsPage,
    StatusBarIndicator,
    DownloadAction,
    BookmarkAction,
    ReadingModeExporter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginSignatureAlgorithm {
    Ed25519,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginSignature {
    algorithm: PluginSignatureAlgorithm,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPluginManifest {
    id: String,
    name: String,
    description: String,
    author: String,
    homepage: String,
    permissions: Vec<String>,
    contributes: Vec<String>,
    min_ely_build: String,
    checksum: String,
    signature: RawPluginSignature,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPluginSignature {
    algorithm: String,
    value: String,
}

impl PluginId {
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if !is_valid_plugin_id(value) {
            return Err(DomainError::InvalidPluginId { value: value.to_string() });
        }

        Ok(Self(value.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PluginManifest {
    pub fn from_toml(value: &str) -> Result<Self, DomainError> {
        let raw: RawPluginManifest = toml::from_str(value)
            .map_err(|error| DomainError::InvalidPluginManifest { reason: error.to_string() })?;

        Self::from_raw(raw)
    }

    fn from_raw(raw: RawPluginManifest) -> Result<Self, DomainError> {
        Ok(Self {
            id: PluginId::parse(raw.id)?,
            name: non_empty_plugin_field("name", raw.name)?,
            description: non_empty_plugin_field("description", raw.description)?,
            author: non_empty_plugin_field("author", raw.author)?,
            homepage: plugin_homepage(raw.homepage)?,
            permissions: parse_unique_permissions(&raw.permissions)?,
            contributes: parse_unique_contributions(&raw.contributes)?,
            min_ely_build: parse_ely_build(raw.min_ely_build)?,
            checksum: plugin_checksum(raw.checksum)?,
            signature: PluginSignature::from_raw(raw.signature)?,
        })
    }

    #[must_use]
    pub fn id(&self) -> &PluginId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn author(&self) -> &str {
        &self.author
    }

    #[must_use]
    pub fn homepage(&self) -> &str {
        &self.homepage
    }

    #[must_use]
    pub fn permissions(&self) -> &[PluginPermission] {
        &self.permissions
    }

    #[must_use]
    pub fn contributes(&self) -> &[PluginContributionPoint] {
        &self.contributes
    }

    #[must_use]
    pub fn min_ely_build(&self) -> &Version {
        &self.min_ely_build
    }

    #[must_use]
    pub fn checksum(&self) -> &str {
        &self.checksum
    }

    #[must_use]
    pub fn signature(&self) -> &PluginSignature {
        &self.signature
    }
}

impl PluginPermission {
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "tabs:read" => Ok(Self::TabsRead),
            "tabs:write" => Ok(Self::TabsWrite),
            "spaces:read" => Ok(Self::SpacesRead),
            "spaces:write" => Ok(Self::SpacesWrite),
            "bookmarks:read" => Ok(Self::BookmarksRead),
            "bookmarks:write" => Ok(Self::BookmarksWrite),
            "history:read" => Ok(Self::HistoryRead),
            "downloads:read" => Ok(Self::DownloadsRead),
            "downloads:write" => Ok(Self::DownloadsWrite),
            "page:metadata" => Ok(Self::PageMetadata),
            "page:screenshot" => Ok(Self::PageScreenshot),
            "page:script" => Ok(Self::PageScript),
            "clipboard:read" => Ok(Self::ClipboardRead),
            "clipboard:write" => Ok(Self::ClipboardWrite),
            "filesystem:read" => Ok(Self::FilesystemRead),
            "filesystem:write" => Ok(Self::FilesystemWrite),
            "network:fetch" => Ok(Self::NetworkFetch),
            "settings:read" => Ok(Self::SettingsRead),
            "settings:write" => Ok(Self::SettingsWrite),
            "sync:plugin" => Ok(Self::SyncPlugin),
            "ui:panel" => Ok(Self::UiPanel),
            "ui:command" => Ok(Self::UiCommand),
            "ui:context_menu" => Ok(Self::UiContextMenu),
            _ => Err(DomainError::InvalidPluginPermission { value: value.to_string() }),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TabsRead => "tabs:read",
            Self::TabsWrite => "tabs:write",
            Self::SpacesRead => "spaces:read",
            Self::SpacesWrite => "spaces:write",
            Self::BookmarksRead => "bookmarks:read",
            Self::BookmarksWrite => "bookmarks:write",
            Self::HistoryRead => "history:read",
            Self::DownloadsRead => "downloads:read",
            Self::DownloadsWrite => "downloads:write",
            Self::PageMetadata => "page:metadata",
            Self::PageScreenshot => "page:screenshot",
            Self::PageScript => "page:script",
            Self::ClipboardRead => "clipboard:read",
            Self::ClipboardWrite => "clipboard:write",
            Self::FilesystemRead => "filesystem:read",
            Self::FilesystemWrite => "filesystem:write",
            Self::NetworkFetch => "network:fetch",
            Self::SettingsRead => "settings:read",
            Self::SettingsWrite => "settings:write",
            Self::SyncPlugin => "sync:plugin",
            Self::UiPanel => "ui:panel",
            Self::UiCommand => "ui:command",
            Self::UiContextMenu => "ui:context_menu",
        }
    }
}

impl PluginContributionPoint {
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "command-bar-command" => Ok(Self::CommandBarCommand),
            "tab-context-menu" => Ok(Self::TabContextMenu),
            "page-context-menu" => Ok(Self::PageContextMenu),
            "sidebar-panel" => Ok(Self::SidebarPanel),
            "settings-page" => Ok(Self::SettingsPage),
            "status-bar-indicator" => Ok(Self::StatusBarIndicator),
            "download-action" => Ok(Self::DownloadAction),
            "bookmark-action" => Ok(Self::BookmarkAction),
            "reading-mode-exporter" => Ok(Self::ReadingModeExporter),
            _ => Err(DomainError::InvalidPluginContribution { value: value.to_string() }),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CommandBarCommand => "command-bar-command",
            Self::TabContextMenu => "tab-context-menu",
            Self::PageContextMenu => "page-context-menu",
            Self::SidebarPanel => "sidebar-panel",
            Self::SettingsPage => "settings-page",
            Self::StatusBarIndicator => "status-bar-indicator",
            Self::DownloadAction => "download-action",
            Self::BookmarkAction => "bookmark-action",
            Self::ReadingModeExporter => "reading-mode-exporter",
        }
    }
}

impl PluginSignature {
    fn from_raw(raw: RawPluginSignature) -> Result<Self, DomainError> {
        let algorithm = PluginSignatureAlgorithm::parse(raw.algorithm.as_str())?;
        let value = plugin_signature_value(&algorithm, raw.value)?;
        Ok(Self { algorithm, value })
    }

    #[must_use]
    pub fn algorithm(&self) -> &PluginSignatureAlgorithm {
        &self.algorithm
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl PluginSignatureAlgorithm {
    fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "ed25519" => Ok(Self::Ed25519),
            _ => Err(DomainError::InvalidPluginSignatureAlgorithm { value: value.to_string() }),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
        }
    }
}

fn parse_unique_permissions(values: &[String]) -> Result<Vec<PluginPermission>, DomainError> {
    let mut seen = BTreeSet::new();
    let mut permissions = Vec::with_capacity(values.len());
    for value in values {
        let permission = PluginPermission::parse(value.as_str())?;
        if !seen.insert(permission.as_str()) {
            return Err(DomainError::DuplicatePluginPermission {
                value: permission.as_str().to_string(),
            });
        }
        permissions.push(permission);
    }
    Ok(permissions)
}

fn parse_unique_contributions(
    values: &[String],
) -> Result<Vec<PluginContributionPoint>, DomainError> {
    let mut seen = BTreeSet::new();
    let mut contributions = Vec::with_capacity(values.len());
    for value in values {
        let contribution = PluginContributionPoint::parse(value.as_str())?;
        if !seen.insert(contribution.as_str()) {
            return Err(DomainError::DuplicatePluginContribution {
                value: contribution.as_str().to_string(),
            });
        }
        contributions.push(contribution);
    }
    Ok(contributions)
}

fn non_empty_plugin_field(
    field: &'static str,
    value: impl Into<String>,
) -> Result<String, DomainError> {
    let value = value.into();
    let value = value.trim();
    if value.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    Ok(value.to_string())
}

fn plugin_homepage(value: impl Into<String>) -> Result<String, DomainError> {
    let value = non_empty_plugin_field("homepage", value)?;
    let parsed = Url::parse(value.as_str())
        .map_err(|_| DomainError::InvalidPluginHomepage { value: value.clone() })?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(DomainError::InvalidPluginHomepage { value });
    }
    Ok(parsed.to_string())
}

fn parse_ely_build(value: impl Into<String>) -> Result<Version, DomainError> {
    let value = non_empty_plugin_field("min_ely_build", value)?;
    Version::parse(value.as_str()).map_err(|_| DomainError::InvalidPluginBuildRequirement { value })
}

fn plugin_checksum(value: impl Into<String>) -> Result<String, DomainError> {
    let value = non_empty_plugin_field("checksum", value)?;
    if !is_hex_of_len(value.as_str(), 64) {
        return Err(DomainError::InvalidPluginChecksum { value });
    }
    Ok(value.to_ascii_lowercase())
}

fn plugin_signature_value(
    algorithm: &PluginSignatureAlgorithm,
    value: impl Into<String>,
) -> Result<String, DomainError> {
    let value = non_empty_plugin_field("signature", value)?;
    let valid = match algorithm {
        PluginSignatureAlgorithm::Ed25519 => is_hex_of_len(value.as_str(), 128),
    };
    if !valid {
        return Err(DomainError::InvalidPluginSignature { value });
    }
    Ok(value.to_ascii_lowercase())
}

fn is_valid_plugin_id(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|ch| {
                    ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_'
                })
                && segment
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
                && segment
                    .chars()
                    .last()
                    .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        })
}

fn is_hex_of_len(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|ch| ch.is_ascii_hexdigit())
}
