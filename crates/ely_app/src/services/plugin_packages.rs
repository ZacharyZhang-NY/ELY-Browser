use std::{
    ffi::OsStr,
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use ely_domain::PluginManifest;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPluginPackage {
    source_path: PathBuf,
    manifest: PluginManifest,
    package_hash: String,
}

#[derive(Debug, Error)]
pub enum PluginPackageError {
    #[error("plugin package must use .rplug extension: {path}")]
    InvalidPackageExtension { path: PathBuf },

    #[error("plugin package is unavailable: {path}")]
    PackageUnavailable { path: PathBuf },

    #[error("plugin package must be a directory: {path}")]
    PackageNotDirectory { path: PathBuf },

    #[error("plugin package is missing {entry}: {path}")]
    MissingEntry { entry: &'static str, path: PathBuf },

    #[error("failed to read plugin package entry {entry}: {path}")]
    ReadFailed { entry: &'static str, path: PathBuf, source: io::Error },

    #[error("plugin package entry name must be valid utf-8: {path}")]
    InvalidEntryName { path: PathBuf },

    #[error("plugin package entry cannot be a symlink: {path}")]
    SymlinkEntry { path: PathBuf },

    #[error("plugin package entry type is unsupported: {path}")]
    UnsupportedEntry { path: PathBuf },

    #[error("plugin package checksum mismatch for component.wasm")]
    ChecksumMismatch,

    #[error("plugin package signature mismatch for signatures/ed25519.sig")]
    SignatureMismatch,

    #[error(transparent)]
    Manifest(#[from] ely_domain::DomainError),
}

pub struct PluginPackageReader;

impl PluginPackageReader {
    pub fn read_directory_package(
        path: &Path,
    ) -> Result<VerifiedPluginPackage, PluginPackageError> {
        require_rplug_directory(path)?;

        let manifest_path = path.join("plugin.toml");
        require_file("plugin.toml", &manifest_path)?;
        let manifest_text = read_text("plugin.toml", &manifest_path)?;
        let manifest = PluginManifest::from_toml(manifest_text.as_str())?;

        let component_path = path.join("component.wasm");
        require_file("component.wasm", &component_path)?;
        let component_checksum = sha256_file(&component_path)?;
        if component_checksum != manifest.checksum() {
            return Err(PluginPackageError::ChecksumMismatch);
        }

        let signature_path = path.join("signatures").join("ed25519.sig");
        require_file("signatures/ed25519.sig", &signature_path)?;
        let signature = read_text("signatures/ed25519.sig", &signature_path)?;
        if signature.trim().to_ascii_lowercase() != manifest.signature().value() {
            return Err(PluginPackageError::SignatureMismatch);
        }

        let package_hash = sha256_directory(path)?;
        Ok(VerifiedPluginPackage::new(path.to_path_buf(), manifest, package_hash))
    }
}

impl VerifiedPluginPackage {
    fn new(source_path: PathBuf, manifest: PluginManifest, package_hash: String) -> Self {
        Self { source_path, manifest, package_hash }
    }

    #[must_use]
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    #[must_use]
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    #[must_use]
    pub fn package_hash(&self) -> &str {
        &self.package_hash
    }
}

fn require_rplug_directory(path: &Path) -> Result<(), PluginPackageError> {
    if path.extension().and_then(|value| value.to_str()) != Some("rplug") {
        return Err(PluginPackageError::InvalidPackageExtension { path: path.to_path_buf() });
    }

    let metadata = fs::metadata(path)
        .map_err(|_| PluginPackageError::PackageUnavailable { path: path.to_path_buf() })?;
    if !metadata.is_dir() {
        return Err(PluginPackageError::PackageNotDirectory { path: path.to_path_buf() });
    }

    Ok(())
}

fn require_file(entry: &'static str, path: &Path) -> Result<(), PluginPackageError> {
    let metadata = fs::metadata(path)
        .map_err(|_| PluginPackageError::MissingEntry { entry, path: path.to_path_buf() })?;
    if !metadata.is_file() {
        return Err(PluginPackageError::MissingEntry { entry, path: path.to_path_buf() });
    }

    Ok(())
}

fn read_text(entry: &'static str, path: &Path) -> Result<String, PluginPackageError> {
    fs::read_to_string(path).map_err(|source| PluginPackageError::ReadFailed {
        entry,
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_file(path: &Path) -> Result<String, PluginPackageError> {
    let mut file = fs::File::open(path).map_err(|source| PluginPackageError::ReadFailed {
        entry: "component.wasm",
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file.read(&mut buffer).map_err(|source| PluginPackageError::ReadFailed {
            entry: "component.wasm",
            path: path.to_path_buf(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_directory(path: &Path) -> Result<String, PluginPackageError> {
    let mut entries = Vec::new();
    collect_package_entries(path, Path::new(""), &mut entries)?;
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    let mut hasher = Sha256::new();
    for entry in entries {
        let relative_path = canonical_relative_path(entry.relative_path.as_path())?;
        match entry.kind {
            PackageEntryKind::Directory => {
                hasher.update(b"dir\0");
                hasher.update(relative_path.as_bytes());
                hasher.update(b"\0");
            }
            PackageEntryKind::File => {
                hasher.update(b"file\0");
                hasher.update(relative_path.as_bytes());
                hasher.update(b"\0");
                hash_file_content(entry.absolute_path.as_path(), &mut hasher)?;
            }
        }
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_package_entries(
    root: &Path,
    relative_dir: &Path,
    entries: &mut Vec<PackageEntry>,
) -> Result<(), PluginPackageError> {
    let absolute_dir = root.join(relative_dir);
    for entry in fs::read_dir(absolute_dir.as_path()).map_err(|source| {
        PluginPackageError::ReadFailed { entry: "package", path: absolute_dir.clone(), source }
    })? {
        let entry = entry.map_err(|source| PluginPackageError::ReadFailed {
            entry: "package",
            path: absolute_dir.clone(),
            source,
        })?;
        let absolute_path = entry.path();
        let relative_path = relative_dir.join(entry.file_name());
        let metadata = fs::symlink_metadata(absolute_path.as_path()).map_err(|source| {
            PluginPackageError::ReadFailed { entry: "package", path: absolute_path.clone(), source }
        })?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            return Err(PluginPackageError::SymlinkEntry { path: absolute_path });
        }
        if file_type.is_dir() {
            entries.push(PackageEntry::new(
                relative_path.clone(),
                absolute_path,
                PackageEntryKind::Directory,
            ));
            collect_package_entries(root, relative_path.as_path(), entries)?;
            continue;
        }
        if file_type.is_file() {
            entries.push(PackageEntry::new(relative_path, absolute_path, PackageEntryKind::File));
            continue;
        }

        return Err(PluginPackageError::UnsupportedEntry { path: absolute_path });
    }

    Ok(())
}

fn canonical_relative_path(path: &Path) -> Result<String, PluginPackageError> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(PluginPackageError::InvalidEntryName { path: path.to_path_buf() });
        };
        parts.push(utf8_entry_name(value, path)?);
    }
    Ok(parts.join("/"))
}

fn utf8_entry_name<'a>(value: &'a OsStr, path: &Path) -> Result<&'a str, PluginPackageError> {
    value.to_str().ok_or_else(|| PluginPackageError::InvalidEntryName { path: path.to_path_buf() })
}

fn hash_file_content(path: &Path, hasher: &mut Sha256) -> Result<(), PluginPackageError> {
    let mut file = fs::File::open(path).map_err(|source| PluginPackageError::ReadFailed {
        entry: "package",
        path: path.to_path_buf(),
        source,
    })?;
    let metadata = file.metadata().map_err(|source| PluginPackageError::ReadFailed {
        entry: "package",
        path: path.to_path_buf(),
        source,
    })?;
    hasher.update(metadata.len().to_le_bytes());
    hasher.update(b"\0");
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file.read(&mut buffer).map_err(|source| PluginPackageError::ReadFailed {
            entry: "package",
            path: path.to_path_buf(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackageEntry {
    relative_path: PathBuf,
    absolute_path: PathBuf,
    kind: PackageEntryKind,
}

impl PackageEntry {
    fn new(relative_path: PathBuf, absolute_path: PathBuf, kind: PackageEntryKind) -> Self {
        Self { relative_path, absolute_path, kind }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PackageEntryKind {
    Directory,
    File,
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use sha2::{Digest, Sha256};

    use super::PluginPackageReader;

    #[test]
    fn reads_verified_directory_package() -> Result<(), Box<dyn Error>> {
        let package = write_package("verified", b"wasm component")?;

        let package = PluginPackageReader::read_directory_package(&package)?;
        let manifest = package.manifest();

        assert_eq!(manifest.id().as_str(), "com.elydora.verified");
        assert_eq!(manifest.name(), "Verified Plugin");
        Ok(())
    }

    #[test]
    fn rejects_package_checksum_mismatch() -> Result<(), Box<dyn Error>> {
        let package = write_package("checksum", b"wasm component")?;
        fs::write(package.join("component.wasm"), b"changed")?;

        let error = PluginPackageReader::read_directory_package(&package)
            .err()
            .ok_or_else(|| std::io::Error::other("package read succeeded"))?;

        assert!(matches!(error, super::PluginPackageError::ChecksumMismatch));
        Ok(())
    }

    #[test]
    fn rejects_package_signature_mismatch() -> Result<(), Box<dyn Error>> {
        let package = write_package("signature", b"wasm component")?;
        fs::write(package.join("signatures").join("ed25519.sig"), "aa")?;

        let error = PluginPackageReader::read_directory_package(&package)
            .err()
            .ok_or_else(|| std::io::Error::other("package read succeeded"))?;

        assert!(matches!(error, super::PluginPackageError::SignatureMismatch));
        Ok(())
    }

    #[test]
    fn rejects_non_package_directory() -> Result<(), Box<dyn Error>> {
        let package = temp_root()?.join("invalid");
        fs::create_dir_all(&package)?;

        let error = PluginPackageReader::read_directory_package(&package)
            .err()
            .ok_or_else(|| std::io::Error::other("package read succeeded"))?;

        assert!(matches!(error, super::PluginPackageError::InvalidPackageExtension { .. }));
        Ok(())
    }

    #[test]
    fn package_hash_covers_non_wasm_entries() -> Result<(), Box<dyn Error>> {
        let package = write_package("package-hash", b"wasm component")?;
        let first_hash =
            PluginPackageReader::read_directory_package(&package)?.package_hash().to_string();
        fs::write(package.join("README.md"), "updated package metadata")?;

        let second_hash =
            PluginPackageReader::read_directory_package(&package)?.package_hash().to_string();

        assert_ne!(first_hash, second_hash);
        Ok(())
    }

    fn write_package(name: &str, component: &[u8]) -> Result<PathBuf, Box<dyn Error>> {
        let package = temp_root()?.join(format!("{name}.rplug"));
        fs::create_dir_all(package.join("signatures"))?;
        fs::write(package.join("component.wasm"), component)?;

        let checksum = sha256_bytes(component);
        let signature = "b".repeat(128);
        fs::write(package.join("signatures").join("ed25519.sig"), &signature)?;
        fs::write(
            package.join("plugin.toml"),
            manifest_toml(name, checksum.as_str(), signature.as_str()),
        )?;

        Ok(package)
    }

    fn manifest_toml(name: &str, checksum: &str, signature: &str) -> String {
        format!(
            r#"
id = "com.elydora.{name}"
name = "Verified Plugin"
description = "Exports verified content."
author = "Elydora"
homepage = "https://elydora.com/plugins/{name}"
permissions = ["page:metadata"]
contributes = ["command-bar-command"]
min_ely_build = "0.1.0"
checksum = "{checksum}"

[signature]
algorithm = "ed25519"
value = "{signature}"
"#
        )
    }

    fn sha256_bytes(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn temp_root() -> Result<PathBuf, Box<dyn Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("ely-plugin-package-{nanos}"));
        fs::create_dir_all(&path)?;
        Ok(path)
    }
}
