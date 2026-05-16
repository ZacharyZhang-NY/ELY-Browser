use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{SyncConnectionState, SyncObjectKind, SyncObjectState, SyncObjectStatus};
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, px, rgb, rgba,
};
use gpui_component::{IconName, input::Input, scroll::ScrollableElement};

use crate::shell::auth::AuthFlowPhase;

use crate::brand::SYNC_SERVICE_NAME;

use super::sync_controls::{
    button_bg, render_dual_button_row, render_policy_toggle, render_primary_button,
    render_reset_button, render_sign_out_button,
};
use super::{ElyShell, render_canvas_surface};
use crate::shell::chrome::SERIF_FAMILY;

impl ElyShell {
    pub(super) fn render_sync_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(
            div().size_full().pt(px(40.0)).px(px(56.0)).pb(px(32.0)).flex().justify_center().child(
                div()
                    .max_w(px(960.0))
                    .grid()
                    .grid_cols(2)
                    .gap(px(32.0))
                    .child(render_left_column(self, snapshot, cx))
                    .child(render_right_column(self, snapshot, cx)),
            ),
        )
    }
}

fn render_left_column(
    shell: &mut ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(20.0))
        .child(render_status_pill(snapshot))
        .child(render_serif_headline())
        .child(render_intro_paragraph())
        .child(render_account_card(shell, snapshot, cx))
        .child(render_metrics_card(shell, snapshot, cx))
        .into_any_element()
}

fn render_status_pill(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(5.0))
        .rounded(px(999.0))
        .bg(rgba(pill_bg()))
        .text_size(px(11.0))
        .text_color(rgb(colors::ink_3()))
        .child(div().text_color(rgb(colors::accent())).child(IconName::Globe))
        .child(format!(
            "{SYNC_SERVICE_NAME} · {}",
            connection_label(snapshot.sync_status.connection())
        ))
        .into_any_element()
}

fn render_serif_headline() -> AnyElement {
    div()
        .font_family(SERIF_FAMILY)
        .text_size(px(46.0))
        .font_weight(FontWeight(400.0))
        .text_color(rgb(colors::ink()))
        .child("Your tabs, on every device.")
        .into_any_element()
}

fn render_intro_paragraph() -> AnyElement {
    div()
        .max_w(px(440.0))
        .text_size(px(14.0))
        .text_color(rgb(colors::ink_2()))
        .child(
            "ELY keeps tabs, workspaces, pinned items, and history mirrored across your \
             devices — encrypted in your hands and replayed at the edge.",
        )
        .into_any_element()
}

fn render_metrics_card(
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .max_w(px(380.0))
        .p(px(20.0))
        .rounded(px(16.0))
        .bg(rgba(card_bg()))
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(div().text_size(px(12.5)).text_color(rgb(colors::ink_3())).child("Local queue"))
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap(px(12.0))
                .child(render_metric(
                    "Pending",
                    snapshot.sync_status.pending_objects(),
                    colors::ink(),
                ))
                .child(render_metric(
                    "Failed",
                    snapshot.sync_status.failed_objects(),
                    colors::error(),
                )),
        )
        .child(render_reset_button(shell, cx))
        .into_any_element()
}

fn render_account_card(
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let card = div()
        .max_w(px(380.0))
        .p(px(20.0))
        .rounded(px(16.0))
        .bg(rgba(card_bg()))
        .flex()
        .flex_col()
        .gap(px(14.0));

    match snapshot.sync_status.connection() {
        SyncConnectionState::SignedOut => card
            .child(render_account_heading("Sign in"))
            .child(render_account_subtitle("We'll email a 6-digit code from browser@elydora.com."))
            .children(account_form(shell, cx))
            .into_any_element(),
        SyncConnectionState::SignedIn
        | SyncConnectionState::AwaitingDeviceApproval
        | SyncConnectionState::SyncReady { .. }
        | SyncConnectionState::SyncError { .. } => card
            .child(render_account_heading("Account"))
            .child(render_signed_in_chip())
            .child(render_sign_out_button(shell, cx))
            .into_any_element(),
    }
}

fn account_form(shell: &ElyShell, cx: &mut Context<ElyShell>) -> Vec<AnyElement> {
    let mut elements: Vec<AnyElement> = Vec::new();

    let phase = shell.auth_flow_phase.clone();
    let prefill_email = phase.email().map(str::to_string);

    elements.push(render_account_label("Email"));
    elements.push(render_input(&shell.auth_email_input, prefill_email.as_deref()));

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
            elements.push(render_primary_button(
                shell,
                "send-otp",
                "Sending...",
                true,
                cx,
                |_, _| {},
            ));
        }
        AuthFlowPhase::AwaitingOtp { .. } | AuthFlowPhase::Verifying { .. } => {
            elements.push(render_account_label("Code"));
            elements.push(render_input(&shell.auth_otp_input, None));
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

fn render_account_heading(label: &str) -> AnyElement {
    div()
        .text_size(px(13.0))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(colors::ink()))
        .child(label.to_string())
        .into_any_element()
}

fn render_account_subtitle(text: &str) -> AnyElement {
    div()
        .text_size(px(12.0))
        .text_color(rgb(colors::ink_3()))
        .child(text.to_string())
        .into_any_element()
}

fn render_account_label(label: &'static str) -> AnyElement {
    div()
        .text_size(px(10.5))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(colors::ink_4()))
        .child(label)
        .into_any_element()
}

fn render_input(
    state: &gpui::Entity<gpui_component::input::InputState>,
    hint: Option<&str>,
) -> AnyElement {
    let mut wrapper = div()
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(8.0))
        .bg(rgba(button_bg()))
        .child(Input::new(state).appearance(false).cleanable(false));
    if let Some(hint) = hint {
        wrapper = wrapper.child(
            div().text_size(px(10.0)).text_color(rgb(colors::ink_4())).child(hint.to_string()),
        );
    }
    wrapper.into_any_element()
}

fn render_inline_error(message: &str) -> AnyElement {
    div()
        .text_size(px(11.5))
        .text_color(rgb(colors::error()))
        .child(message.to_string())
        .into_any_element()
}

fn render_signed_in_chip() -> AnyElement {
    div()
        .text_size(px(13.0))
        .text_color(rgb(colors::ink_2()))
        .child("Signed in. New sessions on this device share the same encrypted snapshot.")
        .into_any_element()
}

fn render_metric(label: &'static str, value: usize, color: u32) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_size(px(10.5)).text_color(rgb(colors::ink_4())).child(label))
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(color))
                .child(value.to_string()),
        )
        .into_any_element()
}

fn render_right_column(
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(render_what_syncs_card(shell, snapshot, cx))
        .into_any_element()
}

fn render_what_syncs_card(
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .p(px(18.0))
        .rounded(px(16.0))
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
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::ink()))
                        .child("What syncs"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(colors::ink_3()))
                        .child(format!("{} kinds tracked", snapshot.sync_status.objects().len())),
                ),
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
        .gap(px(10.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(rgba(colors::divider()))
        .child(render_state_dot(status.state()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::ink()))
                        .child(sync_object_kind_label(status.kind())),
                )
                .child(div().text_size(px(11.0)).text_color(rgb(colors::ink_4())).child(format!(
                    "{} local · {}",
                    status.local_count(),
                    sync_object_state_label(status.state())
                ))),
        )
        .child(render_policy_toggle(shell, index, status, cx))
        .into_any_element()
}

fn render_state_dot(state: SyncObjectState) -> AnyElement {
    let color = match state {
        SyncObjectState::LocalOnly => colors::ink_4(),
        SyncObjectState::Paused => colors::ink_5(),
        SyncObjectState::PrivacyControlled => colors::accent(),
        SyncObjectState::Synced => colors::success(),
    };

    div().size(px(8.0)).rounded_full().bg(rgb(color)).into_any_element()
}

fn connection_label(connection: &SyncConnectionState) -> String {
    match connection {
        SyncConnectionState::SignedOut => "Local-only · drop a session token to enable".to_string(),
        SyncConnectionState::SignedIn => "Signed in · awaiting first sync".to_string(),
        SyncConnectionState::AwaitingDeviceApproval => {
            "Signed in · waiting for device approval".to_string()
        }
        SyncConnectionState::SyncReady { last_synced_at_secs } => {
            format!("Synced · last upload {}", relative_time_since(*last_synced_at_secs))
        }
        SyncConnectionState::SyncError { message } => {
            format!("Sync error · {}", short_message(message))
        }
    }
}

fn relative_time_since(secs: u64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let when = UNIX_EPOCH + Duration::from_secs(secs);
    let elapsed = SystemTime::now().duration_since(when).unwrap_or_default();
    let total_secs = elapsed.as_secs();
    if total_secs < 60 {
        return format!("{total_secs}s ago");
    }
    if total_secs < 3600 {
        return format!("{}m ago", total_secs / 60);
    }
    if total_secs < 86400 {
        return format!("{}h ago", total_secs / 3600);
    }
    format!("{}d ago", total_secs / 86400)
}

fn short_message(message: &str) -> String {
    const MAX_LEN: usize = 72;
    if message.len() <= MAX_LEN {
        return message.to_string();
    }
    let truncated: String = message.chars().take(MAX_LEN - 1).collect();
    format!("{truncated}…")
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

fn sync_object_state_label(state: SyncObjectState) -> &'static str {
    match state {
        SyncObjectState::LocalOnly => "Local only",
        SyncObjectState::Paused => "Paused",
        SyncObjectState::PrivacyControlled => "Privacy controlled",
        SyncObjectState::Synced => "Synced",
    }
}

fn pill_bg() -> u32 {
    colors::pick(0xffffffb3, 0x1f1d1bb3)
}
fn card_bg() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}
