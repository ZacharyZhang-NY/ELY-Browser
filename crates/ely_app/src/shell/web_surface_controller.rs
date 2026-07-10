use std::{collections::HashMap, sync::Arc};

use ely_browser_core::{BrowserCore, BrowserSnapshot};
use ely_domain::{BrowserTab, ProfileKind, TabId, UrlText};
use gpui::{AnyElement, Bounds, Context, Pixels, Point};

use crate::services::{ProfileDataMode, servo_live::ServoLivePermissionGrant};

use super::{
    ElyShell,
    web_surface_metadata::WebSurfacePageMetadata,
    web_surface_permissions::{
        WebSurfaceSitePermission, web_surface_site_permissions_for_core_tab,
    },
    web_surface_runtime::{WebSurfaceUrlChange, WebSurfaceUrlChangeKind},
    web_surface_state::{WebSurfaceInputOutcome, WebSurfaceState},
    web_surface_view::{
        render_failed_web_surface, render_loading_web_surface, render_ready_web_surface,
    },
};

impl ElyShell {
    pub(super) fn render_external_web_canvas(
        &mut self,
        tab: &BrowserTab,
        snapshot: &BrowserSnapshot,
        bottom_corner_radius: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = cx.entity().clone();
        let Some(profile_data_mode) = profile_data_mode_for(tab, snapshot) else {
            return render_failed_web_surface(tab, "Profile context is unavailable.", state_entity);
        };

        match self.web_surfaces.state_for_scope(tab.id(), tab.profile_id(), profile_data_mode) {
            Some(WebSurfaceState::Ready(frame)) => {
                render_ready_web_surface(frame, tab, state_entity, bottom_corner_radius)
            }
            Some(WebSurfaceState::Failed { message, .. }) => {
                render_failed_web_surface(tab, message.as_str(), state_entity)
            }
            Some(WebSurfaceState::Loading { previous_frame: Some(frame), .. }) => {
                render_ready_web_surface(frame, tab, state_entity, bottom_corner_radius)
            }
            Some(WebSurfaceState::Loading { previous_frame: None, .. }) | None => {
                render_loading_web_surface(tab, state_entity, bottom_corner_radius)
            }
        }
    }

    pub(super) fn tick_external_web_surfaces(&mut self, cx: &mut Context<Self>) -> bool {
        let (visible_tab_ids, open_tab_ids, visible_tabs) = match &self.state {
            super::ShellState::Ready(core) => {
                let visible_tabs = core.visible_content_tabs().unwrap_or_else(|_| Vec::new());
                let visible_tab_ids = visible_tabs.iter().map(|tab| tab.id().clone()).collect();
                (visible_tab_ids, core.open_tab_ids(), visible_web_surface_tabs(core, visible_tabs))
            }
            super::ShellState::StartupError(_) => (Vec::new(), Vec::new(), Vec::new()),
        };
        self.web_surfaces.retain_tabs(&open_tab_ids);
        let mut url_changed = self.ensure_visible_web_surfaces(visible_tabs);
        let result = self.web_surfaces.tick(&visible_tab_ids);
        for url_change in result.url_changes {
            url_changed |= self.apply_web_surface_url_change(url_change);
        }
        let mut metadata_changed = false;
        for metadata in result.page_metadata {
            metadata_changed |= self.apply_web_surface_page_metadata(metadata);
        }
        let mut permission_changed = false;
        if let super::ShellState::Ready(core) = &mut self.state {
            for grant in result.permission_transfers {
                permission_changed |= core
                    .transfer_site_permission_once(
                        grant.profile_id(),
                        grant.origin(),
                        grant.feature(),
                        grant.grant_revision(),
                    )
                    .unwrap_or(false);
            }
            for consumed in result.permission_consumptions {
                permission_changed |= apply_permission_consumption(core, &consumed);
            }
        }
        let sync_changed = self.drain_sync_updates();
        if url_changed || metadata_changed {
            self.schedule_cloud_sync_upload(cx);
        }
        result.changed || url_changed || metadata_changed || permission_changed || sync_changed
    }

    pub(super) fn external_web_surface_tick_delay(&self) -> std::time::Duration {
        let visible_tab_ids = match &self.state {
            super::ShellState::Ready(core) => {
                core.visible_content_tab_ids().unwrap_or_else(|_| Vec::new())
            }
            super::ShellState::StartupError(_) => Vec::new(),
        };
        self.web_surfaces.next_tick_delay(&visible_tab_ids)
    }

    fn flush_external_web_surface_tick(&mut self, cx: &mut Context<Self>) {
        if self.tick_external_web_surfaces(cx) {
            cx.notify();
        }
    }

    pub(super) fn record_external_web_viewport(
        &mut self,
        tab_id: TabId,
        bounds: Bounds<Pixels>,
        scale_factor: f32,
        cx: &mut Context<Self>,
    ) {
        if self.web_surfaces.record_viewport_size(&tab_id, bounds, scale_factor)
            == WebSurfaceInputOutcome::Applied
        {
            self.flush_external_web_surface_tick(cx);
        }
    }

    pub(super) fn scroll_external_web_viewport(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        delta: Point<Pixels>,
        position: Point<Pixels>,
        scale_factor: f32,
        cx: &mut Context<Self>,
    ) {
        if self.web_surfaces.record_scroll_delta(
            &tab_id,
            requested_url.as_str(),
            delta,
            position,
            scale_factor,
        ) == WebSurfaceInputOutcome::Applied
        {
            self.flush_external_web_surface_tick(cx);
        }
    }

    pub(super) fn hover_external_web_viewport(
        &mut self,
        tab_id: TabId,
        position: Point<Pixels>,
        scale_factor: f32,
        cx: &mut Context<Self>,
    ) {
        if self.web_surfaces.record_hover_point(&tab_id, position, scale_factor)
            == WebSurfaceInputOutcome::Applied
        {
            self.flush_external_web_surface_tick(cx);
        }
    }

    pub(super) fn click_external_web_viewport(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        position: Point<Pixels>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        let scale_factor = window.scale_factor();
        if self.web_surfaces.record_click_point(
            &tab_id,
            requested_url.as_str(),
            position,
            scale_factor,
        ) == WebSurfaceInputOutcome::Applied
        {
            self.flush_external_web_surface_tick(cx);
        }
    }

    /// Hand focus to the shell's root focus handle so subsequent
    /// keystrokes route to the web surface. Called on the very first
    /// mouse-down inside an external page so the user can start
    /// typing without waiting for the click to fully resolve.
    pub(super) fn focus_web_surface(&self, window: &mut gpui::Window) {
        self.focus_handle.focus(window);
    }

    pub(super) fn type_text_in_external_web_viewport(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        text: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.web_surfaces.record_typed_text(&tab_id, requested_url.as_str(), text)
            == WebSurfaceInputOutcome::Applied
        {
            self.flush_external_web_surface_tick(cx);
            return true;
        }

        false
    }
}

impl ElyShell {
    fn ensure_visible_web_surfaces(&mut self, visible_tabs: Vec<VisibleWebSurfaceTab>) -> bool {
        let mut changed = false;
        for visible in visible_tabs {
            changed |= self.web_surfaces.ensure_surface(
                &visible.tab,
                visible.profile_data_mode,
                visible.permissions.as_ref(),
            );
        }
        changed
    }

    fn apply_web_surface_url_change(&mut self, change: WebSurfaceUrlChange) -> bool {
        let Ok(url) = UrlText::parse(change.loaded_url) else {
            return false;
        };
        let super::ShellState::Ready(core) = &mut self.state else {
            return false;
        };

        match change.kind {
            WebSurfaceUrlChangeKind::UserInitiated => {
                core.navigate_tab_to_loaded_url(&change.tab_id, url).is_ok_and(|changed| changed)
            }
            WebSurfaceUrlChangeKind::Observed => {
                core.replace_tab_loaded_url(&change.tab_id, url).is_ok_and(|changed| changed)
            }
        }
    }

    fn apply_web_surface_page_metadata(&mut self, metadata: WebSurfacePageMetadata) -> bool {
        let super::ShellState::Ready(core) = &mut self.state else {
            return false;
        };
        let mut changed = false;
        if let Some(title) = metadata.title
            && let Ok(true) = core.set_tab_title(&metadata.tab_id, title)
        {
            changed = true;
        }
        if let Some(favicon_key) = metadata.favicon_key
            && let Ok(true) = core.set_tab_favicon_key(&metadata.tab_id, favicon_key)
        {
            changed = true;
        }
        changed
    }
}

struct VisibleWebSurfaceTab {
    tab: BrowserTab,
    profile_data_mode: ProfileDataMode,
    permissions: Arc<[WebSurfaceSitePermission]>,
}

fn visible_web_surface_tabs(
    core: &BrowserCore,
    tabs: Vec<BrowserTab>,
) -> Vec<VisibleWebSurfaceTab> {
    let mut permission_cache = HashMap::new();
    let mut visible = Vec::new();
    for tab in tabs {
        if !super::web_surface::is_external_web_url(tab.url().as_str()) {
            continue;
        }
        let Ok(kind) = core.profile_kind_for(tab.profile_id()) else {
            continue;
        };
        let permissions = permission_cache
            .entry(tab.profile_id().clone())
            .or_insert_with(|| {
                Arc::<[WebSurfaceSitePermission]>::from(web_surface_site_permissions_for_core_tab(
                    core, &tab,
                ))
            })
            .clone();
        visible.push(VisibleWebSurfaceTab {
            tab,
            profile_data_mode: profile_data_mode_from_kind(kind),
            permissions,
        });
    }
    visible
}

fn profile_data_mode_for(tab: &BrowserTab, snapshot: &BrowserSnapshot) -> Option<ProfileDataMode> {
    snapshot
        .profiles
        .iter()
        .find(|profile| profile.id() == tab.profile_id())
        .map(|profile| profile_data_mode_from_kind(profile.kind().clone()))
}

fn profile_data_mode_from_kind(kind: ProfileKind) -> ProfileDataMode {
    match kind {
        ProfileKind::Standard => ProfileDataMode::Persistent,
        ProfileKind::Private => ProfileDataMode::Transient,
    }
}

fn apply_permission_consumption(
    core: &mut BrowserCore,
    consumed: &ServoLivePermissionGrant,
) -> bool {
    let transferred = core
        .transfer_site_permission_once(
            consumed.profile_id(),
            consumed.origin(),
            consumed.feature(),
            consumed.grant_revision(),
        )
        .unwrap_or(false);
    let finished = core
        .finish_site_permission_once(
            consumed.profile_id(),
            consumed.origin(),
            consumed.feature(),
            consumed.grant_revision(),
        )
        .unwrap_or(false);
    transferred || finished
}

#[cfg(test)]
mod tests {
    use ely_browser_core::{BrowserCore, InitialBrowserConfig};
    use ely_domain::{SiteOrigin, SitePermissionDecision, SitePermissionFeature};

    use super::{ServoLivePermissionGrant, apply_permission_consumption};

    #[test]
    fn consumption_receipt_finishes_a_pending_allow_once_grant()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let profile_id = core.snapshot()?.active_profile_id;
        let origin = SiteOrigin::parse("https://example.com")?;
        core.set_site_permission(
            origin.clone(),
            SitePermissionFeature::Camera,
            SitePermissionDecision::AllowOnce,
        )?;
        let revision =
            core.site_permission_revision(&profile_id, &origin, SitePermissionFeature::Camera);
        let consumed = ServoLivePermissionGrant::new(
            profile_id,
            origin,
            SitePermissionFeature::Camera,
            revision,
        );

        assert!(apply_permission_consumption(&mut core, &consumed));
        assert!(core.snapshot()?.site_permissions.is_empty());
        Ok(())
    }
}
