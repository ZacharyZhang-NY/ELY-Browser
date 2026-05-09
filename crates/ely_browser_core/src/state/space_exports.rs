use ely_domain::{
    ArchivePolicy, BrowserTab, ProfileId, ProfileKind, Space, SpaceId, TabId, UrlText,
};
use serde::{Deserialize, Serialize};

use super::BrowserCore;
use crate::CoreError;

pub const ELYSPACE_SCHEMA_VERSION: u16 = 1;
pub const ELYSPACE_FILE_EXTENSION: &str = "elyspace";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpaceImportProfileMapping {
    PreserveExisting,
    UseActiveProfile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ElySpacePackage {
    version: u16,
    space: ElySpaceRecord,
    tabs: Vec<ElySpaceTabRecord>,
}

impl ElySpacePackage {
    #[must_use]
    pub fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub fn space_name(&self) -> &str {
        &self.space.name
    }

    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ElySpaceRecord {
    name: String,
    icon: String,
    accent_hex: u32,
    default_profile_id: String,
    archive_policy: ElySpaceArchivePolicy,
    sidebar_width_px: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ElySpaceTabRecord {
    title: String,
    url: String,
    profile_id: String,
    favicon_key: Option<String>,
    pinned: bool,
    favorite: bool,
    sort_key: u64,
    sync_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ElySpaceArchivePolicy {
    Manual,
    IdleDays { days: u16 },
}

impl BrowserCore {
    pub fn export_space_package_json(&self, space_id: &SpaceId) -> Result<String, CoreError> {
        serde_json::to_string_pretty(&self.export_space_package(space_id)?)
            .map_err(invalid_space_package)
    }

    pub fn export_space_package(&self, space_id: &SpaceId) -> Result<ElySpacePackage, CoreError> {
        let space = self
            .spaces
            .iter()
            .find(|space| space.id() == space_id)
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })?;
        let tabs = self
            .tabs
            .iter()
            .filter(|tab| tab.space_id() == space_id)
            .map(ElySpaceTabRecord::from_tab)
            .collect();

        Ok(ElySpacePackage {
            version: ELYSPACE_SCHEMA_VERSION,
            space: ElySpaceRecord::from_space(space),
            tabs,
        })
    }

    pub fn import_space_package_json(
        &mut self,
        package_json: &str,
        profile_mapping: SpaceImportProfileMapping,
    ) -> Result<SpaceId, CoreError> {
        let package = serde_json::from_str(package_json).map_err(invalid_space_package)?;
        self.import_space_package(package, profile_mapping)
    }

    pub fn import_space_package(
        &mut self,
        package: ElySpacePackage,
        profile_mapping: SpaceImportProfileMapping,
    ) -> Result<SpaceId, CoreError> {
        validate_package_version(package.version)?;
        self.validate_imported_favorites(&package)?;
        let default_profile_id = self.imported_default_profile_id(&package, profile_mapping)?;
        let sort_key = self.next_space_sort_key();
        let mut space = Space::new(
            package.space.name.clone(),
            package.space.icon.clone(),
            package.space.accent_hex,
            default_profile_id.clone(),
            sort_key,
        );
        space.set_archive_policy(package.space.archive_policy.clone().into());
        space.set_sidebar_width_px(package.space.sidebar_width_px);

        let space_id = space.id().clone();
        let tabs = self.imported_tabs(&space_id, &default_profile_id, &package, profile_mapping)?;
        let active_tab_id = tabs[0].id().clone();

        self.spaces.push(space);
        self.tabs.extend(tabs);
        self.active_tabs_by_space.insert(space_id.clone(), active_tab_id.clone());
        self.select_tab(&active_tab_id)?;
        Ok(space_id)
    }

    fn imported_default_profile_id(
        &self,
        package: &ElySpacePackage,
        profile_mapping: SpaceImportProfileMapping,
    ) -> Result<ProfileId, CoreError> {
        match profile_mapping {
            SpaceImportProfileMapping::PreserveExisting => {
                let profile_id = ProfileId::parse(&package.space.default_profile_id)?;
                self.standard_profile_id(&profile_id)?;
                Ok(profile_id)
            }
            SpaceImportProfileMapping::UseActiveProfile => self.stable_active_profile_id(),
        }
    }

    fn validate_imported_favorites(&self, package: &ElySpacePackage) -> Result<(), CoreError> {
        let imported_favorites = package.tabs.iter().filter(|tab| tab.favorite).count();
        let favorite_limit = self.favorite_limit.value();
        if self.favorites().len() + imported_favorites > favorite_limit {
            return Err(CoreError::FavoriteLimitReached { limit: favorite_limit });
        }
        Ok(())
    }

    fn imported_tabs(
        &self,
        space_id: &SpaceId,
        default_profile_id: &ProfileId,
        package: &ElySpacePackage,
        profile_mapping: SpaceImportProfileMapping,
    ) -> Result<Vec<BrowserTab>, CoreError> {
        let mut tabs = package
            .tabs
            .iter()
            .map(|tab| self.imported_tab(space_id, default_profile_id, tab, profile_mapping))
            .collect::<Result<Vec<_>, _>>()?;

        if tabs.is_empty() {
            tabs.push(self.build_tab_for(
                space_id.clone(),
                default_profile_id.clone(),
                self.new_tab_url()?,
            ));
        }
        Ok(tabs)
    }

    fn imported_tab(
        &self,
        space_id: &SpaceId,
        default_profile_id: &ProfileId,
        record: &ElySpaceTabRecord,
        profile_mapping: SpaceImportProfileMapping,
    ) -> Result<BrowserTab, CoreError> {
        let profile_id = match profile_mapping {
            SpaceImportProfileMapping::PreserveExisting => {
                let profile_id = ProfileId::parse(&record.profile_id)?;
                self.standard_profile_id(&profile_id)?;
                profile_id
            }
            SpaceImportProfileMapping::UseActiveProfile => default_profile_id.clone(),
        };
        let mut tab = BrowserTab::new(
            TabId::new(),
            space_id.clone(),
            profile_id,
            record.title.clone(),
            UrlText::parse(&record.url)?,
        )
        .with_sort_key(record.sort_key);
        tab.set_pinned(record.pinned);
        tab.set_favorite(record.favorite);
        tab.set_sync_enabled(record.sync_enabled);
        if let Some(favicon_key) = &record.favicon_key {
            tab.set_favicon_key(favicon_key.clone())?;
        }
        Ok(tab)
    }

    fn standard_profile_id(&self, profile_id: &ProfileId) -> Result<(), CoreError> {
        let profile = self
            .profiles
            .iter()
            .find(|profile| profile.id() == profile_id)
            .ok_or_else(|| CoreError::ProfileNotFound { id: profile_id.clone() })?;
        if profile.kind() == &ProfileKind::Private {
            return Err(CoreError::PrivateProfileDefaultLocked { id: profile_id.clone() });
        }
        Ok(())
    }

    fn stable_active_profile_id(&self) -> Result<ProfileId, CoreError> {
        if self.active_profile()?.kind() == &ProfileKind::Private {
            Ok(self.active_space()?.default_profile_id().clone())
        } else {
            Ok(self.active_profile_id.clone())
        }
    }
}

impl ElySpaceRecord {
    fn from_space(space: &Space) -> Self {
        Self {
            name: space.name().to_string(),
            icon: space.icon().to_string(),
            accent_hex: space.accent_hex(),
            default_profile_id: space.default_profile_id().as_str().to_string(),
            archive_policy: ElySpaceArchivePolicy::from(space.archive_policy()),
            sidebar_width_px: space.sidebar_width_px(),
        }
    }
}

impl ElySpaceTabRecord {
    fn from_tab(tab: &BrowserTab) -> Self {
        Self {
            title: tab.title().to_string(),
            url: tab.url().as_str().to_string(),
            profile_id: tab.profile_id().as_str().to_string(),
            favicon_key: tab.favicon_key().map(str::to_string),
            pinned: tab.flags().pinned,
            favorite: tab.flags().favorite,
            sort_key: tab.sort_key(),
            sync_enabled: tab.sync_enabled(),
        }
    }
}

impl From<&ArchivePolicy> for ElySpaceArchivePolicy {
    fn from(policy: &ArchivePolicy) -> Self {
        match policy {
            ArchivePolicy::Manual => Self::Manual,
            ArchivePolicy::IdleDays(days) => Self::IdleDays { days: *days },
        }
    }
}

impl From<ElySpaceArchivePolicy> for ArchivePolicy {
    fn from(policy: ElySpaceArchivePolicy) -> Self {
        match policy {
            ElySpaceArchivePolicy::Manual => Self::Manual,
            ElySpaceArchivePolicy::IdleDays { days } => Self::IdleDays(days),
        }
    }
}

fn validate_package_version(version: u16) -> Result<(), CoreError> {
    if version == ELYSPACE_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(CoreError::InvalidSpacePackage { reason: format!("unsupported version {version}") })
    }
}

fn invalid_space_package(error: serde_json::Error) -> CoreError {
    CoreError::InvalidSpacePackage { reason: error.to_string() }
}
