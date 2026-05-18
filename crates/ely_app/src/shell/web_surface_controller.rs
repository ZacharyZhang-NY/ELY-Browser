use ely_browser_core::{BrowserCore, BrowserSnapshot};
use ely_domain::{BrowserTab, ProfileKind, TabId, UrlText};
use gpui::{AnyElement, Bounds, Context, NativeSurfaceHandle, Pixels, Point};

use crate::services::ProfileDataMode;

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
        if profile_data_mode_for(tab, snapshot).is_none() {
            return render_failed_web_surface(tab, "Profile context is unavailable.", state_entity);
        }

        match self.web_surfaces.state(tab.id()) {
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
        let sync_changed = self.drain_sync_updates();
        if url_changed || metadata_changed {
            self.schedule_cloud_sync_upload(cx);
        }
        result.changed || url_changed || metadata_changed || sync_changed
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

    pub(super) fn record_external_web_surface(
        &mut self,
        tab_id: TabId,
        bounds: Bounds<Pixels>,
        scale_factor: f32,
        native_surface: NativeSurfaceHandle,
        cx: &mut Context<Self>,
    ) {
        let viewport_changed =
            self.web_surfaces.record_viewport_size(&tab_id, bounds, scale_factor)
                == WebSurfaceInputOutcome::Applied;
        let surface_changed = self.web_surfaces.record_native_surface(&tab_id, native_surface)
            == WebSurfaceInputOutcome::Applied;

        if viewport_changed || surface_changed {
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
                &visible.permissions,
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
    permissions: Vec<WebSurfaceSitePermission>,
}

fn visible_web_surface_tabs(
    core: &BrowserCore,
    tabs: Vec<BrowserTab>,
) -> Vec<VisibleWebSurfaceTab> {
    tabs.into_iter()
        .filter(|tab| super::web_surface::is_external_web_url(tab.url().as_str()))
        .filter_map(|tab| {
            let profile_data_mode =
                core.profile_kind_for(tab.profile_id()).ok().map(profile_data_mode_from_kind)?;
            let permissions = web_surface_site_permissions_for_core_tab(core, &tab);
            Some(VisibleWebSurfaceTab { tab, profile_data_mode, permissions })
        })
        .collect()
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
