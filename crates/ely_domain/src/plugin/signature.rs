use serde::Deserialize;

use crate::DomainError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginSignatureAlgorithm {
    Ed25519,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginSignature {
    algorithm: PluginSignatureAlgorithm,
    key_id: String,
    public_key: String,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPluginSignature {
    pub(super) algorithm: String,
    pub(super) key_id: String,
    pub(super) public_key: String,
    pub(super) value: String,
}

impl PluginSignature {
    pub(super) fn from_raw(raw: RawPluginSignature) -> Result<Self, DomainError> {
        let algorithm = PluginSignatureAlgorithm::parse(raw.algorithm.as_str())?;
        let key_id = plugin_signature_key_id(raw.key_id)?;
        let public_key = plugin_signature_public_key(raw.public_key)?;
        let value = plugin_signature_value(&algorithm, raw.value)?;
        Ok(Self { algorithm, key_id, public_key, value })
    }

    #[must_use]
    pub fn algorithm(&self) -> &PluginSignatureAlgorithm {
        &self.algorithm
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl PluginSignatureAlgorithm {
    fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "ed25519" => Ok(Self::Ed25519),
            _ => Err(DomainError::InvalidPluginSignatureAlgorithm { value: value.to_string() }),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
        }
    }
}

fn plugin_signature_key_id(value: impl Into<String>) -> Result<String, DomainError> {
    let value = super::non_empty_plugin_field("signature.key_id", value)?;
    let valid = (3..=128).contains(&value.len())
        && value.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '-' | '_')
        });
    if !valid {
        return Err(DomainError::InvalidPluginSignatureKeyId { value });
    }
    Ok(value)
}

fn plugin_signature_public_key(value: impl Into<String>) -> Result<String, DomainError> {
    let value = super::non_empty_plugin_field("signature.public_key", value)?;
    if !super::is_hex_of_len(value.as_str(), 64) {
        return Err(DomainError::InvalidPluginSignaturePublicKey { value });
    }
    Ok(value.to_ascii_lowercase())
}

fn plugin_signature_value(
    algorithm: &PluginSignatureAlgorithm,
    value: impl Into<String>,
) -> Result<String, DomainError> {
    let value = super::non_empty_plugin_field("signature", value)?;
    let valid = match algorithm {
        PluginSignatureAlgorithm::Ed25519 => super::is_hex_of_len(value.as_str(), 128),
    };
    if !valid {
        return Err(DomainError::InvalidPluginSignature { value });
    }
    Ok(value.to_ascii_lowercase())
}
