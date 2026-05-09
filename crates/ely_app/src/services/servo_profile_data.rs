use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use ely_domain::ProfileId;

const ELY_QUALIFIER: &str = "com";
const ELY_ORGANIZATION: &str = "elydora";
const ELY_APPLICATION: &str = "ELY Browser";

pub(super) fn default_profile_data_root() -> Option<PathBuf> {
    ProjectDirs::from(ELY_QUALIFIER, ELY_ORGANIZATION, ELY_APPLICATION)
        .map(|project_dirs| project_dirs.data_dir().join("profiles"))
}

pub(super) fn profile_data_dir(profile_data_root: &Path, profile_id: &ProfileId) -> PathBuf {
    profile_data_root.join(profile_id.as_str()).join("servo")
}
