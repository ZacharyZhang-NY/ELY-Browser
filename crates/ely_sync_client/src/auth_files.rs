use std::{
    fs,
    io::{self, ErrorKind, Read, Write},
    path::{Component, Path, PathBuf},
};

use cap_fs_ext::{
    DirExt, FollowSymlinks, MetadataExt as CrossPlatformMetadataExt, OpenOptionsFollowExt,
};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, OpenOptions, Permissions},
};
use zeroize::Zeroizing;

use crate::error::SyncClientError;

const MARKER_CONTENT: &[u8] = b"ely-bearer-migration-v1\n";

pub(super) fn read_legacy_bytes(
    path: &Path,
    maximum_bytes: usize,
) -> Result<Option<Zeroizing<Vec<u8>>>, SyncClientError> {
    let Some((directory, name)) = open_parent(path, false)? else {
        return Ok(None);
    };
    let Some(file) = open_existing(&directory, name)? else {
        return Ok(None);
    };
    validate_private_file(&file)?;
    let mut bytes = Zeroizing::new(Vec::new());
    file.into_std().take((maximum_bytes + 1) as u64).read_to_end(&mut bytes).map_err(storage_io)?;
    if bytes.len() > maximum_bytes {
        return Err(storage_error("legacy bearer token is too large"));
    }
    Ok(Some(bytes))
}

pub(super) fn ensure_migration_marker(path: &Path) -> Result<(), SyncClientError> {
    let (directory, name) = open_parent(path, true)?
        .ok_or_else(|| storage_error("bearer migration marker parent is missing"))?;
    if let Some(file) = open_existing(&directory, name)? {
        return validate_migration_marker(file);
    }
    let temporary_path = marker_temporary_path(path)?;
    remove_file_if_present(&temporary_path)?;
    let temporary_name = temporary_path
        .file_name()
        .ok_or_else(|| storage_error("bearer migration marker temp name is missing"))?;
    let mut options = private_open_options();
    options.write(true).create_new(true);
    let mut file = directory.open_with(temporary_name, &options).map_err(storage_io)?;
    validate_private_file(&file)?;
    set_private_file_permissions(&file)?;
    file.write_all(MARKER_CONTENT).map_err(storage_io)?;
    file.sync_all().map_err(storage_io)?;
    drop(file);
    directory.rename(temporary_name, &directory, name).map_err(storage_io)?;
    sync_directory(&directory)
}

pub(super) fn migration_marker_exists(path: &Path) -> Result<bool, SyncClientError> {
    let Some((directory, name)) = open_parent(path, false)? else {
        return Ok(false);
    };
    let Some(file) = open_existing(&directory, name)? else {
        return Ok(false);
    };
    validate_migration_marker(file)?;
    Ok(true)
}

fn validate_migration_marker(file: File) -> Result<(), SyncClientError> {
    validate_private_file(&file)?;
    let mut bytes = Vec::new();
    file.into_std()
        .take((MARKER_CONTENT.len() + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(storage_io)?;
    if bytes != MARKER_CONTENT {
        return Err(storage_error("bearer migration marker content is invalid"));
    }
    Ok(())
}

fn marker_temporary_path(path: &Path) -> Result<PathBuf, SyncClientError> {
    let mut name = path
        .file_name()
        .ok_or_else(|| storage_error("bearer migration marker name is missing"))?
        .to_os_string();
    name.push(".tmp");
    Ok(path.with_file_name(name))
}

pub(super) fn remove_file_if_present(path: &Path) -> Result<(), SyncClientError> {
    let Some((directory, name)) = open_parent(path, false)? else {
        return Ok(());
    };
    let metadata = match directory.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(storage_io(error)),
    };
    if metadata.is_symlink() {
        return directory.remove_file_or_symlink(name).map_err(storage_io);
    }
    let file = open_existing(&directory, name)?.ok_or_else(|| {
        storage_error("legacy bearer token disappeared during cleanup validation")
    })?;
    validate_private_file(&file)?;
    drop(file);
    directory.remove_file_or_symlink(name).map_err(storage_io)
}

pub(super) fn open_lock_file(path: &Path) -> Result<fs::File, SyncClientError> {
    let (directory, name) =
        open_parent(path, true)?.ok_or_else(|| storage_error("bearer lock parent is missing"))?;
    let mut options = private_open_options();
    options.read(true).write(true).create(true);
    let file = directory.open_with(name, &options).map_err(storage_io)?;
    validate_private_file(&file)?;
    set_private_file_permissions(&file)?;
    Ok(file.into_std())
}

fn open_parent(
    path: &Path,
    create: bool,
) -> Result<Option<(Dir, &std::ffi::OsStr)>, SyncClientError> {
    let parent = path.parent().ok_or_else(|| storage_error("credential file parent is missing"))?;
    let name = path.file_name().ok_or_else(|| storage_error("credential file name is missing"))?;
    let Some(directory) = open_directory_nofollow(parent, create)? else {
        return Ok(None);
    };
    validate_private_directory(&directory)?;
    set_private_directory_permissions(&directory)?;
    Ok(Some((directory, name)))
}

fn open_directory_nofollow(path: &Path, create: bool) -> Result<Option<Dir>, SyncClientError> {
    let mut root = PathBuf::new();
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => root.push(prefix.as_os_str()),
            Component::RootDir => root.push(std::path::MAIN_SEPARATOR_STR),
            Component::Normal(name) => names.push(name.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(storage_error("credential path traversal is invalid"));
            }
        }
    }
    if root.as_os_str().is_empty() {
        return Err(storage_error("credential paths must be absolute"));
    }
    let mut directory = Dir::open_ambient_dir(root, ambient_authority()).map_err(storage_io)?;
    #[cfg(windows)]
    let strict_component = names.len().saturating_sub(4);
    #[cfg(windows)]
    let mut require_nofollow = false;
    #[cfg(windows)]
    let mut component_index = 0;
    #[cfg(not(windows))]
    let mut require_nofollow = directory_requires_nofollow(&directory)?;
    for name in names {
        #[cfg(windows)]
        {
            require_nofollow |= component_index >= strict_component;
            component_index += 1;
        }
        if create {
            match directory.create_dir(&name) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(storage_io(error)),
            }
        }
        let opened = match directory.open_dir_nofollow(&name) {
            Ok(directory) => directory,
            Err(_) if !require_nofollow => match directory.open_dir(&name) {
                Ok(directory) => directory,
                Err(error) if !create && error.kind() == ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(storage_io(error)),
            },
            Err(error) if !create && error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(storage_io(error)),
        };
        #[cfg(not(windows))]
        {
            require_nofollow |= directory_requires_nofollow(&opened)?;
        }
        directory = opened;
    }
    Ok(Some(directory))
}

#[cfg(unix)]
fn directory_requires_nofollow(directory: &Dir) -> Result<bool, SyncClientError> {
    use cap_std::fs::MetadataExt;

    let metadata = directory.dir_metadata().map_err(storage_io)?;
    Ok(metadata.uid() == rustix::process::geteuid().as_raw() || metadata.mode() & 0o022 != 0)
}

#[cfg(not(any(unix, windows)))]
fn directory_requires_nofollow(_directory: &Dir) -> Result<bool, SyncClientError> {
    Ok(true)
}

fn open_existing(directory: &Dir, name: &std::ffi::OsStr) -> Result<Option<File>, SyncClientError> {
    let mut options = private_open_options();
    options.read(true);
    match directory.open_with(name, &options) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(storage_io(error)),
    }
}

fn private_open_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

fn validate_private_directory(directory: &Dir) -> Result<(), SyncClientError> {
    let metadata = directory.dir_metadata().map_err(storage_io)?;
    if !metadata.is_dir() {
        return Err(storage_error("credential file parent is not a directory"));
    }
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        if metadata.uid() != rustix::process::geteuid().as_raw() {
            return Err(storage_error("credential file parent ownership is invalid"));
        }
    }
    Ok(())
}

fn validate_private_file(file: &File) -> Result<(), SyncClientError> {
    let metadata = file.metadata().map_err(storage_io)?;
    if !metadata.is_file() || CrossPlatformMetadataExt::nlink(&metadata) != 1 {
        return Err(storage_error("credential file link state is invalid"));
    }
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        if metadata.uid() != rustix::process::geteuid().as_raw() {
            return Err(storage_error("credential file ownership is invalid"));
        }
    }
    Ok(())
}

fn set_private_directory_permissions(directory: &Dir) -> Result<(), SyncClientError> {
    #[cfg(unix)]
    directory
        .set_permissions(".", Permissions::from_std(fs::Permissions::from_mode(0o700)))
        .map_err(storage_io)?;
    Ok(())
}

fn set_private_file_permissions(file: &File) -> Result<(), SyncClientError> {
    #[cfg(unix)]
    file.set_permissions(Permissions::from_std(fs::Permissions::from_mode(0o600)))
        .map_err(storage_io)?;
    Ok(())
}

#[cfg(unix)]
fn sync_directory(directory: &Dir) -> Result<(), SyncClientError> {
    directory.try_clone().map_err(storage_io)?.into_std_file().sync_all().map_err(storage_io)
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Dir) -> Result<(), SyncClientError> {
    Ok(())
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn storage_io(error: io::Error) -> SyncClientError {
    storage_error(error.to_string())
}

fn storage_error(message: impl Into<String>) -> SyncClientError {
    SyncClientError::BearerCredentialStorage(message.into())
}
