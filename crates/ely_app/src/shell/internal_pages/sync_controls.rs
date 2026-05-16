use ely_design_system::colors;
use ely_domain::{SyncObjectPolicy, SyncObjectStatus};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, rgb, rgba,
};

use crate::shell::ElyShell;
use crate::shell::chrome::animations::{chrome_motion_feedback, toggle_thumb_motion};

pub(super) fn render_primary_button<F>(
    shell: &ElyShell,
    id: &'static str,
    label: &'static str,
    disabled: bool,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut Context<ElyShell>) + 'static,
{
    let press_id = if disabled { None } else { shell.chrome_motion_animation_id(id) };
    let element = div()
        .id(SharedString::from(id))
        .px(px(14.0))
        .py(px(8.0))
        .rounded(px(8.0))
        .bg(rgba(colors::accent()))
        .text_size(px(12.5))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(0xfff5e6))
        .when(!disabled, |el| {
            el.cursor_pointer()
                .hover(|style| style.opacity(0.92))
                .active(|style| style.opacity(0.78))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.trigger_chrome_motion(id);
                    handler(shell, cx);
                }))
        })
        .when(disabled, |el| el.opacity(0.6))
        .child(label);

    chrome_motion_feedback(press_id, SharedString::from(format!("{id}-selection")), false, element)
}

pub(super) fn render_dual_button_row<P, S>(
    shell: &ElyShell,
    disabled: bool,
    cx: &mut Context<ElyShell>,
    primary: P,
    secondary: S,
) -> AnyElement
where
    P: Fn(&mut ElyShell, &mut Context<ElyShell>) + 'static,
    S: Fn(&mut ElyShell, &mut Context<ElyShell>) + 'static,
{
    div()
        .flex()
        .gap(px(8.0))
        .child(render_primary_button(
            shell,
            "verify-otp",
            if disabled { "Verifying..." } else { "Verify" },
            disabled,
            cx,
            primary,
        ))
        .child(render_secondary_button(shell, "resend-otp", "Resend code", disabled, cx, secondary))
        .into_any_element()
}

pub(super) fn render_sign_out_button(shell: &ElyShell, cx: &mut Context<ElyShell>) -> AnyElement {
    render_secondary_button(shell, "sign-out", "Sign out", false, cx, |shell, cx| {
        shell.submit_sign_out(cx);
    })
}

pub(super) fn render_reset_button(shell: &ElyShell, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .gap(px(8.0))
        .child(render_primary_button(shell, "sync-upload", "Sync now", false, cx, |shell, _| {
            shell.trigger_cloud_sync_upload();
        }))
        .child(render_secondary_button(
            shell,
            "sync-reset",
            "Reset to defaults",
            false,
            cx,
            |shell, cx| shell.reset_sync_settings(cx),
        ))
        .into_any_element()
}

pub(super) fn render_policy_toggle(
    shell: &ElyShell,
    index: usize,
    status: &SyncObjectStatus,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let enabled = status.policy() == SyncObjectPolicy::Enabled;
    let next_policy = if enabled { SyncObjectPolicy::Paused } else { SyncObjectPolicy::Enabled };
    let kind = status.kind();
    let track_color = if enabled { colors::accent() } else { 0x281e1426 };
    let id = SharedString::from(format!("sync-policy-{index}"));
    let press_id = shell.chrome_motion_animation_id(id.as_str());
    let thumb_press_id = press_id.clone();
    let selection_id = SharedString::from(format!("{}-selection", id.as_str()));

    let element = div()
        .id(id.clone())
        .w(px(34.0))
        .h(px(20.0))
        .rounded_full()
        .bg(rgba(track_color))
        .p(px(2.0))
        .cursor_pointer()
        .hover(|style| style.opacity(0.9))
        .active(|style| style.opacity(0.78))
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.trigger_chrome_motion(id.clone());
            shell.set_sync_object_policy(kind, next_policy, cx);
        }))
        .child(toggle_thumb_motion(
            thumb_press_id,
            enabled,
            14.0,
            div().size(px(16.0)).rounded_full().bg(rgb(0xffffff)),
        ));

    chrome_motion_feedback(press_id, selection_id, enabled, element)
}

fn render_secondary_button<F>(
    shell: &ElyShell,
    id: &'static str,
    label: &'static str,
    disabled: bool,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut Context<ElyShell>) + 'static,
{
    let press_id = if disabled { None } else { shell.chrome_motion_animation_id(id) };
    let element = div()
        .id(SharedString::from(id))
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(8.0))
        .bg(rgba(button_bg()))
        .text_size(px(12.0))
        .text_color(rgb(colors::ink_2()))
        .when(!disabled, |el| {
            el.cursor_pointer()
                .hover(|style| style.bg(rgba(button_bg_hover())))
                .active(|style| style.opacity(0.85))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.trigger_chrome_motion(id);
                    handler(shell, cx);
                }))
        })
        .when(disabled, |el| el.opacity(0.6))
        .child(label);

    chrome_motion_feedback(press_id, SharedString::from(format!("{id}-selection")), false, element)
}

pub(super) fn button_bg() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}

fn button_bg_hover() -> u32 {
    colors::pick(0xffffffeb, 0x1f1d1beb)
}
