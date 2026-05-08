use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use directories::ProjectDirs;
use ely_domain::{PluginId, PluginManifest};
use thiserror::Error;

use super::plugin_packages::{PluginPackageError, PluginPackageReader, VerifiedPluginPackage};

const PLUGIN_PACKAGES_DIR: &str = "plugin-packages";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredPluginPackage {
    manifest: PluginManifest,
    package_hash: String,
    path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginPackageStore {
    root: PathBuf,
}

#[derive(Debug, Error)]
pub enum PluginPackageStoreError {
    #[error("application data directory is unavailable")]
    DataDirectoryUnavailable,

    #[error("stored plugin package is invalid: {path}")]
    InvalidStoredPackage { path: PathBuf, source: PluginPackageError },

    #[error("stored plugin package does not match selected package: {path}")]
    StoredPackageMismatch { path: PathBuf },

    #[error("plugin package changed while storing: {path}")]
    SourcePackageChanged { path: PathBuf },

    #[error("plugin package entry cannot be copied through a symlink: {path}")]
    SymlinkEntry { path: PathBuf },

    #[error("plugin package entry type is unsupported: {path}")]
    UnsupportedEntry { path: PathBuf },

    #[error("failed to create plugin package directory: {path}")]
    CreateDirectoryFailed { path: PathBuf, source: io::Error },

    #[error("failed to read plugin package directory: {path}")]
    ReadDirectoryFailed { path: PathBuf, source: io::Error },

    #[error("failed to copy plugin package file from {source_path} to {destination_path}")]
    CopyFileFailed { source_path: PathBuf, destination_path: PathBuf, source: io::Error },

    #[error("failed to move plugin package from {source_path} to {destination_path}")]
    MovePackageFailed { source_path: PathBuf, destination_path: PathBuf, source: io::Error },

    #[error("failed to clean plugin package staging directory: {path}")]
    CleanupFailed { path: PathBuf, source: io::Error },

    #[error("failed to remove plugin package directory: {path}")]
    RemoveDirectoryFailed { path: PathBuf, source: io::Error },

    #[error("system clock is unavailable for plugin package staging")]
    ClockUnavailable { source: SystemTimeError },
}

impl StoredPluginPackage {
    fn new(manifest: PluginManifest, package_hash: String, path: PathBuf) -> Self {
        Self { manifest, package_hash, path }
    }

    #[must_use]
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

impl PluginPackageStore {
    pub fn application() -> Result<Self, PluginPackageStoreError> {
        let Some(project_dirs) = ProjectDirs::from("com", "elydora", "ELY Browser") else {
            return Err(PluginPackageStoreError::DataDirectoryUnavailable);
        };

        Ok(Self::new(project_dirs.data_local_dir().join(PLUGIN_PACKAGES_DIR)))
    }

    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn store(
        &self,
        package: &VerifiedPluginPackage,
    ) -> Result<StoredPluginPackage, PluginPackageStoreError> {
        let destination = self.package_path(package);
        if destination.exists() {
            return self.read_existing_package(package, destination);
        }

        let parent = self.plugin_root(package);
        create_directory(&parent)?;
        let staging = parent.join(staging_package_name(package.package_hash())?);
        if let Err(error) = copy_package_directory(package.source_path(), staging.as_path()) {
            return Err(remove_staging_after_error(staging.as_path(), error));
        }

        let staged_package = match PluginPackageReader::read_directory_package(staging.as_path()) {
            Ok(staged_package) => staged_package,
            Err(source) => {
                let error =
                    PluginPackageStoreError::InvalidStoredPackage { path: staging.clone(), source };
                return Err(remove_staging_after_error(staging.as_path(), error));
            }
        };
        if staged_package.manifest().id() != package.manifest().id()
            || staged_package.package_hash() != package.package_hash()
        {
            let error = PluginPackageStoreError::SourcePackageChanged {
                path: package.source_path().to_path_buf(),
            };
            return Err(remove_staging_after_error(staging.as_path(), error));
        }

        if let Err(source) = fs::rename(staging.as_path(), destination.as_path()) {
            let error = PluginPackageStoreError::MovePackageFailed {
                source_path: staging.clone(),
                destination_path: destination.clone(),
                source,
            };
            return Err(remove_staging_after_error(staging.as_path(), error));
        }

        Ok(StoredPluginPackage::new(
            staged_package.manifest().clone(),
            staged_package.package_hash().to_string(),
            destination,
        ))
    }

    pub fn remove_plugin(&self, plugin_id: &PluginId) -> Result<(), PluginPackageStoreError> {
        let path = self.root.join(plugin_id.as_str());
        match fs::remove_dir_all(path.as_path()) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(PluginPackageStoreError::RemoveDirectoryFailed { path, source }),
        }
    }

    fn package_path(&self, package: &VerifiedPluginPackage) -> PathBuf {
        self.plugin_root(package).join(format!("{}.rplug", package.package_hash()))
    }

    fn plugin_root(&self, package: &VerifiedPluginPackage) -> PathBuf {
        self.root.join(package.manifest().id().as_str())
    }

    fn read_existing_package(
        &self,
        source_package: &VerifiedPluginPackage,
        path: PathBuf,
    ) -> Result<StoredPluginPackage, PluginPackageStoreError> {
        let stored_package =
            PluginPackageReader::read_directory_package(path.as_path()).map_err(|source| {
                PluginPackageStoreError::InvalidStoredPackage { path: path.clone(), source }
            })?;

        if stored_package.manifest().id() != source_package.manifest().id()
            || stored_package.package_hash() != source_package.package_hash()
        {
            return Err(PluginPackageStoreError::StoredPackageMismatch { path });
        }

        Ok(StoredPluginPackage::new(
            stored_package.manifest().clone(),
            stored_package.package_hash().to_string(),
            path,
        ))
    }
}

fn copy_package_directory(
    source_path: &Path,
    destination_path: &Path,
) -> Result<(), PluginPackageStoreError> {
    create_directory(destination_path)?;
    let entries = fs::read_dir(source_path).map_err(|source| {
        PluginPackageStoreError::ReadDirectoryFailed { path: source_path.to_path_buf(), source }
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| PluginPackageStoreError::ReadDirectoryFailed {
            path: source_path.to_path_buf(),
            source,
        })?;
        let source_entry = entry.path();
        let destination_entry = destination_path.join(entry.file_name());
        let metadata = fs::symlink_metadata(source_entry.as_path()).map_err(|source| {
            PluginPackageStoreError::ReadDirectoryFailed { path: source_entry.clone(), source }
        })?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            return Err(PluginPackageStoreError::SymlinkEntry { path: source_entry });
        }
        if file_type.is_dir() {
            copy_package_directory(source_entry.as_path(), destination_entry.as_path())?;
            continue;
        }
        if file_type.is_file() {
            fs::copy(source_entry.as_path(), destination_entry.as_path()).map_err(|source| {
                PluginPackageStoreError::CopyFileFailed {
                    source_path: source_entry,
                    destination_path: destination_entry,
                    source,
                }
            })?;
            continue;
        }

        return Err(PluginPackageStoreError::UnsupportedEntry { path: source_entry });
    }

    Ok(())
}

fn create_directory(path: &Path) -> Result<(), PluginPackageStoreError> {
    fs::create_dir_all(path).map_err(|source| PluginPackageStoreError::CreateDirectoryFailed {
        path: path.to_path_buf(),
        source,
    })
}

fn remove_staging_after_error(
    path: &Path,
    error: PluginPackageStoreError,
) -> PluginPackageStoreError {
    match fs::remove_dir_all(path) {
        Ok(()) => error,
        Err(source) if source.kind() == io::ErrorKind::NotFound => error,
        Err(source) => PluginPackageStoreError::CleanupFailed { path: path.to_path_buf(), source },
    }
}

fn staging_package_name(package_hash: &str) -> Result<String, PluginPackageStoreError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| PluginPackageStoreError::ClockUnavailable { source })?
        .as_nanos();
    Ok(format!(".staging-{package_hash}-{nanos}.rplug"))
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{PluginPackageStore, PluginPackageStoreError};
    use crate::services::plugin_package_fixtures::write_signed_package;
    use crate::services::plugin_packages::PluginPackageReader;

    #[test]
    fn stores_verified_package_under_plugin_hash_path() -> Result<(), Box<dyn Error>> {
        let tree = TempTree::new("store")?;
        let source_package = write_package(tree.path(), "verified", b"wasm component")?;
        let package = PluginPackageReader::read_directory_package(source_package.as_path())?;
        let store = PluginPackageStore::new(tree.path().join("store"));

        let stored_package = store.store(&package)?;

        assert_eq!(stored_package.manifest().id().as_str(), "com.elydora.verified");
        assert_eq!(stored_package.package_hash, package.package_hash());
        assert_eq!(
            stored_package.path,
            tree.path()
                .join("store")
                .join("com.elydora.verified")
                .join(format!("{}.rplug", package.package_hash()))
        );
        assert!(stored_package.path.join("plugin.toml").is_file());
        assert!(stored_package.path.join("component.wasm").is_file());
        assert!(stored_package.path.join("signatures").join("ed25519.sig").is_file());
        Ok(())
    }

    #[test]
    fn rejects_corrupt_existing_stored_package() -> Result<(), Box<dyn Error>> {
        let tree = TempTree::new("corrupt")?;
        let source_package = write_package(tree.path(), "verified", b"wasm component")?;
        let package = PluginPackageReader::read_directory_package(source_package.as_path())?;
        let store = PluginPackageStore::new(tree.path().join("store"));
        let stored_package = store.store(&package)?;
        fs::write(stored_package.path.join("component.wasm"), b"changed")?;

        let error = store
            .store(&package)
            .err()
            .ok_or_else(|| std::io::Error::other("corrupt stored package was accepted"))?;

        assert!(matches!(error, PluginPackageStoreError::InvalidStoredPackage { .. }));
        Ok(())
    }

    #[test]
    fn removes_stored_packages_for_plugin() -> Result<(), Box<dyn Error>> {
        let tree = TempTree::new("remove")?;
        let source_package = write_package(tree.path(), "verified", b"wasm component")?;
        let package = PluginPackageReader::read_directory_package(source_package.as_path())?;
        let store = PluginPackageStore::new(tree.path().join("store"));
        let stored_package = store.store(&package)?;

        store.remove_plugin(package.manifest().id())?;

        assert!(!stored_package.path.exists());
        assert!(!tree.path().join("store").join("com.elydora.verified").exists());
        Ok(())
    }

    fn write_package(root: &Path, name: &str, component: &[u8]) -> Result<PathBuf, Box<dyn Error>> {
        write_signed_package(root, name, component)
    }

    struct TempTree {
        path: PathBuf,
    }

    impl TempTree {
        fn new(name: &str) -> Result<Self, Box<dyn Error>> {
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = std::env::temp_dir().join(format!("ely-plugin-store-{name}-{nanos}"));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            _ = fs::remove_dir_all(self.path.as_path());
        }
    }
}
