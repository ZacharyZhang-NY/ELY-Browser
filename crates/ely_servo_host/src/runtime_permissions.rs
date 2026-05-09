use std::{cell::RefCell, collections::HashMap, rc::Rc};

use ely_domain::{ProfileId, TabId};

use crate::PermissionDecision;

pub(super) type PermissionStore = Rc<RefCell<HashMap<PermissionKey, PermissionDecision>>>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct PermissionKey {
    profile_id: ProfileId,
    tab_id: TabId,
    feature: String,
}

impl PermissionKey {
    pub(super) fn new(profile_id: ProfileId, tab_id: TabId, feature: String) -> Self {
        Self { profile_id, tab_id, feature }
    }
}
