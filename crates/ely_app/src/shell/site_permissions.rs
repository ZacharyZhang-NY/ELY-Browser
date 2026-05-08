use ely_domain::{SiteOrigin, SitePermissionDecision, SitePermissionFeature};
use gpui::Context;

use super::{ElyShell, ShellState};

impl ElyShell {
    pub(super) fn set_site_permission(
        &mut self,
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        decision: SitePermissionDecision,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.set_site_permission(origin, feature, decision).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn revoke_site_permission(
        &mut self,
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.revoke_site_permission(&origin, feature).is_ok()
        {
            cx.notify();
        }
    }
}
