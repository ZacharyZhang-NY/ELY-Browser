use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{SiteOrigin, SitePermissionDecision, SitePermissionEntry};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_site_permissions_settings_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_site_permissions_header(snapshot))
                .child(render_site_permissions_summary(snapshot))
                .child(render_site_permissions_controls(
                    snapshot,
                    self.site_permissions_clear_confirmation.as_ref()
                        == Some(&snapshot.active_profile_id),
                    cx,
                ))
                .child(render_site_permissions_list(snapshot, cx)),
        )
    }
}

fn render_site_permissions_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(px(26.0))
                        .text_color(rgb(colors::INK))
                        .child("Site Permissions"),
                )
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("Profile: {}", snapshot.active_profile_name)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .font_semibold()
                .text_color(rgb(colors::MUTED))
                .child(IconName::Globe)
                .child("Profile scoped"),
        )
        .into_any_element()
}

fn render_site_permissions_summary(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .border_t_1()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .children([
            site_permission_metric("Configured", snapshot.site_permissions.len()),
            site_permission_metric("Allowed", allowed_count(snapshot)),
            site_permission_metric("Denied", denied_count(snapshot)),
            site_permission_metric("Audit Events", snapshot.site_permission_audit_events.len()),
        ])
        .into_any_element()
}

fn site_permission_metric(label: &'static str, value: usize) -> AnyElement {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(colors::MUTED)).child(label))
        .child(
            div().text_sm().font_semibold().text_color(rgb(colors::INK)).child(value.to_string()),
        )
        .into_any_element()
}

fn render_site_permissions_controls(
    snapshot: &BrowserSnapshot,
    confirming_clear: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if snapshot.site_permissions.is_empty() {
        return div().into_any_element();
    }

    if confirming_clear {
        return render_clear_confirmation(cx);
    }

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .text_sm()
                .text_color(rgb(colors::MUTED))
                .child("Clear all configured permissions for this Profile."),
        )
        .child(
            Button::new("request-clear-site-permissions")
                .danger()
                .xsmall()
                .label("Clear Permissions")
                .tooltip("Clear Profile Permissions")
                .on_click(cx.listener(|shell, _, _, cx| {
                    shell.request_clear_site_permissions_confirmation(cx);
                })),
        )
        .into_any_element()
}

fn render_clear_confirmation(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::ERROR))
        .bg(rgb(colors::CANVAS_SOFT))
        .px_4()
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(rgb(colors::INK))
                        .child("Confirm permission clearing"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors::MUTED))
                        .child("Each cleared permission records a local audit event."),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("cancel-clear-site-permissions")
                        .ghost()
                        .xsmall()
                        .label("Cancel")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.cancel_clear_site_permissions_confirmation(cx);
                        })),
                )
                .child(
                    Button::new("confirm-clear-site-permissions")
                        .danger()
                        .xsmall()
                        .label("Clear")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.clear_active_profile_site_permissions(cx);
                        })),
                ),
        )
        .into_any_element()
}

fn render_site_permissions_list(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if snapshot.site_permissions.is_empty() {
        return div()
            .flex_1()
            .border_t_1()
            .border_color(rgb(colors::HAIRLINE))
            .pt_5()
            .text_sm()
            .text_color(rgb(colors::MUTED))
            .child("No site permissions are configured for this Profile.")
            .into_any_element();
    }

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(
            snapshot
                .site_permissions
                .iter()
                .enumerate()
                .map(|(index, entry)| render_site_permission_entry(index, entry, cx)),
        )
        .into_any_element()
}

fn render_site_permission_entry(
    index: usize,
    entry: &SitePermissionEntry,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let origin = entry.origin().clone();
    let route = site_settings_route(&origin);
    let decision = entry.decision();

    div()
        .py_3()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_color(rgb(permission_decision_color(decision)))
                        .child(permission_decision_icon(decision)),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .truncate()
                                .text_color(rgb(colors::INK))
                                .child(entry.origin().as_str().to_string()),
                        )
                        .child(div().text_xs().truncate().text_color(rgb(colors::MUTED)).child(
                            format!("{} - {}", entry.feature().label(), entry.decision().label()),
                        )),
                ),
        )
        .child(
            Button::new(("open-site-permission-settings", index))
                .ghost()
                .xsmall()
                .label("Open")
                .tooltip("Open Site Settings")
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.open_internal_tab(&route, window, cx);
                })),
        )
        .into_any_element()
}

fn allowed_count(snapshot: &BrowserSnapshot) -> usize {
    snapshot
        .site_permissions
        .iter()
        .filter(|entry| {
            matches!(
                entry.decision(),
                SitePermissionDecision::AllowOnce | SitePermissionDecision::AllowAlways
            )
        })
        .count()
}

fn denied_count(snapshot: &BrowserSnapshot) -> usize {
    snapshot
        .site_permissions
        .iter()
        .filter(|entry| entry.decision() == SitePermissionDecision::DenyAlways)
        .count()
}

fn site_settings_route(origin: &SiteOrigin) -> String {
    format!("ely://site/{}", origin.as_str())
}

fn permission_decision_color(decision: SitePermissionDecision) -> u32 {
    match decision {
        SitePermissionDecision::AllowOnce | SitePermissionDecision::AllowAlways => colors::SUCCESS,
        SitePermissionDecision::DenyAlways => colors::ERROR,
    }
}

fn permission_decision_icon(decision: SitePermissionDecision) -> IconName {
    match decision {
        SitePermissionDecision::AllowOnce | SitePermissionDecision::AllowAlways => {
            IconName::CircleCheck
        }
        SitePermissionDecision::DenyAlways => IconName::CircleX,
    }
}
