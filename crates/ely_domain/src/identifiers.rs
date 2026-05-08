use std::fmt;

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

entity_id!(TabId, "tab");
entity_id!(SpaceId, "space");
entity_id!(ProfileId, "profile");
entity_id!(SplitId, "split");
entity_id!(WebViewId, "webview");
entity_id!(DownloadId, "download");
entity_id!(BookmarkId, "bookmark");
entity_id!(ReadingListId, "reading");
entity_id!(NoteId, "note");
