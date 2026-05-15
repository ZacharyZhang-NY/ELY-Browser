use ely_design_system::colors;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{IconName, StyledExt};

use crate::brand::{AUTH_CALLBACK_URL, SYNC_SERVICE_NAME};

use super::{ElyShell, render_canvas_surface};

#[derive(Clone, Debug, Eq, PartialEq)]
enum AuthCallbackState {
    CodeReceived,
    ProviderError(String),
    MissingCode,
}

impl ElyShell {
    pub(super) fn render_auth_callback_page(&mut self, url: &str) -> AnyElement {
        let state = auth_callback_state(url);
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_auth_callback_header(&state))
                .child(render_auth_callback_rows(&state)),
        )
    }
}

fn render_auth_callback_header(state: &AuthCallbackState) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div().text_size(px(26.0)).text_color(rgb(colors::ink())).child("Auth Callback"),
                )
                .child(div().text_sm().text_color(rgb(colors::muted())).child(SYNC_SERVICE_NAME)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .font_semibold()
                .text_color(rgb(auth_callback_color(state)))
                .child(auth_callback_icon(state))
                .child(auth_callback_badge(state)),
        )
        .into_any_element()
}

fn render_auth_callback_rows(state: &AuthCallbackState) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .border_t_1()
        .border_color(rgb(colors::hairline()))
        .child(auth_callback_row(
            IconName::Globe,
            "Service",
            SYNC_SERVICE_NAME,
            "Cloud account authority",
        ))
        .child(auth_callback_row(
            IconName::ExternalLink,
            "Callback",
            AUTH_CALLBACK_URL,
            "Desktop protocol",
        ))
        .child(auth_callback_row(
            auth_callback_icon(state),
            "Result",
            auth_callback_result(state),
            auth_callback_detail(state),
        ))
        .into_any_element()
}

fn auth_callback_row(
    icon: IconName,
    label: &'static str,
    value: impl Into<String>,
    detail: impl Into<String>,
) -> AnyElement {
    div()
        .py_4()
        .border_b_1()
        .border_color(rgb(colors::hairline()))
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
                .child(div().text_color(rgb(colors::muted())).child(icon))
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
                                .text_color(rgb(colors::ink()))
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::muted()))
                                .child(detail.into()),
                        ),
                ),
        )
        .child(div().text_xs().font_semibold().text_color(rgb(colors::muted())).child(value.into()))
        .into_any_element()
}

fn auth_callback_state(url: &str) -> AuthCallbackState {
    if let Some(error) = query_value(url, "error")
        && !error.is_empty()
    {
        return AuthCallbackState::ProviderError(error);
    }

    if query_value(url, "code").is_some_and(|code| !code.is_empty()) {
        return AuthCallbackState::CodeReceived;
    }

    AuthCallbackState::MissingCode
}

fn query_value(url: &str, name: &str) -> Option<String> {
    let query = url.strip_prefix(AUTH_CALLBACK_URL)?.strip_prefix('?')?;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| value.to_string())
    })
}

fn auth_callback_icon(state: &AuthCallbackState) -> IconName {
    match state {
        AuthCallbackState::CodeReceived => IconName::CircleCheck,
        AuthCallbackState::ProviderError(_) => IconName::TriangleAlert,
        AuthCallbackState::MissingCode => IconName::Info,
    }
}

fn auth_callback_color(state: &AuthCallbackState) -> u32 {
    match state {
        AuthCallbackState::CodeReceived => colors::success(),
        AuthCallbackState::ProviderError(_) => colors::error(),
        AuthCallbackState::MissingCode => colors::muted(),
    }
}

fn auth_callback_badge(state: &AuthCallbackState) -> &'static str {
    match state {
        AuthCallbackState::CodeReceived => "Code received",
        AuthCallbackState::ProviderError(_) => "Provider error",
        AuthCallbackState::MissingCode => "Callback pending",
    }
}

fn auth_callback_result(state: &AuthCallbackState) -> String {
    match state {
        AuthCallbackState::CodeReceived => "Authorization code".to_string(),
        AuthCallbackState::ProviderError(error) => error.to_string(),
        AuthCallbackState::MissingCode => "Awaiting code".to_string(),
    }
}

fn auth_callback_detail(state: &AuthCallbackState) -> &'static str {
    match state {
        AuthCallbackState::CodeReceived => "Ready for session exchange",
        AuthCallbackState::ProviderError(_) => "Provider returned an OAuth error",
        AuthCallbackState::MissingCode => "Callback code missing",
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthCallbackState, auth_callback_state, query_value};

    #[test]
    fn auth_callback_state_reads_code() {
        assert_eq!(
            auth_callback_state("ely://auth/callback?code=abc&state=nonce"),
            AuthCallbackState::CodeReceived
        );
    }

    #[test]
    fn auth_callback_state_reads_provider_error() {
        assert_eq!(
            auth_callback_state("ely://auth/callback?error=access_denied"),
            AuthCallbackState::ProviderError("access_denied".to_string())
        );
    }

    #[test]
    fn auth_callback_state_handles_missing_code() {
        assert_eq!(auth_callback_state("ely://auth/callback"), AuthCallbackState::MissingCode);
    }

    #[test]
    fn query_value_filters_other_routes() {
        assert_eq!(query_value("ely://history?code=abc", "code"), None);
    }
}
