use ely_browser_core::BrowserSnapshot;
use ely_domain::{BrowserTab, ProfileKind, TabId};
use gpui::{AnyElement, Bounds, Context, Pixels, Point};

use crate::services::{
    ProfileDataMode,
    servo_sidecar::{ServoSidecarError, SidecarSitePermission, SidecarSnapshot},
};

use super::{
    ElyShell,
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{WebSurfaceClickPoint, WebSurfaceScrollOffset, WebSurfaceSize},
    web_surface_permissions::sidecar_site_permissions_for_tab,
    web_surface_state::{WebSurfaceRequest, WebSurfaceState},
    web_surface_view::{
        render_failed_web_surface, render_loading_web_surface, render_ready_web_surface,
    },
};

struct PendingWebSurfaceFrame {
    tab_id: TabId,
    requested_url: String,
    size: WebSurfaceSize,
    scroll_offset: WebSurfaceScrollOffset,
    click_point: Option<WebSurfaceClickPoint>,
    typed_text: Option<String>,
}

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
        let site_permissions = sidecar_site_permissions_for_tab(tab, snapshot);

        self.ensure_external_web_frame(tab, profile_data_mode, site_permissions, cx);

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

    fn ensure_external_web_frame(
        &mut self,
        tab: &BrowserTab,
        profile_data_mode: ProfileDataMode,
        site_permissions: Vec<SidecarSitePermission>,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self.web_surfaces.prepare_request(tab, profile_data_mode) else {
            return;
        };

        let WebSurfaceRequest {
            tab_id,
            requested_url,
            size,
            scroll_offset,
            click_point,
            typed_text,
            client,
            mut snapshot_request,
        } = request;
        snapshot_request = snapshot_request.with_site_permissions(site_permissions);
        let pending_frame = PendingWebSurfaceFrame {
            tab_id,
            requested_url,
            size,
            scroll_offset,
            click_point,
            typed_text,
        };
        cx.spawn(async move |shell, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { client.snapshot(snapshot_request) })
                .await;

            _ = shell.update(cx, |shell, cx| {
                shell.handle_external_web_frame_result(pending_frame, result);
                cx.notify();
            });
        })
        .detach();
    }

    fn handle_external_web_frame_result(
        &mut self,
        pending_frame: PendingWebSurfaceFrame,
        result: Result<SidecarSnapshot, ServoSidecarError>,
    ) {
        let PendingWebSurfaceFrame {
            tab_id,
            requested_url,
            size,
            scroll_offset,
            click_point,
            typed_text,
        } = pending_frame;
        if !self.web_surfaces.is_loading(
            &tab_id,
            requested_url.as_str(),
            size,
            scroll_offset,
            click_point,
            typed_text.as_deref(),
        ) {
            return;
        }

        let state = match result {
            Ok(snapshot) => match WebSurfaceFrame::from_snapshot(
                requested_url.clone(),
                scroll_offset,
                click_point,
                typed_text.clone(),
                snapshot,
            ) {
                Ok(frame) => WebSurfaceState::Ready(frame),
                Err(error) => WebSurfaceState::Failed {
                    requested_url,
                    size,
                    scroll_offset,
                    click_point,
                    typed_text,
                    message: error.to_string(),
                },
            },
            Err(error) => WebSurfaceState::Failed {
                requested_url,
                size,
                scroll_offset,
                click_point,
                typed_text,
                message: error.to_string(),
            },
        };
        self.web_surfaces.finish(tab_id, state);
    }

    pub(super) fn record_external_web_viewport(
        &mut self,
        tab_id: TabId,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.web_surfaces.record_viewport_size(&tab_id, bounds) {
            cx.notify();
        }
    }

    pub(super) fn scroll_external_web_viewport(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        delta: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.web_surfaces.record_scroll_delta(&tab_id, requested_url.as_str(), delta) {
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
        if self.web_surfaces.record_click_point(&tab_id, requested_url.as_str(), position) {
            cx.notify();
        }
    }

    pub(super) fn type_text_in_external_web_viewport(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        text: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.web_surfaces.record_typed_text(&tab_id, requested_url.as_str(), text) {
            cx.notify();
            return true;
        }

        false
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
