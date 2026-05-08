use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::BrowserTab;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::StyledExt;

pub(super) fn tab_context_row(label: &'static str, value: String) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(div().min_w(px(128.0)).text_xs().text_color(rgb(colors::MUTED)).child(label))
        .child(
            div()
                .min_w_0()
                .text_sm()
                .font_semibold()
                .truncate()
                .text_color(rgb(colors::INK))
                .child(value),
        )
        .into_any_element()
}

pub(super) fn favicon_status(tab: &BrowserTab) -> String {
    tab.favicon_key()
        .map_or_else(|| "No favicon saved".to_string(), |favicon| format!("Saved: {favicon}"))
}

pub(super) fn space_name(snapshot: &BrowserSnapshot, tab: &BrowserTab) -> String {
    snapshot
        .spaces
        .iter()
        .find(|space| space.id() == tab.space_id())
        .map_or_else(|| tab.space_id().as_str().to_string(), |space| space.name().to_string())
}

pub(super) fn profile_name(snapshot: &BrowserSnapshot, tab: &BrowserTab) -> String {
    snapshot
        .profiles
        .iter()
        .find(|profile| profile.id() == tab.profile_id())
        .map_or_else(|| tab.profile_id().as_str().to_string(), |profile| profile.name().to_string())
}
