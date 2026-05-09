use std::{
    env,
    path::{Path, PathBuf},
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

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

pub(super) fn transient_profile_data_dir(
    profile_id: &ProfileId,
) -> Result<PathBuf, SystemTimeError> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(env::temp_dir().join("ely-browser-servo-profiles").join(format!(
        "{}-{}-{timestamp}",
        std::process::id(),
        profile_id.as_str()
    )))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProfileDataMode {
    Persistent,
    Transient,
}
