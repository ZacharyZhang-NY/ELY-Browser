use crate::ProfileId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileKind {
    Standard,
    Private,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    id: ProfileId,
    name: String,
    color_hex: u32,
    kind: ProfileKind,
}

impl Profile {
    #[must_use]
    pub fn new(name: impl Into<String>, color_hex: u32, kind: ProfileKind) -> Self {
        Self { id: ProfileId::new(), name: name.into(), color_hex, kind }
    }

    #[must_use]
    pub fn id(&self) -> &ProfileId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn color_hex(&self) -> u32 {
        self.color_hex
    }

    #[must_use]
    pub fn kind(&self) -> &ProfileKind {
        &self.kind
    }
}
