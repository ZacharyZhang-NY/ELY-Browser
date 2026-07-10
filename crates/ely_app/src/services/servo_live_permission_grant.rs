use ely_domain::{ProfileId, SiteOrigin, SitePermissionFeature};

use super::wire::LivePermissionConsumption;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ServoLivePermissionGrant {
    profile_id: ProfileId,
    origin: SiteOrigin,
    feature: SitePermissionFeature,
    grant_revision: u64,
}

impl ServoLivePermissionGrant {
    pub(crate) fn new(
        profile_id: ProfileId,
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        grant_revision: u64,
    ) -> Self {
        Self { profile_id, origin, feature, grant_revision }
    }

    pub(crate) fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub(crate) fn origin(&self) -> &SiteOrigin {
        &self.origin
    }

    pub(crate) fn feature(&self) -> SitePermissionFeature {
        self.feature
    }

    pub(crate) fn grant_revision(&self) -> u64 {
        self.grant_revision
    }
}

impl TryFrom<LivePermissionConsumption> for ServoLivePermissionGrant {
    type Error = ely_domain::DomainError;

    fn try_from(consumed: LivePermissionConsumption) -> Result<Self, Self::Error> {
        Ok(Self {
            profile_id: ProfileId::parse(consumed.profile_id)?,
            origin: SiteOrigin::parse(consumed.origin)?,
            feature: SitePermissionFeature::parse(&consumed.feature)?,
            grant_revision: consumed.grant_revision,
        })
    }
}
