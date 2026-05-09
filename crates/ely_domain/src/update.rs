#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UpdatePolicy {
    #[default]
    Automatic,
    Manual,
}

impl UpdatePolicy {
    pub const ALL: &[Self] = &[Self::Automatic, Self::Manual];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::Manual => "Manual",
        }
    }

    #[must_use]
    pub fn detail(self) -> &'static str {
        match self {
            Self::Automatic => "Use release manifests as the automatic update source.",
            Self::Manual => "Keep release manifest checks user-initiated.",
        }
    }
}
