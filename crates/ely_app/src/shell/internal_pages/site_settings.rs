use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{
    SiteOrigin, SitePermissionAuditAction, SitePermissionAuditEvent, SitePermissionDecision,
    SitePermissionFeature,
};
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_site_settings_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        route: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(origin) = SiteOrigin::from_site_route(route).ok().flatten() else {
            return render_canvas_surface(
                div()
                    .size_full()
                    .p_8()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .child(render_invalid_site_route()),
            );
        };

        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_site_settings_header(snapshot, &origin))
                .child(render_site_permission_summary(snapshot, &origin))
                .child(render_site_permission_rows(snapshot, &origin, cx))
                .child(render_site_permission_audit(snapshot, &origin)),
        )
    }
}

fn render_site_settings_header(snapshot: &BrowserSnapshot, origin: &SiteOrigin) -> AnyElement {
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
                    div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Site Settings"),
                )
                .child(div().text_sm().truncate().text_color(rgb(colors::MUTED)).child(format!(
                    "{} / {}",
                    snapshot.active_profile_name,
                    origin.as_str()
                ))),
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

fn render_site_permission_summary(snapshot: &BrowserSnapshot, origin: &SiteOrigin) -> AnyElement {
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
            site_metric("Configured", configured_count(snapshot, origin)),
            site_metric("Allowed", allowed_count(snapshot, origin)),
            site_metric("Denied", denied_count(snapshot, origin)),
            site_metric("Audit Events", audit_count(snapshot, origin)),
        ])
        .into_any_element()
}

fn site_metric(label: &'static str, value: usize) -> AnyElement {
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

fn render_site_permission_rows(
    snapshot: &BrowserSnapshot,
    origin: &SiteOrigin,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_xs().font_semibold().text_color(rgb(colors::MUTED)).child("Permissions"))
        .child(div().flex_1().min_h_0().flex().flex_col().overflow_y_scrollbar().children(
            SitePermissionFeature::all().iter().copied().enumerate().map(|(index, feature)| {
                render_site_permission_row(snapshot, origin, index, feature, cx)
            }),
        ))
        .into_any_element()
}

fn render_site_permission_row(
    snapshot: &BrowserSnapshot,
    origin: &SiteOrigin,
    index: usize,
    feature: SitePermissionFeature,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let decision = decision_for(snapshot, origin, feature);
    let status = decision.map_or("Ask", |decision| decision.label());
    let status_color = decision.map_or(colors::MUTED, decision_color);
    let button_base = index * 4;

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
                .child(div().text_color(rgb(status_color)).child(permission_icon(decision)))
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
                                .child(feature.label()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(colors::MUTED))
                                .child(feature_scope_label(feature)),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    div()
                        .min_w(px(78.0))
                        .text_xs()
                        .font_semibold()
                        .text_color(rgb(status_color))
                        .child(status),
                )
                .child(permission_decision_button(
                    button_base,
                    origin.clone(),
                    feature,
                    SitePermissionDecision::AllowOnce,
                    "Once",
                    decision,
                    cx,
                ))
                .child(permission_decision_button(
                    button_base + 1,
                    origin.clone(),
                    feature,
                    SitePermissionDecision::AllowAlways,
                    "Allow",
                    decision,
                    cx,
                ))
                .child(permission_decision_button(
                    button_base + 2,
                    origin.clone(),
                    feature,
                    SitePermissionDecision::DenyAlways,
                    "Deny",
                    decision,
                    cx,
                ))
                .when(decision.is_some(), |this| {
                    this.child(permission_reset_button(
                        button_base + 3,
                        origin.clone(),
                        feature,
                        cx,
                    ))
                }),
        )
        .into_any_element()
}

fn permission_decision_button(
    id: usize,
    origin: SiteOrigin,
    feature: SitePermissionFeature,
    target: SitePermissionDecision,
    label: &'static str,
    current: Option<SitePermissionDecision>,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let button = Button::new(("site-permission-decision", id))
        .xsmall()
        .label(label)
        .tooltip(target.label())
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_site_permission(origin.clone(), feature, target, cx);
        }));

    match (target, current == Some(target)) {
        (SitePermissionDecision::DenyAlways, true) => button.danger().into_any_element(),
        (_, true) => button.primary().into_any_element(),
        _ => button.ghost().into_any_element(),
    }
}

fn permission_reset_button(
    id: usize,
    origin: SiteOrigin,
    feature: SitePermissionFeature,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    Button::new(("site-permission-reset", id))
        .ghost()
        .xsmall()
        .icon(IconName::Undo2)
        .tooltip("Reset Permission")
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.revoke_site_permission(origin.clone(), feature, cx);
        }))
        .into_any_element()
}

fn render_site_permission_audit(snapshot: &BrowserSnapshot, origin: &SiteOrigin) -> AnyElement {
    let events = snapshot
        .site_permission_audit_events
        .iter()
        .filter(|event| event.origin() == origin)
        .rev()
        .take(4)
        .collect::<Vec<_>>();

    if events.is_empty() {
        return div().into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_xs().font_semibold().text_color(rgb(colors::MUTED)).child("Audit"))
        .children(events.into_iter().map(render_audit_row))
        .into_any_element()
}

fn render_audit_row(event: &SitePermissionAuditEvent) -> AnyElement {
    div()
        .py_2()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_xs()
        .child(div().min_w_0().truncate().text_color(rgb(colors::BODY)).child(format!(
            "{} - {}",
            event.feature().label(),
            audit_action_label(event.action())
        )))
        .child(div().text_color(rgb(colors::MUTED)).child("Local audit"))
        .into_any_element()
}

fn render_invalid_site_route() -> AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Site Settings"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(colors::MUTED))
                        .child("Site settings require an http or https origin."),
                ),
        )
        .into_any_element()
}

fn decision_for(
    snapshot: &BrowserSnapshot,
    origin: &SiteOrigin,
    feature: SitePermissionFeature,
) -> Option<SitePermissionDecision> {
    snapshot
        .site_permissions
        .iter()
        .find(|entry| entry.origin() == origin && entry.feature() == feature)
        .map(|entry| entry.decision())
}

fn configured_count(snapshot: &BrowserSnapshot, origin: &SiteOrigin) -> usize {
    snapshot.site_permissions.iter().filter(|entry| entry.origin() == origin).count()
}

fn allowed_count(snapshot: &BrowserSnapshot, origin: &SiteOrigin) -> usize {
    snapshot
        .site_permissions
        .iter()
        .filter(|entry| entry.origin() == origin)
        .filter(|entry| {
            matches!(
                entry.decision(),
                SitePermissionDecision::AllowOnce | SitePermissionDecision::AllowAlways
            )
        })
        .count()
}

fn denied_count(snapshot: &BrowserSnapshot, origin: &SiteOrigin) -> usize {
    snapshot
        .site_permissions
        .iter()
        .filter(|entry| {
            entry.origin() == origin && entry.decision() == SitePermissionDecision::DenyAlways
        })
        .count()
}

fn audit_count(snapshot: &BrowserSnapshot, origin: &SiteOrigin) -> usize {
    snapshot.site_permission_audit_events.iter().filter(|event| event.origin() == origin).count()
}

fn decision_color(decision: SitePermissionDecision) -> u32 {
    match decision {
        SitePermissionDecision::AllowOnce | SitePermissionDecision::AllowAlways => colors::SUCCESS,
        SitePermissionDecision::DenyAlways => colors::ERROR,
    }
}

fn permission_icon(decision: Option<SitePermissionDecision>) -> IconName {
    match decision {
        Some(SitePermissionDecision::AllowOnce | SitePermissionDecision::AllowAlways) => {
            IconName::CircleCheck
        }
        Some(SitePermissionDecision::DenyAlways) => IconName::CircleX,
        None => IconName::Info,
    }
}

fn audit_action_label(action: &SitePermissionAuditAction) -> &'static str {
    match action {
        SitePermissionAuditAction::Set(decision) => decision.label(),
        SitePermissionAuditAction::Revoked => "Reset",
    }
}

fn feature_scope_label(feature: SitePermissionFeature) -> &'static str {
    match feature {
        SitePermissionFeature::Camera => "Controls camera capture requests.",
        SitePermissionFeature::Microphone => "Controls microphone capture requests.",
        SitePermissionFeature::ScreenCapture => "Controls screen capture requests.",
        SitePermissionFeature::Location => "Controls location access requests.",
        SitePermissionFeature::Notifications => "Controls system notification prompts.",
        SitePermissionFeature::ClipboardRead => "Controls clipboard read requests.",
        SitePermissionFeature::ClipboardWrite => "Controls clipboard write requests.",
        SitePermissionFeature::Downloads => "Controls automatic download requests.",
        SitePermissionFeature::Popups => "Controls popup window requests.",
        SitePermissionFeature::Autoplay => "Controls autoplay behavior.",
        SitePermissionFeature::WebUsb => "Controls WebUSB device access.",
        SitePermissionFeature::WebHid => "Controls WebHID device access.",
        SitePermissionFeature::WebSerial => "Controls WebSerial device access.",
        SitePermissionFeature::StoragePersistence => "Controls persistent storage requests.",
        SitePermissionFeature::InsecureContent => "Controls insecure content loading.",
        SitePermissionFeature::CertificateException => "Controls certificate exceptions.",
    }
}
