use ely_browser_core::BrowserSnapshot;
use ely_domain::{BrowserTab, ProfileKind, TabId, UrlText};
use gpui::{AnyElement, Bounds, Context, Pixels, Point};

use crate::services::ProfileDataMode;

use super::{
    ElyShell,
    web_surface_permissions::web_surface_site_permissions_for_tab,
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = cx.entity().clone();
        let Some(profile_data_mode) = profile_data_mode_for(tab, snapshot) else {
            return render_failed_web_surface(tab, "Profile context is unavailable.", state_entity);
        };

        let permissions = web_surface_site_permissions_for_tab(tab, snapshot);
        if let Some(url_change) =
            self.web_surfaces.ensure_surface(tab, profile_data_mode, &permissions)
            && self.apply_web_surface_url_change(url_change)
        {
            cx.notify();
        }

        match self.web_surfaces.state(tab.id()) {
            Some(WebSurfaceState::Ready(frame)) => {
                render_ready_web_surface(frame, tab, state_entity)
            }
            Some(WebSurfaceState::Failed { message, .. }) => {
                render_failed_web_surface(tab, message.as_str(), state_entity)
            }
            Some(WebSurfaceState::Loading { previous_frame: Some(frame), .. }) => {
                render_ready_web_surface(frame, tab, state_entity)
            }
            Some(WebSurfaceState::Loading { previous_frame: None, .. }) | None => {
                render_loading_web_surface(tab, state_entity)
            }
        }
    }

    pub(super) fn tick_external_web_surfaces(&mut self) -> bool {
        let result = self.web_surfaces.tick();
        let mut url_changed = false;
        for url_change in result.url_changes {
            url_changed |= self.apply_web_surface_url_change(url_change);
        }
        result.changed || url_changed
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
            cx.notify();
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
            cx.notify();
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
            cx.notify();
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
            cx.notify();
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
            cx.notify();
            return true;
        }

        false
    }
}

impl ElyShell {
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
}

fn profile_data_mode_for(tab: &BrowserTab, snapshot: &BrowserSnapshot) -> Option<ProfileDataMode> {
    snapshot.profiles.iter().find(|profile| profile.id() == tab.profile_id()).map(|profile| {
        match profile.kind() {
            ProfileKind::Standard => ProfileDataMode::Persistent,
            ProfileKind::Private => ProfileDataMode::Transient,
        }
    })
}
