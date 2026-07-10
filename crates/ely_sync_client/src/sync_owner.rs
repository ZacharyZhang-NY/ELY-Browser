use std::{
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
use uuid::Uuid;

use crate::{SyncClientError, device_api::is_subject_id};

const OWNER_FILE: &str = ".sync-owner-user-id";
const OWNER_TEMP_PREFIX: &str = ".sync-owner-tmp-";
const MAX_SUBJECT_ID_BYTES: usize = 128;

#[derive(Clone, Debug)]
pub struct SyncOwnerStore {
    root: PathBuf,
}

impl SyncOwnerStore {
    pub fn new(browser_data_root: &Path) -> Self {
        Self { root: browser_data_root.to_path_buf() }
    }

    pub fn claim(&self, user_id: &str) -> Result<(), SyncClientError> {
        validate_subject_id(user_id)?;
        let directory = open_root(&self.root, true)?
            .ok_or_else(|| storage("browser data root is unavailable"))?;
        if let Some(owner) = read_owner(&directory)? {
            return verify_owner(&owner, user_id);
        }
        persist_owner(&directory, user_id)
    }

    pub fn verify(&self, user_id: &str) -> Result<(), SyncClientError> {
        validate_subject_id(user_id)?;
        let Some(directory) = open_root(&self.root, false)? else {
            return Err(SyncClientError::SyncOwnerUnclaimed);
        };
        let owner = read_owner(&directory)?.ok_or(SyncClientError::SyncOwnerUnclaimed)?;
        verify_owner(&owner, user_id)
    }
}

fn persist_owner(directory: &Dir, user_id: &str) -> Result<(), SyncClientError> {
    let temporary = format!("{OWNER_TEMP_PREFIX}{}", Uuid::now_v7().simple());
    let mut options = private_open_options();
    options.write(true).create_new(true);
    let mut file = directory.open_with(&temporary, &options).map_err(storage_io)?;
    validate_private_file(&file)?;
    set_private_file_permissions(&file)?;
    let write_result = file
        .write_all(user_id.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = write_result {
        let _ = remove_temporary(directory, &temporary);
        return Err(storage_io(error));
    }

    match publish_owner(directory, &temporary) {
        Ok(()) => sync_directory(directory)?,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            remove_temporary(directory, &temporary)?;
        }
        Err(error) => {
            let _ = remove_temporary(directory, &temporary);
            return Err(storage_io(error));
        }
    }
    let owner = read_owner(directory)?.ok_or_else(|| storage("sync owner disappeared"))?;
    verify_owner(&owner, user_id)
}

fn read_owner(directory: &Dir) -> Result<Option<String>, SyncClientError> {
    let mut options = private_open_options();
    options.read(true);
    let file = match directory.open_with(OWNER_FILE, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(storage_io(error)),
    };
    validate_private_file(&file)?;
    let mut bytes = Vec::new();
    file.into_std()
        .take((MAX_SUBJECT_ID_BYTES + 2) as u64)
        .read_to_end(&mut bytes)
        .map_err(storage_io)?;
    if bytes.len() > MAX_SUBJECT_ID_BYTES + 1 || bytes.last() != Some(&b'\n') {
        return Err(storage("sync owner record is invalid"));
    }
    bytes.pop();
    let owner = String::from_utf8(bytes).map_err(|_| storage("sync owner record is invalid"))?;
    validate_subject_id(&owner)?;
    Ok(Some(owner))
}

fn open_root(path: &Path, create: bool) -> Result<Option<Dir>, SyncClientError> {
    let mut root = PathBuf::new();
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => root.push(prefix.as_os_str()),
            Component::RootDir => root.push(std::path::MAIN_SEPARATOR_STR),
            Component::Normal(name) => names.push(name.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir => return Err(storage("browser data root traversal is invalid")),
        }
    }
    if root.as_os_str().is_empty() {
        return Err(storage("browser data root must be absolute"));
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
    validate_private_directory(&directory)?;
    if create {
        set_private_directory_permissions(&directory)?;
    }
    Ok(Some(directory))
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
        return Err(storage("browser data root is invalid"));
    }
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        if metadata.uid() != rustix::process::geteuid().as_raw() {
            return Err(storage("browser data root ownership is invalid"));
        }
    }
    Ok(())
}

fn validate_private_file(file: &File) -> Result<(), SyncClientError> {
    let metadata = file.metadata().map_err(storage_io)?;
    if !metadata.is_file() || CrossPlatformMetadataExt::nlink(&metadata) != 1 {
        return Err(storage("sync owner record is invalid"));
    }
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o077 != 0 {
            return Err(storage("sync owner record permissions are invalid"));
        }
    }
    Ok(())
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

fn set_private_directory_permissions(directory: &Dir) -> Result<(), SyncClientError> {
    #[cfg(unix)]
    directory
        .set_permissions(".", Permissions::from_std(std::fs::Permissions::from_mode(0o700)))
        .map_err(storage_io)?;
    Ok(())
}

fn set_private_file_permissions(file: &File) -> Result<(), SyncClientError> {
    #[cfg(unix)]
    file.set_permissions(Permissions::from_std(std::fs::Permissions::from_mode(0o600)))
        .map_err(storage_io)?;
    Ok(())
}

#[cfg(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
))]
fn publish_owner(directory: &Dir, temporary: &str) -> io::Result<()> {
    use std::os::fd::AsFd;

    rustix::fs::renameat_with(
        directory.as_fd(),
        temporary,
        directory.as_fd(),
        OWNER_FILE,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(Into::into)
}

#[cfg(windows)]
fn publish_owner(directory: &Dir, temporary: &str) -> io::Result<()> {
    directory.rename(temporary, directory, OWNER_FILE)
}

#[cfg(not(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox",
    windows
)))]
fn publish_owner(directory: &Dir, temporary: &str) -> io::Result<()> {
    directory.hard_link(temporary, directory, OWNER_FILE)?;
    directory.remove_file(temporary)
}

fn remove_temporary(directory: &Dir, temporary: &str) -> Result<(), SyncClientError> {
    match directory.remove_file(temporary) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(storage_io(error)),
    }
}

#[cfg(unix)]
fn sync_directory(directory: &Dir) -> Result<(), SyncClientError> {
    directory.try_clone().map_err(storage_io)?.into_std_file().sync_all().map_err(storage_io)
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Dir) -> Result<(), SyncClientError> {
    Ok(())
}

fn validate_subject_id(value: &str) -> Result<(), SyncClientError> {
    if is_subject_id(value) {
        return Ok(());
    }
    Err(storage("sync owner user identifier is invalid"))
}

fn verify_owner(owner: &str, user_id: &str) -> Result<(), SyncClientError> {
    if owner == user_id { Ok(()) } else { Err(SyncClientError::SyncOwnerMismatch) }
}

fn storage_io(error: io::Error) -> SyncClientError {
    storage(error.to_string())
}

fn storage(message: impl Into<String>) -> SyncClientError {
    SyncClientError::SyncOwnerStorage(message.into())
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
