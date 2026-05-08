use std::{
    ffi::OsStr,
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use ely_domain::PluginManifest;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginSignatureVerificationError {
    #[error("plugin signature public key is invalid for key {key_id}")]
    InvalidPublicKey { key_id: String },

    #[error("plugin signature value is invalid for key {key_id}")]
    InvalidSignature { key_id: String },

    #[error("plugin signature verification failed for key {key_id}")]
    VerificationFailed { key_id: String },

    #[error("failed to read plugin package entry {entry}: {path}")]
    ReadFailed { entry: &'static str, path: PathBuf, source: io::Error },

    #[error("plugin package entry name must be valid utf-8: {path}")]
    InvalidEntryName { path: PathBuf },

    #[error("plugin package entry cannot be signed through a symlink: {path}")]
    SymlinkEntry { path: PathBuf },

    #[error("plugin package entry type is unsupported: {path}")]
    UnsupportedEntry { path: PathBuf },
}

pub struct PluginSignatureVerifier;

impl PluginSignatureVerifier {
    pub fn verify(
        package_root: &Path,
        manifest: &PluginManifest,
    ) -> Result<(), PluginSignatureVerificationError> {
        let key_id = manifest.signature().key_id().to_string();
        let public_key =
            decode_hex_array::<32>(manifest.signature().public_key()).ok_or_else(|| {
                PluginSignatureVerificationError::InvalidPublicKey { key_id: key_id.clone() }
            })?;
        let signature = decode_hex_array::<64>(manifest.signature().value()).ok_or_else(|| {
            PluginSignatureVerificationError::InvalidSignature { key_id: key_id.clone() }
        })?;
        let key = VerifyingKey::from_bytes(&public_key).map_err(|_| {
            PluginSignatureVerificationError::InvalidPublicKey { key_id: key_id.clone() }
        })?;
        let signature = Signature::from_bytes(&signature);
        let payload = signing_payload(package_root, manifest)?;

        key.verify(payload.as_slice(), &signature)
            .map_err(|_| PluginSignatureVerificationError::VerificationFailed { key_id })
    }
}

pub(crate) fn signing_payload(
    package_root: &Path,
    manifest: &PluginManifest,
) -> Result<Vec<u8>, PluginSignatureVerificationError> {
    let mut payload = Vec::new();
    append_field(&mut payload, "format", "ely.rplug.signature.v1");
    append_field(&mut payload, "id", manifest.id().as_str());
    append_field(&mut payload, "name", manifest.name());
    append_field(&mut payload, "description", manifest.description());
    append_field(&mut payload, "author", manifest.author());
    append_field(&mut payload, "homepage", manifest.homepage());
    append_list(
        &mut payload,
        "permissions",
        manifest.permissions().iter().map(|permission| permission.as_str()),
    );
    append_list(
        &mut payload,
        "contributes",
        manifest.contributes().iter().map(|contribution| contribution.as_str()),
    );
    append_field(&mut payload, "min_ely_build", manifest.min_ely_build().to_string().as_str());
    append_field(&mut payload, "checksum", manifest.checksum());
    append_field(&mut payload, "signature_algorithm", manifest.signature().algorithm().as_str());
    append_field(&mut payload, "signature_key_id", manifest.signature().key_id());
    append_field(&mut payload, "signature_public_key", manifest.signature().public_key());
    append_field(&mut payload, "package_files_sha256", package_files_hash(package_root)?.as_str());
    Ok(payload)
}

fn package_files_hash(package_root: &Path) -> Result<String, PluginSignatureVerificationError> {
    let mut entries = Vec::new();
    collect_signed_entries(package_root, Path::new(""), &mut entries)?;
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    let mut hasher = Sha256::new();
    for entry in entries {
        let relative_path = canonical_relative_path(entry.relative_path.as_path())?;
        match entry.kind {
            SignedEntryKind::Directory => {
                hasher.update(b"dir\0");
                hasher.update(relative_path.as_bytes());
                hasher.update(b"\0");
            }
            SignedEntryKind::File => {
                hasher.update(b"file\0");
                hasher.update(relative_path.as_bytes());
                hasher.update(b"\0");
                hash_file_content(entry.absolute_path.as_path(), &mut hasher)?;
            }
        }
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_signed_entries(
    root: &Path,
    relative_dir: &Path,
    entries: &mut Vec<SignedEntry>,
) -> Result<(), PluginSignatureVerificationError> {
    let absolute_dir = root.join(relative_dir);
    for entry in fs::read_dir(absolute_dir.as_path()).map_err(|source| {
        PluginSignatureVerificationError::ReadFailed {
            entry: "package",
            path: absolute_dir.clone(),
            source,
        }
    })? {
        let entry = entry.map_err(|source| PluginSignatureVerificationError::ReadFailed {
            entry: "package",
            path: absolute_dir.clone(),
            source,
        })?;
        let absolute_path = entry.path();
        let relative_path = relative_dir.join(entry.file_name());
        if excluded_signature_entry(relative_path.as_path()) {
            continue;
        }

        let metadata = fs::symlink_metadata(absolute_path.as_path()).map_err(|source| {
            PluginSignatureVerificationError::ReadFailed {
                entry: "package",
                path: absolute_path.clone(),
                source,
            }
        })?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            return Err(PluginSignatureVerificationError::SymlinkEntry { path: absolute_path });
        }
        if file_type.is_dir() {
            entries.push(SignedEntry::new(
                relative_path.clone(),
                absolute_path,
                SignedEntryKind::Directory,
            ));
            collect_signed_entries(root, relative_path.as_path(), entries)?;
            continue;
        }
        if file_type.is_file() {
            entries.push(SignedEntry::new(relative_path, absolute_path, SignedEntryKind::File));
            continue;
        }

        return Err(PluginSignatureVerificationError::UnsupportedEntry { path: absolute_path });
    }

    Ok(())
}

fn excluded_signature_entry(path: &Path) -> bool {
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(value)) if value == OsStr::new("plugin.toml") => true,
        Some(Component::Normal(value)) if value == OsStr::new("signatures") => true,
        _ => false,
    }
}

fn canonical_relative_path(path: &Path) -> Result<String, PluginSignatureVerificationError> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(PluginSignatureVerificationError::InvalidEntryName {
                path: path.to_path_buf(),
            });
        };
        parts.push(utf8_entry_name(value, path)?);
    }
    Ok(parts.join("/"))
}

fn utf8_entry_name<'a>(
    value: &'a OsStr,
    path: &Path,
) -> Result<&'a str, PluginSignatureVerificationError> {
    value.to_str().ok_or_else(|| PluginSignatureVerificationError::InvalidEntryName {
        path: path.to_path_buf(),
    })
}

fn hash_file_content(
    path: &Path,
    hasher: &mut Sha256,
) -> Result<(), PluginSignatureVerificationError> {
    let mut file =
        fs::File::open(path).map_err(|source| PluginSignatureVerificationError::ReadFailed {
            entry: "package",
            path: path.to_path_buf(),
            source,
        })?;
    let metadata =
        file.metadata().map_err(|source| PluginSignatureVerificationError::ReadFailed {
            entry: "package",
            path: path.to_path_buf(),
            source,
        })?;
    hasher.update(metadata.len().to_le_bytes());
    hasher.update(b"\0");
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file.read(&mut buffer).map_err(|source| {
            PluginSignatureVerificationError::ReadFailed {
                entry: "package",
                path: path.to_path_buf(),
                source,
            }
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(())
}

fn append_field(payload: &mut Vec<u8>, name: &str, value: &str) {
    payload.extend_from_slice(name.as_bytes());
    payload.extend_from_slice(b"\0");
    payload.extend_from_slice(value.len().to_string().as_bytes());
    payload.extend_from_slice(b"\0");
    payload.extend_from_slice(value.as_bytes());
    payload.extend_from_slice(b"\0");
}

fn append_list<'a>(payload: &mut Vec<u8>, name: &str, values: impl Iterator<Item = &'a str>) {
    let values = values.collect::<Vec<_>>();
    payload.extend_from_slice(name.as_bytes());
    payload.extend_from_slice(b"\0");
    payload.extend_from_slice(values.len().to_string().as_bytes());
    payload.extend_from_slice(b"\0");
    for value in values {
        payload.extend_from_slice(value.len().to_string().as_bytes());
        payload.extend_from_slice(b"\0");
        payload.extend_from_slice(value.as_bytes());
        payload.extend_from_slice(b"\0");
    }
}

fn decode_hex_array<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }

    let mut output = [0_u8; N];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        output[index] = (high << 4) | low;
    }
    Some(output)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignedEntry {
    relative_path: PathBuf,
    absolute_path: PathBuf,
    kind: SignedEntryKind,
}

impl SignedEntry {
    fn new(relative_path: PathBuf, absolute_path: PathBuf, kind: SignedEntryKind) -> Self {
        Self { relative_path, absolute_path, kind }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SignedEntryKind {
    Directory,
    File,
}
