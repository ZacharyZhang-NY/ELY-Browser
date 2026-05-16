use std::{
    env,
    path::{Path, PathBuf},
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use directories::ProjectDirs;
use ely_domain::{ProfileId, ProfileKind};

const ELY_QUALIFIER: &str = "com";
const ELY_ORGANIZATION: &str = "elydora";
const ELY_APPLICATION: &str = "ELY Browser";
const DEFAULT_STANDARD_PROFILE_DIR: &str = "default";

pub(crate) fn default_profile_data_root() -> Option<PathBuf> {
    ProjectDirs::from(ELY_QUALIFIER, ELY_ORGANIZATION, ELY_APPLICATION)
        .map(|project_dirs| project_dirs.data_dir().join("profiles"))
}

pub(crate) fn profile_data_dir(profile_data_root: &Path, profile_id: &ProfileId) -> PathBuf {
    profile_data_root.join(profile_id.as_str()).join("servo")
}

pub(crate) fn sync_profile_data_dir(
    profile_data_root: &Path,
    profile_id: &ProfileId,
    profile_name: &str,
    profile_kind: &ProfileKind,
) -> PathBuf {
    if profile_name == "Default" && matches!(profile_kind, ProfileKind::Standard) {
        return profile_data_root.join(DEFAULT_STANDARD_PROFILE_DIR).join("servo");
    }
    profile_data_dir(profile_data_root, profile_id)
}

pub(crate) fn transient_profile_data_dir(
    profile_id: &ProfileId,
) -> Result<PathBuf, SystemTimeError> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(env::temp_dir().join("ely-browser-servo-profiles").join(format!(
        "{}-{}-{timestamp}",
        std::process::id(),
        profile_id.as_str()
    )))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProfileDataMode {
    Persistent,
    Transient,
}

#[cfg(test)]
mod tests {
    use super::{profile_data_dir, sync_profile_data_dir};
    use ely_domain::{ProfileId, ProfileKind};

    #[test]
    fn default_standard_sync_profile_dir_is_stable() {
        let root = std::path::Path::new("/profiles");
        let first_id = ProfileId::new();
        let second_id = ProfileId::new();

        assert_eq!(
            sync_profile_data_dir(root, &first_id, "Default", &ProfileKind::Standard),
            sync_profile_data_dir(root, &second_id, "Default", &ProfileKind::Standard)
        );
    }

    #[test]
    fn custom_sync_profile_dir_keeps_profile_identity() {
        let root = std::path::Path::new("/profiles");
        let profile_id = ProfileId::new();

        assert_eq!(
            sync_profile_data_dir(root, &profile_id, "Personal", &ProfileKind::Standard),
            profile_data_dir(root, &profile_id)
        );
    }
}
