use std::collections::{BTreeMap, BTreeSet};

use gpui::{KeyBinding, Keystroke, NoAction};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    SHORTCUT_ACTIONS, SHORTCUT_BINDINGS, ShortcutAction, ShortcutConflict, ShortcutPlatform,
    display_keystroke, key_binding_for_action,
};

pub(crate) const ELYKEYS_FILE_EXTENSION: &str = "elykeys";
const SHORTCUT_PROFILE_SCHEMA_VERSION: u16 = 1;
const SHORTCUT_PLATFORMS: &[ShortcutPlatform] =
    &[ShortcutPlatform::Macos, ShortcutPlatform::WindowsLinux];

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShortcutProfile {
    schema_version: u16,
    bindings: Vec<ShortcutProfileBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShortcutProfileBinding {
    action: ShortcutAction,
    platform: ShortcutPlatform,
    keystrokes: Vec<String>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub(crate) enum ShortcutProfileError {
    #[error("Shortcut profile schema version {actual} is unsupported.")]
    UnsupportedSchemaVersion { actual: u16 },
    #[error("Shortcut profile has duplicate entry for {platform} {action}.")]
    DuplicateEntry { action: &'static str, platform: &'static str },
    #[error("Shortcut profile is missing entry for {platform} {action}.")]
    MissingEntry { action: &'static str, platform: &'static str },
    #[error("Shortcut profile has empty key for {platform} {action}.")]
    EmptyKeystroke { action: &'static str, platform: &'static str },
    #[error("Shortcut profile key `{keystroke}` for {platform} {action} is invalid.")]
    InvalidKeystroke { action: &'static str, platform: &'static str, keystroke: String },
    #[error("Shortcut profile repeats `{keystroke}` for {platform} {action}.")]
    DuplicateKeystroke { action: &'static str, platform: &'static str, keystroke: String },
    #[error("Shortcut profile maps `{keystroke}` on {platform} to multiple actions.")]
    ConflictingKeystroke { platform: &'static str, keystroke: String },
    #[error("Shortcut profile JSON is invalid: {0}")]
    InvalidJson(String),
}

impl Default for ShortcutProfile {
    fn default() -> Self {
        Self::default_profile()
    }
}

impl ShortcutProfile {
    pub(crate) fn default_profile() -> Self {
        let mut bindings = Vec::new();

        for action in SHORTCUT_ACTIONS {
            for platform in SHORTCUT_PLATFORMS {
                bindings.push(ShortcutProfileBinding {
                    action: *action,
                    platform: *platform,
                    keystrokes: default_keystrokes(*action, *platform),
                });
            }
        }

        Self { schema_version: SHORTCUT_PROFILE_SCHEMA_VERSION, bindings }
    }

    pub(crate) fn to_json(&self) -> Result<String, ShortcutProfileError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| ShortcutProfileError::InvalidJson(error.to_string()))
    }

    pub(crate) fn bindings_for_action(
        &self,
        action: ShortcutAction,
        platform: ShortcutPlatform,
    ) -> &[String] {
        self.bindings
            .iter()
            .find(|binding| binding.action == action && binding.platform == platform)
            .map_or(&[], |binding| binding.keystrokes.as_slice())
    }

    pub(crate) fn display_bindings_for_action(
        &self,
        action: ShortcutAction,
        platform: ShortcutPlatform,
    ) -> String {
        let bindings = self
            .bindings_for_action(action, platform)
            .iter()
            .map(|keystroke| display_keystroke(keystroke, platform))
            .collect::<Vec<_>>();

        if bindings.is_empty() { "Unassigned".to_string() } else { bindings.join(" / ") }
    }

    pub(crate) fn conflicts(&self) -> Vec<ShortcutConflict> {
        let mut bindings_by_key: BTreeMap<(ShortcutPlatform, &str), Vec<ShortcutAction>> =
            BTreeMap::new();

        for binding in &self.bindings {
            for keystroke in &binding.keystrokes {
                bindings_by_key
                    .entry((binding.platform, keystroke.as_str()))
                    .or_default()
                    .push(binding.action);
            }
        }

        bindings_by_key
            .into_iter()
            .filter_map(|((platform, keystroke), actions)| {
                (actions.len() > 1).then_some(ShortcutConflict {
                    platform,
                    keystroke: keystroke.to_string(),
                    actions,
                })
            })
            .collect()
    }

    fn validate(mut self) -> Result<Self, ShortcutProfileError> {
        if self.schema_version != SHORTCUT_PROFILE_SCHEMA_VERSION {
            return Err(ShortcutProfileError::UnsupportedSchemaVersion {
                actual: self.schema_version,
            });
        }

        let mut seen_entries = BTreeSet::new();
        let mut bindings_by_key = BTreeMap::<(ShortcutPlatform, String), ShortcutAction>::new();

        for binding in &mut self.bindings {
            if !seen_entries.insert((binding.action, binding.platform)) {
                return Err(ShortcutProfileError::DuplicateEntry {
                    action: binding.action.label(),
                    platform: binding.platform.label(),
                });
            }

            normalize_keystrokes(binding)?;
            for keystroke in &binding.keystrokes {
                if let Some(existing_action) =
                    bindings_by_key.insert((binding.platform, keystroke.clone()), binding.action)
                    && existing_action != binding.action
                {
                    return Err(ShortcutProfileError::ConflictingKeystroke {
                        platform: binding.platform.label(),
                        keystroke: keystroke.clone(),
                    });
                }
            }
        }

        for action in SHORTCUT_ACTIONS {
            for platform in SHORTCUT_PLATFORMS {
                if !seen_entries.contains(&(*action, *platform)) {
                    return Err(ShortcutProfileError::MissingEntry {
                        action: action.label(),
                        platform: platform.label(),
                    });
                }
            }
        }

        self.bindings.sort_by_key(|binding| (binding.action, binding.platform));
        Ok(self)
    }
}

pub(crate) fn parse_shortcut_profile_json(
    profile_json: &str,
) -> Result<ShortcutProfile, ShortcutProfileError> {
    serde_json::from_str::<ShortcutProfile>(profile_json)
        .map_err(|error| ShortcutProfileError::InvalidJson(error.to_string()))?
        .validate()
}

pub(crate) fn key_bindings_for_profile(profile: &ShortcutProfile) -> Vec<KeyBinding> {
    profile
        .bindings
        .iter()
        .flat_map(|binding| {
            binding
                .keystrokes
                .iter()
                .map(move |keystroke| key_binding_for_action(binding.action, keystroke))
        })
        .collect()
}

pub(crate) fn shortcut_rebinding_key_bindings(
    previous: &ShortcutProfile,
    next: &ShortcutProfile,
) -> Vec<KeyBinding> {
    let mut removed_keys = BTreeSet::new();

    for binding in SHORTCUT_BINDINGS {
        removed_keys.insert(binding.keystroke().to_string());
    }
    for binding in &previous.bindings {
        for keystroke in &binding.keystrokes {
            removed_keys.insert(keystroke.clone());
        }
    }

    let mut key_bindings = removed_keys
        .into_iter()
        .map(|keystroke| KeyBinding::new(&keystroke, NoAction {}, None))
        .collect::<Vec<_>>();
    key_bindings.extend(key_bindings_for_profile(next));
    key_bindings
}

fn default_keystrokes(action: ShortcutAction, platform: ShortcutPlatform) -> Vec<String> {
    let mut keystrokes = SHORTCUT_BINDINGS
        .iter()
        .filter(|binding| binding.action() == action && binding.platform() == platform)
        .map(|binding| binding.keystroke().to_string())
        .collect::<Vec<_>>();
    keystrokes.sort();
    keystrokes
}

fn normalize_keystrokes(binding: &mut ShortcutProfileBinding) -> Result<(), ShortcutProfileError> {
    let mut seen_keystrokes = BTreeSet::new();

    for keystroke in &mut binding.keystrokes {
        let normalized = keystroke.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Err(ShortcutProfileError::EmptyKeystroke {
                action: binding.action.label(),
                platform: binding.platform.label(),
            });
        }
        if Keystroke::parse(&normalized).is_err() {
            return Err(ShortcutProfileError::InvalidKeystroke {
                action: binding.action.label(),
                platform: binding.platform.label(),
                keystroke: keystroke.clone(),
            });
        }
        if !seen_keystrokes.insert(normalized.clone()) {
            return Err(ShortcutProfileError::DuplicateKeystroke {
                action: binding.action.label(),
                platform: binding.platform.label(),
                keystroke: normalized,
            });
        }
        *keystroke = normalized;
    }

    binding.keystrokes.sort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ShortcutProfile, ShortcutProfileError, parse_shortcut_profile_json};
    use serde_json::Value;

    #[test]
    fn default_shortcut_profile_round_trips_as_json() -> Result<(), ShortcutProfileError> {
        let profile = ShortcutProfile::default_profile();
        let json = profile.to_json()?;
        let parsed = parse_shortcut_profile_json(&json)?;

        assert_eq!(parsed, profile);
        Ok(())
    }

    #[test]
    fn shortcut_profile_rejects_conflicting_keys() -> Result<(), ShortcutProfileError> {
        let profile = ShortcutProfile::default_profile();
        let json = profile.to_json()?.replace("\"ctrl-h\"", "\"ctrl-shift-j\"");

        let error = parse_shortcut_profile_json(&json);

        assert!(matches!(error, Err(ShortcutProfileError::ConflictingKeystroke { .. })));
        Ok(())
    }

    #[test]
    fn shortcut_profile_rejects_missing_entries() -> Result<(), ShortcutProfileError> {
        let profile = ShortcutProfile::default_profile();
        let mut value = serde_json::from_str::<Value>(&profile.to_json()?)
            .map_err(|error| ShortcutProfileError::InvalidJson(error.to_string()))?;
        let bindings = value
            .get_mut("bindings")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| ShortcutProfileError::InvalidJson("missing bindings".to_string()))?;
        bindings.pop();
        let json = serde_json::to_string(&value)
            .map_err(|error| ShortcutProfileError::InvalidJson(error.to_string()))?;

        let error = parse_shortcut_profile_json(&json);

        assert!(matches!(error, Err(ShortcutProfileError::MissingEntry { .. })));
        Ok(())
    }
}
