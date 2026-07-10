use std::{
    io::{self, ErrorKind, Read, Write},
    path::{Component, Path},
};

use cap_fs_ext::{
    DirExt, FollowSymlinks, MetadataExt as CrossPlatformMetadataExt, OpenOptionsFollowExt,
};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, Metadata, OpenOptions, Permissions},
};
use ely_sync_client::DeviceIdentity;
use uuid::Uuid;

const SOURCE_COMPONENTS: [&str; 3] = ["default", "servo", "sync"];
const DEVICE_FILE: &str = "device.json";
const COMPLETION_MARKER: &str = ".default-sync-device-migrated-v1";
const LOCK_FILE: &str = ".default-sync-device-migration.lock";
const TEMP_PREFIX: &str = ".ely-sync-migration-";
const COMPLETION_BYTES: &[u8] = b"ely-default-sync-device-migration-v1\n";
const MAX_DEVICE_BYTES: usize = 16 * 1024;

#[cfg(unix)]
type ExpectedOwner = u32;
#[cfg(not(unix))]
type ExpectedOwner = ();

pub(super) fn migrate_default_sync_device(
    profile_root: &Path,
    stable_profile_dir: &Path,
) -> io::Result<()> {
    let expected_owner = effective_owner()?;
    let root = Dir::open_ambient_dir(profile_root, ambient_authority())?;
    validate_directory(&root, expected_owner)?;
    let relative_destination = stable_profile_dir
        .strip_prefix(profile_root)
        .map_err(|_| invalid_source("stable profile directory escapes the profile root"))?;
    let stable = open_or_create_path(&root, relative_destination, expected_owner)?;
    let destination = open_or_create_child(&stable, "sync", expected_owner)?;
    let _lock = acquire_lock(&destination, expected_owner)?;
    cleanup_temporary_files(&destination)?;

    if completion_marker_exists(&destination, expected_owner)? {
        validate_optional_device(&destination, expected_owner)?;
        return Ok(());
    }
    if validate_optional_device(&destination, expected_owner)? {
        return write_completion_marker(&destination, expected_owner);
    }

    let Some(source) = open_source_directory(&root, expected_owner)? else {
        return Ok(());
    };
    let Some(bytes) = read_secure_file(&source, DEVICE_FILE, expected_owner, MAX_DEVICE_BYTES)?
    else {
        return Ok(());
    };
    validate_device_bytes(&bytes)?;
    persist_bytes_if_absent(
        &destination,
        DEVICE_FILE,
        &bytes,
        expected_owner,
        validate_device_bytes,
    )?;
    write_completion_marker(&destination, expected_owner)
}

fn open_source_directory(root: &Dir, owner: ExpectedOwner) -> io::Result<Option<Dir>> {
    let mut directory = root.try_clone()?;
    for component in SOURCE_COMPONENTS {
        directory = match directory.open_dir_nofollow(component) {
            Ok(child) => child,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        validate_directory(&directory, owner)?;
    }
    Ok(Some(directory))
}

fn open_or_create_path(root: &Dir, path: &Path, owner: ExpectedOwner) -> io::Result<Dir> {
    let mut directory = root.try_clone()?;
    for component in path.components() {
        let Component::Normal(name) = component else {
            return Err(invalid_source("stable profile directory contains an invalid component"));
        };
        directory = open_or_create_child(&directory, name, owner)?;
    }
    Ok(directory)
}

fn open_or_create_child(
    parent: &Dir,
    name: impl AsRef<Path>,
    owner: ExpectedOwner,
) -> io::Result<Dir> {
    let name = name.as_ref();
    let directory = match parent.open_dir_nofollow(name) {
        Ok(directory) => directory,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            match parent.create_dir(name) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            let directory = parent.open_dir_nofollow(name)?;
            set_private_directory_permissions(&directory)?;
            directory
        }
        Err(error) => return Err(error),
    };
    validate_directory(&directory, owner)?;
    Ok(directory)
}

fn acquire_lock(directory: &Dir, owner: ExpectedOwner) -> io::Result<std::fs::File> {
    let mut options = private_open_options();
    options.read(true).write(true).create(true);
    let file = directory.open_with(LOCK_FILE, &options)?;
    validate_file(&file, owner)?;
    set_private_file_permissions(&file)?;
    let file = file.into_std();
    fs2::FileExt::lock_exclusive(&file)?;
    Ok(file)
}

fn validate_optional_device(directory: &Dir, owner: ExpectedOwner) -> io::Result<bool> {
    let Some(bytes) = read_secure_file(directory, DEVICE_FILE, owner, MAX_DEVICE_BYTES)? else {
        return Ok(false);
    };
    validate_device_bytes(&bytes)?;
    Ok(true)
}

fn completion_marker_exists(directory: &Dir, owner: ExpectedOwner) -> io::Result<bool> {
    let Some(bytes) =
        read_secure_file(directory, COMPLETION_MARKER, owner, COMPLETION_BYTES.len())?
    else {
        return Ok(false);
    };
    validate_marker_bytes(&bytes)?;
    Ok(true)
}

fn write_completion_marker(directory: &Dir, owner: ExpectedOwner) -> io::Result<()> {
    persist_bytes_if_absent(
        directory,
        COMPLETION_MARKER,
        COMPLETION_BYTES,
        owner,
        validate_marker_bytes,
    )
}

fn persist_bytes_if_absent(
    directory: &Dir,
    destination: &str,
    bytes: &[u8],
    owner: ExpectedOwner,
    validate: fn(&[u8]) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(existing) = read_secure_file(directory, destination, owner, MAX_DEVICE_BYTES)? {
        return validate(&existing);
    }

    let temporary = format!("{TEMP_PREFIX}{}", Uuid::now_v7().simple());
    let mut options = private_open_options();
    options.write(true).create_new(true);
    let mut file = directory.open_with(&temporary, &options)?;
    validate_file(&file, owner)?;
    set_private_file_permissions(&file)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = directory.remove_file(&temporary);
        return Err(error);
    }

    let linked = match directory.hard_link(&temporary, directory, destination) {
        Ok(()) => true,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => false,
        Err(error) => {
            let _ = directory.remove_file(&temporary);
            return Err(error);
        }
    };
    remove_file_if_present(directory, &temporary)?;
    if linked {
        sync_directory(directory)?;
    }
    let existing = read_secure_file(directory, destination, owner, MAX_DEVICE_BYTES)?
        .ok_or_else(|| invalid_source("migration destination disappeared"))?;
    validate(&existing)
}

fn read_secure_file(
    directory: &Dir,
    name: &str,
    owner: ExpectedOwner,
    maximum_bytes: usize,
) -> io::Result<Option<Vec<u8>>> {
    let mut options = private_open_options();
    options.read(true);
    let file = match directory.open_with(name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    validate_file(&file, owner)?;
    if file.metadata()?.len() > maximum_bytes as u64 {
        return Err(invalid_source("migration file exceeds its size limit"));
    }
    let mut bytes = Vec::new();
    file.into_std().take((maximum_bytes + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > maximum_bytes {
        return Err(invalid_source("migration file exceeds its size limit"));
    }
    Ok(Some(bytes))
}

fn cleanup_temporary_files(directory: &Dir) -> io::Result<()> {
    for entry in directory.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with(TEMP_PREFIX) {
            continue;
        }
        if entry.file_type()?.is_dir() {
            return Err(invalid_source("migration temporary path is a directory"));
        }
        directory.remove_file_or_symlink(name)?;
    }
    Ok(())
}

fn remove_file_if_present(directory: &Dir, name: &str) -> io::Result<()> {
    match directory.remove_file(name) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
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

fn validate_directory(directory: &Dir, owner: ExpectedOwner) -> io::Result<()> {
    let metadata = directory.dir_metadata()?;
    if !metadata.is_dir() {
        return Err(invalid_source("migration path is not a directory"));
    }
    validate_owner_and_mode(&metadata, owner)
}

fn validate_file(file: &File, owner: ExpectedOwner) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || CrossPlatformMetadataExt::nlink(&metadata) != 1 {
        return Err(invalid_source("migration file link state is invalid"));
    }
    validate_owner_and_mode(&metadata, owner)
}

#[cfg(unix)]
fn validate_owner_and_mode(metadata: &Metadata, owner: ExpectedOwner) -> io::Result<()> {
    use cap_std::fs::{MetadataExt, PermissionsExt};
    if metadata.uid() != owner || metadata.permissions().mode() & 0o022 != 0 {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "migration path ownership or permissions are invalid",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
const fn validate_owner_and_mode(_metadata: &Metadata, _owner: ExpectedOwner) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn effective_owner() -> io::Result<ExpectedOwner> {
    use std::os::unix::fs::MetadataExt;
    let probe = tempfile::NamedTempFile::new()?;
    Ok(probe.as_file().metadata()?.uid())
}

#[cfg(not(unix))]
const fn effective_owner() -> io::Result<ExpectedOwner> {
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(directory: &Dir) -> io::Result<()> {
    use cap_std::fs::PermissionsExt;
    directory.set_permissions(".", Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
const fn set_private_directory_permissions(_directory: &Dir) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(file: &File) -> io::Result<()> {
    use cap_std::fs::PermissionsExt;
    file.set_permissions(Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
const fn set_private_file_permissions(_file: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn sync_directory(directory: &Dir) -> io::Result<()> {
    directory.try_clone()?.into_std_file().sync_all()
}

#[cfg(not(unix))]
const fn sync_directory(_directory: &Dir) -> io::Result<()> {
    Ok(())
}

fn validate_device_bytes(bytes: &[u8]) -> io::Result<()> {
    DeviceIdentity::validate_stored_bytes(bytes, Path::new(DEVICE_FILE))
        .map_err(|error| invalid_source(error.to_string()))
}

fn validate_marker_bytes(bytes: &[u8]) -> io::Result<()> {
    if bytes == COMPLETION_BYTES {
        Ok(())
    } else {
        Err(invalid_source("legacy sync migration marker is invalid"))
    }
}

fn invalid_source(message: impl Into<String>) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
#[path = "legacy_sync_migration_tests.rs"]
mod tests;
