use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use ed25519_dalek::{Signer, SigningKey};
use ely_domain::PluginManifest;
use sha2::{Digest, Sha256};

use super::plugin_signatures::signing_payload;

const KEY_ID: &str = "elydora-alpha-plugins";
const DETERMINISTIC_SIGNING_KEY: [u8; 32] = [9; 32];
const PARSABLE_SIGNATURE_VALUE: &str = concat!(
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
);

pub(crate) fn write_signed_package(
    root: &Path,
    name: &str,
    component: &[u8],
) -> Result<PathBuf, Box<dyn Error>> {
    let package = root.join(format!("{name}.rplug"));
    fs::create_dir_all(package.join("signatures"))?;
    fs::write(package.join("component.wasm"), component)?;
    sign_package_in_place(package.as_path(), name)?;
    Ok(package)
}

pub(crate) fn sign_package_in_place(package: &Path, name: &str) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(package.join("signatures"))?;

    let signing_key = SigningKey::from_bytes(&DETERMINISTIC_SIGNING_KEY);
    let public_key = hex_bytes(signing_key.verifying_key().to_bytes().as_slice());
    let component = fs::read(package.join("component.wasm"))?;
    let checksum = format!("{:x}", Sha256::digest(component.as_slice()));

    fs::write(
        package.join("plugin.toml"),
        manifest_toml(name, checksum.as_str(), public_key.as_str(), PARSABLE_SIGNATURE_VALUE),
    )?;

    let manifest_text = fs::read_to_string(package.join("plugin.toml"))?;
    let manifest = PluginManifest::from_toml(manifest_text.as_str())?;
    let payload = signing_payload(package, &manifest)?;
    let signature = signing_key.sign(payload.as_slice());
    let signature = hex_bytes(signature.to_bytes().as_slice());

    fs::write(package.join("signatures").join("ed25519.sig"), signature.as_str())?;
    fs::write(
        package.join("plugin.toml"),
        manifest_toml(name, checksum.as_str(), public_key.as_str(), signature.as_str()),
    )?;

    Ok(())
}

fn manifest_toml(name: &str, checksum: &str, public_key: &str, signature: &str) -> String {
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
key_id = "{KEY_ID}"
public_key = "{public_key}"
value = "{signature}"
"#
    )
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(hex_char(byte >> 4));
        encoded.push(hex_char(byte & 0x0f));
    }
    encoded
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'a' + value - 10) as char,
    }
}
