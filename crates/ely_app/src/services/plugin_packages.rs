use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use ely_domain::PluginManifest;
use sha2::{Digest, Sha256};
use thiserror::Error;

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

    #[error("plugin package checksum mismatch for component.wasm")]
    ChecksumMismatch,

    #[error("plugin package signature mismatch for signatures/ed25519.sig")]
    SignatureMismatch,

    #[error(transparent)]
    Manifest(#[from] ely_domain::DomainError),
}

pub struct PluginPackageReader;

impl PluginPackageReader {
    pub fn read_directory_package(path: &Path) -> Result<PluginManifest, PluginPackageError> {
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

        Ok(manifest)
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

        let manifest = PluginPackageReader::read_directory_package(&package)?;

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
