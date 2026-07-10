use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use ely_domain::ProfileId;
use thiserror::Error;

#[cfg(not(test))]
use super::servo_profile_data::{create_profile_data_dir, default_profile_data_root};

#[cfg(not(test))]
const DEFAULT_PROFILE_ID_FILE: &str = "profile-id";
#[cfg(not(test))]
const DEFAULT_PROFILE_DIRECTORY: &str = "default";

#[cfg(test)]
pub(crate) fn default_standard_profile_id() -> Result<ProfileId, ProfileIdentityError> {
    static TEST_PROFILE_ID: std::sync::OnceLock<ProfileId> = std::sync::OnceLock::new();
    Ok(TEST_PROFILE_ID.get_or_init(ProfileId::new).clone())
}

#[cfg(not(test))]
pub(crate) fn default_standard_profile_id() -> Result<ProfileId, ProfileIdentityError> {
    let root = default_profile_data_root().ok_or(ProfileIdentityError::DataRootUnavailable)?;
    let profile_id = load_or_create_profile_id(
        &root.join(DEFAULT_PROFILE_DIRECTORY).join(DEFAULT_PROFILE_ID_FILE),
    )?;
    create_profile_data_dir(&root, &profile_id)?;
    Ok(profile_id)
}

fn load_or_create_profile_id(path: &Path) -> Result<ProfileId, ProfileIdentityError> {
    match read_profile_id(path) {
        Ok(profile_id) => return Ok(profile_id),
        Err(ProfileIdentityError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let parent = path
        .parent()
        .ok_or_else(|| ProfileIdentityError::ParentUnavailable { path: path.to_path_buf() })?;
    create_private_directory(parent)?;
    let profile_id = ProfileId::new();
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(profile_id.as_str().as_bytes())?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    match file.persist_noclobber(path) {
        Ok(_) => {
            sync_parent_directory(parent)?;
            Ok(profile_id)
        }
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => read_profile_id(path),
        Err(error) => Err(error.error.into()),
    }
}

fn create_private_directory(path: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), io::Error> {
    fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), io::Error> {
    Ok(())
}

fn read_profile_id(path: &Path) -> Result<ProfileId, ProfileIdentityError> {
    let raw = fs::read_to_string(path)?;
    ProfileId::parse(raw.trim()).map_err(ProfileIdentityError::Domain)
}

#[derive(Debug, Error)]
pub(crate) enum ProfileIdentityError {
    #[cfg(not(test))]
    #[error("profile data root is unavailable")]
    DataRootUnavailable,

    #[error("profile identity parent directory is unavailable for {path}")]
    ParentUnavailable { path: PathBuf },

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Domain(#[from] ely_domain::DomainError),
}

#[cfg(test)]
mod tests {
    use super::load_or_create_profile_id;
    use crate::services::servo_profile_data::profile_data_dir;
    use ely_browser_core::{BrowserCore, InitialBrowserConfig};

    #[test]
    fn default_profile_identity_survives_restart() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("profiles");
        let path = root.join("default").join("profile-id");

        let first_start = load_or_create_profile_id(&path)?;
        let second_start = load_or_create_profile_id(&path)?;
        let mut first_config = InitialBrowserConfig::ely_defaults()?;
        first_config.profile_id = Some(first_start.clone());
        let first_core = BrowserCore::new(first_config)?;
        let mut second_config = InitialBrowserConfig::ely_defaults()?;
        second_config.profile_id = Some(second_start.clone());
        let second_core = BrowserCore::new(second_config)?;

        assert_eq!(first_start, second_start);
        assert_eq!(std::fs::read_to_string(&path)?.trim(), first_start.as_str());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path.parent().ok_or("missing marker parent")?)?
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        assert_eq!(first_core.active_tab()?.profile_id(), second_core.active_tab()?.profile_id());
        assert_eq!(
            profile_data_dir(&root, first_core.active_tab()?.profile_id()),
            profile_data_dir(&root, second_core.active_tab()?.profile_id()),
        );
        Ok(())
    }

    #[test]
    fn concurrent_starts_converge_on_one_profile_identity() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let path = std::sync::Arc::new(directory.path().join("default").join("profile-id"));
        let threads = (0..4)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || load_or_create_profile_id(&path))
            })
            .collect::<Vec<_>>();
        let mut identities = Vec::new();
        for thread in threads {
            identities.push(
                thread
                    .join()
                    .map_err(|_| std::io::Error::other("profile identity thread panicked"))??,
            );
        }

        assert!(identities.windows(2).all(|pair| pair[0] == pair[1]));
        Ok(())
    }
}
