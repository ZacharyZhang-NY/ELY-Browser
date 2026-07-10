use std::{
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use ely_domain::ProfileId;

const ELY_QUALIFIER: &str = "com";
const ELY_ORGANIZATION: &str = "elydora";
const ELY_APPLICATION: &str = "ELY Browser";
const TRANSIENT_PROFILE_ROOT: &str = "ely-browser-servo-profiles";
const TRANSIENT_PROFILE_PREFIX: &str = "profile-";
const TRANSIENT_PROFILE_LEASE: &str = ".lease";
const TRANSIENT_ROOT_LOCK: &str = ".lock";

pub(crate) struct TransientProfileDataDir {
    directory: Option<tempfile::TempDir>,
    lease: Option<File>,
    root: PathBuf,
    path: PathBuf,
}

impl TransientProfileDataDir {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn close(mut self) -> Result<(), io::Error> {
        self.remove()
    }

    fn remove(&mut self) -> Result<(), io::Error> {
        let Some(directory) = self.directory.take() else {
            return Ok(());
        };
        let path = directory.keep();
        let root_lock = match open_lock_file(&self.root.join(TRANSIENT_ROOT_LOCK)) {
            Ok(root_lock) => root_lock,
            Err(error) => {
                self.lease.take();
                return Err(error);
            }
        };
        if let Err(error) = lock_file(&root_lock) {
            self.lease.take();
            return Err(error);
        }
        self.lease.take();
        match std::fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Drop for TransientProfileDataDir {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

pub(crate) fn default_profile_data_root() -> Option<PathBuf> {
    ProjectDirs::from(ELY_QUALIFIER, ELY_ORGANIZATION, ELY_APPLICATION)
        .map(|project_dirs| project_dirs.data_dir().join("profiles"))
}

pub(crate) fn profile_data_dir(profile_data_root: &Path, profile_id: &ProfileId) -> PathBuf {
    profile_data_root.join(profile_id.as_str()).join("servo")
}

pub(crate) fn sync_profile_data_dir(profile_data_root: &Path, profile_id: &ProfileId) -> PathBuf {
    profile_data_dir(profile_data_root, profile_id)
}

pub(crate) fn create_profile_data_dir(
    profile_data_root: &Path,
    profile_id: &ProfileId,
) -> Result<PathBuf, io::Error> {
    let profile_dir = profile_data_root.join(profile_id.as_str());
    let servo_dir = profile_dir.join("servo");
    std::fs::create_dir_all(&servo_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&profile_dir, std::fs::Permissions::from_mode(0o700))?;
        std::fs::set_permissions(&servo_dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(servo_dir)
}

pub(crate) fn cleanup_stale_transient_profile_data_dirs() -> Result<(), io::Error> {
    cleanup_stale_transient_profile_data_dirs_at(&transient_profile_data_root()?)
}

pub(crate) fn transient_profile_data_dir(
    profile_id: &ProfileId,
) -> Result<TransientProfileDataDir, io::Error> {
    let root = transient_profile_data_root()?;
    create_private_directory(&root)?;
    let root_lock = open_lock_file(&root.join(TRANSIENT_ROOT_LOCK))?;
    lock_file(&root_lock)?;
    cleanup_stale_transient_profile_data_dirs_locked(&root, |path| std::fs::remove_dir_all(path))?;

    let directory = tempfile::Builder::new()
        .prefix(&format!("{TRANSIENT_PROFILE_PREFIX}{}-", profile_id.as_str()))
        .tempdir_in(&root)?;
    create_private_directory(directory.path())?;
    let lease = open_lock_file(&directory.path().join(TRANSIENT_PROFILE_LEASE))?;
    lock_file(&lease)?;
    let path = directory.path().to_path_buf();
    drop(root_lock);
    Ok(TransientProfileDataDir { directory: Some(directory), lease: Some(lease), root, path })
}

fn transient_profile_data_root() -> Result<PathBuf, io::Error> {
    let project_dirs = ProjectDirs::from(ELY_QUALIFIER, ELY_ORGANIZATION, ELY_APPLICATION)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "ELY project directory unavailable")
        })?;
    let base = project_dirs.runtime_dir().unwrap_or_else(|| project_dirs.cache_dir());
    Ok(base.join(TRANSIENT_PROFILE_ROOT))
}

fn cleanup_stale_transient_profile_data_dirs_at(root: &Path) -> Result<(), io::Error> {
    create_private_directory(root)?;
    let root_lock = open_lock_file(&root.join(TRANSIENT_ROOT_LOCK))?;
    lock_file(&root_lock)?;
    cleanup_stale_transient_profile_data_dirs_locked(root, |path| std::fs::remove_dir_all(path))
}

fn cleanup_stale_transient_profile_data_dirs_locked(
    root: &Path,
    remove_dir: impl Fn(&Path) -> Result<(), io::Error>,
) -> Result<(), io::Error> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if !file_type.is_dir()
            || !entry.file_name().to_string_lossy().starts_with(TRANSIENT_PROFILE_PREFIX)
        {
            continue;
        }
        let lease = match open_lock_file(&entry.path().join(TRANSIENT_PROFILE_LEASE)) {
            Ok(lease) => lease,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                remove_directory_if_present(&remove_dir, &entry.path())?;
                continue;
            }
            Err(error) => return Err(error),
        };
        if try_lock_file(&lease)? {
            remove_directory_if_present(&remove_dir, &entry.path())?;
        }
    }
    Ok(())
}

fn remove_directory_if_present(
    remove_dir: &impl Fn(&Path) -> Result<(), io::Error>,
    path: &Path,
) -> Result<(), io::Error> {
    match remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn open_lock_file(path: &Path) -> Result<File, io::Error> {
    OpenOptions::new().create(true).read(true).write(true).truncate(false).open(path)
}

fn lock_file(file: &File) -> Result<(), io::Error> {
    file.lock()
}

fn try_lock_file(file: &File) -> Result<bool, io::Error> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(std::fs::TryLockError::WouldBlock) => Ok(false),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

fn create_private_directory(path: &Path) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(path)?;
            if !metadata.file_type().is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("private directory path is not a directory: {}", path.display()),
                ));
            }
        }
        Err(error) => return Err(error),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProfileDataMode {
    Persistent,
    Transient,
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, io};

    use super::{
        TRANSIENT_PROFILE_LEASE, TRANSIENT_PROFILE_PREFIX,
        cleanup_stale_transient_profile_data_dirs_at,
        cleanup_stale_transient_profile_data_dirs_locked, create_profile_data_dir, open_lock_file,
        profile_data_dir, sync_profile_data_dir, transient_profile_data_dir,
        transient_profile_data_root,
    };
    use ely_domain::ProfileId;

    #[test]
    fn default_standard_sync_profile_dir_keeps_profile_identity() {
        let root = std::path::Path::new("/profiles");
        let first_id = ProfileId::new();
        let second_id = ProfileId::new();

        assert_eq!(sync_profile_data_dir(root, &first_id), profile_data_dir(root, &first_id));
        assert_eq!(sync_profile_data_dir(root, &second_id), profile_data_dir(root, &second_id));
        assert_ne!(profile_data_dir(root, &first_id), profile_data_dir(root, &second_id));
    }

    #[test]
    fn persistent_profile_directory_is_private() -> Result<(), std::io::Error> {
        let directory = tempfile::tempdir()?;
        let profile_id = ProfileId::new();
        let servo_dir = create_profile_data_dir(directory.path(), &profile_id)?;

        assert_eq!(servo_dir, profile_data_dir(directory.path(), &profile_id));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(directory.path().join(profile_id.as_str()))?.permissions().mode()
                    & 0o777,
                0o700
            );
            assert_eq!(std::fs::metadata(servo_dir)?.permissions().mode() & 0o777, 0o700);
        }
        Ok(())
    }

    #[test]
    fn custom_sync_profile_dir_keeps_profile_identity() {
        let root = std::path::Path::new("/profiles");
        let profile_id = ProfileId::new();

        assert_eq!(sync_profile_data_dir(root, &profile_id), profile_data_dir(root, &profile_id));
    }

    #[test]
    fn transient_profile_directory_is_private_and_removed_on_close() -> Result<(), io::Error> {
        let directory = transient_profile_data_dir(&ProfileId::new())?;
        let path = directory.path().to_path_buf();
        assert!(path.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path)?.permissions().mode() & 0o777, 0o700);
        }

        directory.close()?;

        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn transient_profile_root_uses_the_user_project_directory() -> Result<(), io::Error> {
        let project_dirs = directories::ProjectDirs::from(
            super::ELY_QUALIFIER,
            super::ELY_ORGANIZATION,
            super::ELY_APPLICATION,
        )
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "project directory unavailable"))?;
        let expected_base = project_dirs.runtime_dir().unwrap_or_else(|| project_dirs.cache_dir());

        assert_eq!(
            transient_profile_data_root()?,
            expected_base.join(super::TRANSIENT_PROFILE_ROOT)
        );
        Ok(())
    }

    #[test]
    fn startup_cleanup_preserves_leased_profile_and_removes_stale_profile() -> Result<(), io::Error>
    {
        let root = tempfile::tempdir()?;
        let active = root.path().join(format!("{TRANSIENT_PROFILE_PREFIX}active"));
        let stale = root.path().join(format!("{TRANSIENT_PROFILE_PREFIX}stale"));
        std::fs::create_dir_all(&active)?;
        std::fs::create_dir_all(&stale)?;
        let active_lease = open_lock_file(&active.join(TRANSIENT_PROFILE_LEASE))?;
        super::lock_file(&active_lease)?;
        open_lock_file(&stale.join(TRANSIENT_PROFILE_LEASE))?;

        cleanup_stale_transient_profile_data_dirs_at(root.path())?;

        assert!(active.is_dir());
        assert!(!stale.exists());
        Ok(())
    }

    #[test]
    fn cleanup_failure_keeps_stale_profile_for_retry() -> Result<(), io::Error> {
        let root = tempfile::tempdir()?;
        let stale = root.path().join(format!("{TRANSIENT_PROFILE_PREFIX}stale"));
        std::fs::create_dir_all(&stale)?;
        open_lock_file(&stale.join(TRANSIENT_PROFILE_LEASE))?;
        let attempts = Cell::new(0_u8);

        let result = cleanup_stale_transient_profile_data_dirs_locked(root.path(), |path| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected failure"));
            }
            std::fs::remove_dir_all(path)
        });
        let Err(error) = result else {
            return Err(io::Error::other("the first cleanup attempt succeeded"));
        };

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(stale.is_dir());
        cleanup_stale_transient_profile_data_dirs_locked(root.path(), |path| {
            std::fs::remove_dir_all(path)
        })?;
        assert!(!stale.exists());
        Ok(())
    }

    #[test]
    fn cleanup_accepts_a_profile_removed_concurrently() -> Result<(), io::Error> {
        let root = tempfile::tempdir()?;
        let stale = root.path().join(format!("{TRANSIENT_PROFILE_PREFIX}stale"));
        std::fs::create_dir(&stale)?;
        open_lock_file(&stale.join(TRANSIENT_PROFILE_LEASE))?;

        cleanup_stale_transient_profile_data_dirs_locked(root.path(), |path| {
            std::fs::remove_dir_all(path)?;
            Err(io::Error::new(io::ErrorKind::NotFound, "removed by cleanup worker"))
        })?;

        assert!(!stale.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_rejects_a_symlinked_transient_root() -> Result<(), io::Error> {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir()?;
        let victim = tempfile::tempdir()?;
        let stale = victim.path().join(format!("{TRANSIENT_PROFILE_PREFIX}victim"));
        std::fs::create_dir(&stale)?;
        let root = parent.path().join("transient-root");
        symlink(victim.path(), &root)?;

        let Err(error) = cleanup_stale_transient_profile_data_dirs_at(&root) else {
            return Err(io::Error::other("symlinked transient root was accepted"));
        };

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(stale.is_dir());
        Ok(())
    }
}
