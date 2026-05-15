use ely_browser_core::BrowserSnapshot;
use ely_domain::{BrowserTab, ProfileKind, TabId, UrlText};
use gpui::{AnyElement, Bounds, Context, Pixels, Point};

use crate::services::ProfileDataMode;

use super::{
    ElyShell,
    web_surface::WebSurfacePageMetadata,
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
        if profile_data_mode_for(tab, snapshot).is_none() {
            return render_failed_web_surface(tab, "Profile context is unavailable.", state_entity);
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
        let (visible_tab_ids, open_tab_ids, snapshot) = match &self.state {
            super::ShellState::Ready(core) => (
                core.visible_content_tab_ids().unwrap_or_else(|_| Vec::new()),
                core.open_tab_ids(),
                core.snapshot().ok(),
            ),
            super::ShellState::StartupError(_) => (Vec::new(), Vec::new(), None),
        };
        self.web_surfaces.retain_tabs(&open_tab_ids);
        let mut url_changed = snapshot
            .as_ref()
            .map(|snapshot| self.ensure_visible_web_surfaces(snapshot, &visible_tab_ids))
            .unwrap_or(false);
        let result = self.web_surfaces.tick(&visible_tab_ids);
        for url_change in result.url_changes {
            url_changed |= self.apply_web_surface_url_change(url_change);
        }
        let mut metadata_changed = false;
        for metadata in result.page_metadata {
            metadata_changed |= self.apply_web_surface_page_metadata(metadata);
        }
        let sync_changed = self.drain_sync_updates();
        result.changed || url_changed || metadata_changed || sync_changed
    }

    pub(super) fn record_external_web_viewport(
        &mut self,
        tab_id: TabId,
        bounds: Bounds<Pixels>,
        scale_factor: f32,
        _cx: &mut Context<Self>,
    ) {
        let _ = self.web_surfaces.record_viewport_size(&tab_id, bounds, scale_factor);
    }

    pub(super) fn scroll_external_web_viewport(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        delta: Point<Pixels>,
        position: Point<Pixels>,
        scale_factor: f32,
        _cx: &mut Context<Self>,
    ) {
        let _ = self.web_surfaces.record_scroll_delta(
            &tab_id,
            requested_url.as_str(),
            delta,
            position,
            scale_factor,
        );
    }

    pub(super) fn hover_external_web_viewport(
        &mut self,
        tab_id: TabId,
        position: Point<Pixels>,
        scale_factor: f32,
        _cx: &mut Context<Self>,
    ) {
        let _ = self.web_surfaces.record_hover_point(&tab_id, position, scale_factor);
    }

    pub(super) fn click_external_web_viewport(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        position: Point<Pixels>,
        window: &mut gpui::Window,
        _cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        let scale_factor = window.scale_factor();
        let _ = self.web_surfaces.record_click_point(
            &tab_id,
            requested_url.as_str(),
            position,
            scale_factor,
        );
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
        _cx: &mut Context<Self>,
    ) -> bool {
        if self.web_surfaces.record_typed_text(&tab_id, requested_url.as_str(), text)
            == WebSurfaceInputOutcome::Applied
        {
            return true;
        }

        false
    }
}

impl ElyShell {
    fn ensure_visible_web_surfaces(
        &mut self,
        snapshot: &BrowserSnapshot,
        visible_tab_ids: &[TabId],
    ) -> bool {
        let mut url_changed = false;
        let mut changed = false;
        let visible_tabs = visible_external_web_tabs(&snapshot.tabs, visible_tab_ids);
        for tab in visible_tabs {
            let Some(profile_data_mode) = profile_data_mode_for(tab, snapshot) else {
                continue;
            };
            let permissions = web_surface_site_permissions_for_tab(tab, snapshot);
            let outcome = self.web_surfaces.ensure_surface(tab, profile_data_mode, &permissions);
            changed |= outcome.changed;
            if let Some(url_change) = outcome.url_change {
                url_changed |= self.apply_web_surface_url_change(url_change);
            }
        }
        changed || url_changed
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
        if let Some(favicon_url) = metadata.favicon_url
            && let Ok(true) = core.set_tab_favicon_key(&metadata.tab_id, favicon_url)
        {
            changed = true;
        }
        changed
    }
}

fn visible_external_web_tabs<'a>(
    tabs: &'a [BrowserTab],
    visible_tab_ids: &[TabId],
) -> Vec<&'a BrowserTab> {
    visible_tab_ids
        .iter()
        .filter_map(|tab_id| tabs.iter().find(|tab| tab.id() == tab_id))
        .filter(|tab| super::web_surface::is_external_web_url(tab.url().as_str()))
        .collect()
}

fn profile_data_mode_for(tab: &BrowserTab, snapshot: &BrowserSnapshot) -> Option<ProfileDataMode> {
    snapshot.profiles.iter().find(|profile| profile.id() == tab.profile_id()).map(|profile| {
        match profile.kind() {
            ProfileKind::Standard => ProfileDataMode::Persistent,
            ProfileKind::Private => ProfileDataMode::Transient,
        }
    })
}

#[cfg(test)]
mod tests {
    use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};

    use super::visible_external_web_tabs;

    #[test]
    fn visible_external_web_tabs_follow_visible_order() -> Result<(), String> {
        let first_id = TabId::new();
        let second_id = TabId::new();
        let internal_id = TabId::new();
        let tabs = vec![
            web_tab(first_id.clone(), "https://example.com/first")?,
            web_tab(internal_id.clone(), "ely://settings")?,
            web_tab(second_id.clone(), "http://example.com/second")?,
        ];

        let visible =
            visible_external_web_tabs(&tabs, &[internal_id, second_id.clone(), first_id.clone()])
                .into_iter()
                .map(|tab| tab.id().clone())
                .collect::<Vec<_>>();

        assert_eq!(visible, vec![second_id, first_id]);
        Ok(())
    }

    fn web_tab(tab_id: TabId, url: &str) -> Result<BrowserTab, String> {
        let url = UrlText::parse(url).map_err(|error| error.to_string())?;
        Ok(BrowserTab::new(tab_id, SpaceId::new(), ProfileId::new(), "Web", url))
    }
}
