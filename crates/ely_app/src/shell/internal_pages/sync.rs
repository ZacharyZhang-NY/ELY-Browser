use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{ProfileKind, SyncConnectionState, SyncObjectKind, SyncObjectStatus};
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, px, rgb, rgba,
};
use gpui_component::{input::Input, scroll::ScrollableElement};

use crate::shell::auth::AuthFlowPhase;

use super::sync_controls::{
    button_bg, render_dual_button_row, render_policy_toggle, render_primary_button,
    render_reset_button, render_sign_out_button,
};
use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_sync_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p(px(40.0))
                .flex()
                .justify_center()
                .child(render_sync_body(self, snapshot, cx)),
        )
    }
}

fn render_sync_body(
    shell: &mut ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let body = div().max_w(px(860.0)).flex().flex_col().gap(px(18.0)).child(
        div()
            .text_size(px(26.0))
            .font_weight(FontWeight(500.0))
            .text_color(rgb(colors::ink()))
            .child("Sync"),
    );
    if !profile_allows_sync_controls(&snapshot.active_profile_kind) {
        return body.child(render_private_profile_card()).into_any_element();
    }
    body.child(
        div()
            .grid()
            .grid_cols(2)
            .gap(px(18.0))
            .child(render_account_card(shell, snapshot, cx))
            .child(render_data_card(shell, snapshot, cx)),
    )
    .into_any_element()
}

fn profile_allows_sync_controls(profile_kind: &ProfileKind) -> bool {
    profile_kind == &ProfileKind::Standard
}

fn render_private_profile_card() -> AnyElement {
    div()
        .p(px(16.0))
        .rounded(px(12.0))
        .bg(rgba(card_bg()))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::ink()))
                .child("Private profile"),
        )
        .child(
            div().text_size(px(14.0)).text_color(rgb(colors::ink_3())).child("Local session only"),
        )
        .into_any_element()
}

fn render_account_card(
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let card =
        div().p(px(18.0)).rounded(px(12.0)).bg(rgba(card_bg())).flex().flex_col().gap(px(14.0));

    match snapshot.sync_status.connection() {
        SyncConnectionState::SignedOut => card
            .child(render_card_heading("Account"))
            .children(account_form(shell, cx))
            .into_any_element(),
        SyncConnectionState::SignedIn
        | SyncConnectionState::AwaitingDeviceApproval
        | SyncConnectionState::SyncReady { .. }
        | SyncConnectionState::SyncError { .. } => card
            .child(render_card_heading("Account"))
            .child(render_sign_out_button(shell, cx))
            .into_any_element(),
    }
}

fn account_form(shell: &ElyShell, cx: &mut Context<ElyShell>) -> Vec<AnyElement> {
    let mut elements: Vec<AnyElement> = Vec::new();
    let phase = shell.auth_flow_phase.clone();

    elements.push(render_field_label("Email"));
    elements.push(render_input(&shell.auth_email_input));

    match &phase {
        AuthFlowPhase::Idle | AuthFlowPhase::Error { .. } => {
            elements.push(render_primary_button(
                shell,
                "send-otp",
                "Send code",
                false,
                cx,
                |shell, cx| {
                    shell.submit_email_otp_request(cx);
                },
            ));
        }
        AuthFlowPhase::SendingCode { .. } => {
            elements.push(render_primary_button(shell, "send-otp", "Sending", true, cx, |_, _| {}));
        }
        AuthFlowPhase::AwaitingOtp { .. } | AuthFlowPhase::Verifying { .. } => {
            elements.push(render_field_label("Code"));
            elements.push(render_input(&shell.auth_otp_input));
            elements.push(render_dual_button_row(
                shell,
                phase.is_busy(),
                cx,
                |shell, cx| shell.submit_email_otp_verify(cx),
                |shell, cx| shell.submit_email_otp_request(cx),
            ));
        }
    }

    if let Some(message) = phase.error_message() {
        elements.push(render_inline_error(message));
    }

    elements
}

fn render_data_card(
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .p(px(18.0))
        .rounded(px(12.0))
        .bg(rgba(card_bg()))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .max_h(px(640.0))
        .overflow_y_scrollbar()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(10.0))
                .child(render_card_heading("Data"))
                .child(render_reset_button(shell, cx)),
        )
        .child(
            div().flex().flex_col().gap(px(2.0)).children(
                snapshot
                    .sync_status
                    .objects()
                    .iter()
                    .enumerate()
                    .map(|(index, status)| render_sync_object_row(shell, index, status, cx)),
            ),
        )
        .into_any_element()
}

fn render_sync_object_row(
    shell: &ElyShell,
    index: usize,
    status: &SyncObjectStatus,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .py(px(9.0))
        .border_b_1()
        .border_color(rgba(colors::divider()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::ink()))
                .child(sync_object_kind_label(status.kind())),
        )
        .child(render_policy_toggle(shell, index, status, cx))
        .into_any_element()
}

fn render_card_heading(label: &'static str) -> AnyElement {
    div()
        .text_size(px(13.0))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(colors::ink()))
        .child(label)
        .into_any_element()
}

fn render_field_label(label: &'static str) -> AnyElement {
    div()
        .text_size(px(10.5))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(colors::ink_4()))
        .child(label)
        .into_any_element()
}

fn render_input(state: &gpui::Entity<gpui_component::input::InputState>) -> AnyElement {
    div()
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(8.0))
        .bg(rgba(button_bg()))
        .child(Input::new(state).appearance(false).cleanable(false))
        .into_any_element()
}

fn render_inline_error(message: &str) -> AnyElement {
    div()
        .text_size(px(11.5))
        .text_color(rgb(colors::error()))
        .child(message.to_string())
        .into_any_element()
}

fn sync_object_kind_label(kind: SyncObjectKind) -> &'static str {
    match kind {
        SyncObjectKind::Spaces => "Spaces",
        SyncObjectKind::Tabs => "Tabs & pinned",
        SyncObjectKind::Bookmarks => "Bookmarks",
        SyncObjectKind::Notes => "Notes",
        SyncObjectKind::ReadingList => "Reading list",
        SyncObjectKind::Profiles => "Profiles",
        SyncObjectKind::SitePermissions => "Site permissions",
        SyncObjectKind::History => "History",
        SyncObjectKind::PluginSettings => "Plugins",
    }
}

fn card_bg() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}

#[cfg(test)]
mod tests {
    use ely_domain::ProfileKind;

    use super::profile_allows_sync_controls;

    #[test]
    fn private_profile_hides_sync_controls() {
        assert!(!profile_allows_sync_controls(&ProfileKind::Private));
        assert!(profile_allows_sync_controls(&ProfileKind::Standard));
    }
}
