/// Connection lifecycle of the cloud sync client.
///
/// The previous variant set was a single `SignedOut`, which made the
/// Sync settings page report "Local-only · sign-in coming soon" even
/// after the user dropped a bearer token in. The state machine now
/// progresses from `SignedOut` → `SignedIn` (token present, sync not
/// yet attempted) → `SyncReady` (last upload landed) or `SyncError`
/// (last upload failed). `AwaitingDeviceApproval` is the dedicated
/// state for the 403 the worker returns when the device-id bound to
/// the session has not yet been approved by another already-approved
/// device — surfacing it as its own variant lets the UI explain the
/// gap instead of bucketing it into a generic error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncConnectionState {
    SignedOut,
    SignedIn,
    AwaitingDeviceApproval,
    SyncReady { last_synced_at_secs: u64 },
    SyncError { message: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncObjectKind {
    Spaces,
    Tabs,
    Bookmarks,
    Notes,
    ReadingList,
    Profiles,
    SitePermissions,
    History,
    PluginSettings,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SyncObjectPolicy {
    #[default]
    Enabled,
    Paused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncObjectState {
    LocalOnly,
    Paused,
    PrivacyControlled,
    Synced,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncObjectStatus {
    kind: SyncObjectKind,
    local_count: usize,
    state: SyncObjectState,
    policy: SyncObjectPolicy,
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
        Self::with_policy(kind, local_count, state, SyncObjectPolicy::Enabled)
    }

    #[must_use]
    pub fn with_policy(
        kind: SyncObjectKind,
        local_count: usize,
        state: SyncObjectState,
        policy: SyncObjectPolicy,
    ) -> Self {
        Self { kind, local_count, state, policy }
    }

    #[must_use]
    pub fn kind(&self) -> SyncObjectKind {
        self.kind
    }

    #[must_use]
    pub fn local_count(&self) -> usize {
        self.local_count
    }

    #[must_use]
    pub fn state(&self) -> SyncObjectState {
        self.state
    }

    #[must_use]
    pub fn policy(&self) -> SyncObjectPolicy {
        self.policy
    }
}

impl SyncStatus {
    #[must_use]
    pub fn new(connection: SyncConnectionState, objects: Vec<SyncObjectStatus>) -> Self {
        let failed_objects = match &connection {
            SyncConnectionState::SyncError { .. } => 1,
            _ => 0,
        };
        Self { connection, pending_objects: 0, failed_objects, objects }
    }

    /// Convenience constructor preserved for tests + call sites that
    /// have not been migrated to [`SyncStatus::new`] yet. Same result
    /// as `SyncStatus::new(SyncConnectionState::SignedOut, objects)`.
    #[must_use]
    pub fn signed_out(objects: Vec<SyncObjectStatus>) -> Self {
        Self::new(SyncConnectionState::SignedOut, objects)
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
