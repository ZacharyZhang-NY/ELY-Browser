#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncConnectionState {
    SignedOut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncObjectKind {
    Spaces,
    Tabs,
    Bookmarks,
    ReadingList,
    Profiles,
    SitePermissions,
    History,
    PluginSettings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncObjectState {
    LocalOnly,
    PrivacyControlled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncObjectStatus {
    kind: SyncObjectKind,
    local_count: usize,
    state: SyncObjectState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncStatus {
    connection: SyncConnectionState,
    pending_objects: usize,
    failed_objects: usize,
    objects: Vec<SyncObjectStatus>,
}

impl SyncObjectStatus {
    #[must_use]
    pub fn new(kind: SyncObjectKind, local_count: usize, state: SyncObjectState) -> Self {
        Self { kind, local_count, state }
    }

    #[must_use]
    pub fn kind(&self) -> &SyncObjectKind {
        &self.kind
    }

    #[must_use]
    pub fn local_count(&self) -> usize {
        self.local_count
    }

    #[must_use]
    pub fn state(&self) -> &SyncObjectState {
        &self.state
    }
}

impl SyncStatus {
    #[must_use]
    pub fn signed_out(objects: Vec<SyncObjectStatus>) -> Self {
        Self {
            connection: SyncConnectionState::SignedOut,
            pending_objects: 0,
            failed_objects: 0,
            objects,
        }
    }

    #[must_use]
    pub fn connection(&self) -> &SyncConnectionState {
        &self.connection
    }

    #[must_use]
    pub fn pending_objects(&self) -> usize {
        self.pending_objects
    }

    #[must_use]
    pub fn failed_objects(&self) -> usize {
        self.failed_objects
    }

    #[must_use]
    pub fn objects(&self) -> &[SyncObjectStatus] {
        &self.objects
    }
}
