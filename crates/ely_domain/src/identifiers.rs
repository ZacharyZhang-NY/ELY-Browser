use std::fmt;

use crate::DomainError;
use uuid::Uuid;

macro_rules! entity_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, Uuid::now_v7().simple()))
            }

            pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
                let value = value.into();
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(DomainError::EmptyField { field: concat!($prefix, "_id") });
                }
                if !is_valid_entity_id(trimmed, $prefix) {
                    return Err(DomainError::InvalidEntityId {
                        kind: $prefix,
                        value: trimmed.to_string(),
                    });
                }

                Ok(Self(trimmed.to_string()))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

fn is_valid_entity_id(value: &str, prefix: &str) -> bool {
    let Some(suffix) = value.strip_prefix(prefix).and_then(|value| value.strip_prefix('_')) else {
        return false;
    };

    suffix.len() == 32 && suffix.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit())
}

entity_id!(TabId, "tab");
entity_id!(SpaceId, "space");
entity_id!(ProfileId, "profile");
entity_id!(TabGroupId, "tab_group");
entity_id!(SplitId, "split");
entity_id!(WebViewId, "webview");
entity_id!(DownloadId, "download");
entity_id!(BookmarkId, "bookmark");
entity_id!(ReadingListId, "reading");
entity_id!(NoteId, "note");

#[cfg(test)]
mod tests {
    use super::{ProfileId, TabId};
    use crate::DomainError;

    #[test]
    fn parses_existing_entity_id() -> Result<(), DomainError> {
        let profile_id = ProfileId::new();
        let parsed = ProfileId::parse(profile_id.as_str())?;

        assert_eq!(parsed, profile_id);
        Ok(())
    }

    #[test]
    fn rejects_wrong_entity_prefix() {
        let tab_id = TabId::new();
        let result = ProfileId::parse(tab_id.as_str());

        assert!(
            matches!(result, Err(DomainError::InvalidEntityId { kind, .. }) if kind == "profile")
        );
    }

    #[test]
    fn rejects_empty_entity_id() {
        let result = ProfileId::parse(" ");

        assert_eq!(result, Err(DomainError::EmptyField { field: "profile_id" }));
    }
}
