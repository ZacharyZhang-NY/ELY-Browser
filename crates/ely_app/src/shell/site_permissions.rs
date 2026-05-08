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

    pub(super) fn request_clear_site_permissions_confirmation(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && let Ok(snapshot) = core.snapshot()
        {
            self.site_permissions_clear_confirmation = Some(snapshot.active_profile_id);
            cx.notify();
        }
    }

    pub(super) fn cancel_clear_site_permissions_confirmation(&mut self, cx: &mut Context<Self>) {
        self.site_permissions_clear_confirmation = None;
        cx.notify();
    }

    pub(super) fn clear_active_profile_site_permissions(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && let Ok(snapshot) = core.snapshot()
            && self.site_permissions_clear_confirmation.as_ref()
                == Some(&snapshot.active_profile_id)
            && core.clear_active_profile_site_permissions().is_ok()
        {
            self.site_permissions_clear_confirmation = None;
            cx.notify();
        }
    }
}
