use std::time::{SystemTime, UNIX_EPOCH};

use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::BrowserTab;
use gpui::{
    AnyElement, BoxShadow, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, hsla, point, px, rgb, rgba,
};
use gpui_component::{IconName, scroll::ScrollableElement};

use super::ElyShell;

impl ElyShell {
    pub(super) fn render_new_tab_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let greeting = greeting_for_now(SystemTime::now(), &snapshot.active_profile_name);

        div()
            .flex_1()
            .h_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .size_full()
                    .pt(px(48.0))
                    .px(px(64.0))
                    .pb(px(28.0))
                    .flex()
                    .flex_col()
                    .gap(px(48.0))
                    .child(render_hero(greeting, cx))
                    .child(render_favorites_grid(snapshot, cx))
                    .child(render_tabs_section(snapshot, cx)),
            )
            .into_any_element()
    }
}

fn render_hero(greeting: String, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(14.0))
        .child(render_greeting_row(greeting))
        .child(render_serif_headline())
        .child(render_search_bar(cx))
        .child(render_suggestion_pills(cx))
        .into_any_element()
}

fn render_greeting_row(text: String) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_size(px(13.0))
        .text_color(rgb(colors::INK_3))
        .child(
            div()
                .text_color(rgb(colors::ACCENT_LIGHT))
                .child(IconName::Sun),
        )
        .child(text)
        .into_any_element()
}

fn render_serif_headline() -> AnyElement {
    div()
        .text_size(px(64.0))
        .font_weight(FontWeight(400.0))
        .text_color(rgb(colors::INK))
        .child("Where focus finds flow.")
        .into_any_element()
}

fn render_search_bar(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("home-search"))
        .w(px(640.0))
        .h(px(54.0))
        .rounded(px(14.0))
        .bg(rgba(SEARCH_BG))
        .shadow(card_shadow())
        .px(px(16.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .cursor_pointer()
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.focus_address_bar(window, cx);
        }))
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .child(IconName::Search),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(14.0))
                .text_color(rgb(colors::INK_4))
                .child("Search the web or ELY"),
        )
        .child(
            div()
                .size(px(28.0))
                .rounded(px(8.0))
                .bg(rgba(ARROW_CHIP_BG))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(colors::INK_3))
                .child(IconName::ArrowRight),
        )
        .into_any_element()
}

fn render_suggestion_pills(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .child(render_pill(
            "pill-search-tabs",
            IconName::Search,
            "Search Tabs",
            cx,
            |shell, window, cx| shell.focus_address_bar(window, cx),
        ))
        .child(render_pill(
            "pill-switch-workspace",
            IconName::LayoutDashboard,
            "Switch Workspace",
            cx,
            |shell, window, cx| shell.cycle_to_next_space(window, cx),
        ))
        .child(render_pill(
            "pill-open-history",
            IconName::Undo2,
            "Open History",
            cx,
            |shell, window, cx| shell.open_internal_tab("ely://history", window, cx),
        ))
        .into_any_element()
}

fn render_pill<F>(
    id: &'static str,
    icon: IconName,
    label: &'static str,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    div()
        .id(SharedString::from(id))
        .rounded(px(999.0))
        .bg(rgba(PILL_BG))
        .shadow(soft_shadow())
        .px(px(12.0))
        .py(px(6.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(PILL_BG_HOVER)))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, window, cx| handler(shell, window, cx)))
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .text_size(px(12.0))
                .child(icon),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::INK_2))
                .child(label),
        )
        .into_any_element()
}

fn render_favorites_grid(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    if snapshot.favorites.is_empty() {
        return div().into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(render_section_chevron_label("Favorites"))
        .child(
            div()
                .grid()
                .grid_cols(7)
                .gap(px(10.0))
                .children(
                    snapshot
                        .favorites
                        .iter()
                        .enumerate()
                        .map(|(index, tab)| render_favorite_tile(index, tab, cx)),
                )
                .child(render_add_favorite_tile(cx)),
        )
        .into_any_element()
}

fn render_section_chevron_label(label: &'static str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_color(rgb(colors::INK_3))
        .text_size(px(12.5))
        .font_weight(FontWeight(500.0))
        .child(label)
        .child(
            div()
                .text_color(rgb(colors::INK_4))
                .child(IconName::ChevronDown),
        )
        .into_any_element()
}

fn render_favorite_tile(
    index: usize,
    tab: &BrowserTab,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let tab_id = tab.id().clone();
    let initial = tab
        .title()
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let title = tab.title().to_string();

    div()
        .id(SharedString::from(format!("fav-tile-{index}")))
        .h(px(96.0))
        .rounded(px(16.0))
        .bg(rgba(CARD_BG))
        .shadow(card_shadow())
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(CARD_BG_HOVER)))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_tab(&tab_id, window, cx);
        }))
        .child(
            div()
                .size(px(28.0))
                .rounded(px(7.0))
                .bg(rgb(colors::ACCENT))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.0))
                .font_weight(FontWeight(600.0))
                .text_color(rgb(0xffffff))
                .child(initial),
        )
        .child(
            div()
                .text_size(px(11.5))
                .text_color(rgb(colors::INK_3))
                .max_w(px(80.0))
                .truncate()
                .child(title),
        )
        .into_any_element()
}

fn render_add_favorite_tile(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("fav-tile-add"))
        .h(px(96.0))
        .rounded(px(16.0))
        .bg(rgba(ADD_TILE_BG))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(colors::INK_4))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(CARD_BG)))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.open_internal_tab("ely://bookmarks", window, cx);
        }))
        .child(IconName::Plus)
        .into_any_element()
}

fn render_tabs_section(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    if snapshot.tabs.is_empty() {
        return div().into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(render_section_chevron_label("Open Tabs"))
        .child(
            div()
                .flex()
                .flex_col()
                .children(snapshot.tabs.iter().enumerate().map(|(index, tab)| {
                    render_tab_row(index, tab, &snapshot.active_tab_id, cx)
                })),
        )
        .into_any_element()
}

fn render_tab_row(
    index: usize,
    tab: &BrowserTab,
    active_tab_id: &ely_domain::TabId,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let tab_id = tab.id().clone();
    let active = tab.id() == active_tab_id;
    let bg = if active { CARD_BG_HOVER } else { 0x00000000 };

    div()
        .id(SharedString::from(format!("home-tab-row-{index}")))
        .rounded(px(8.0))
        .px(px(10.0))
        .py(px(8.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .cursor_pointer()
        .bg(rgba(bg))
        .hover(|style| style.bg(rgba(CARD_BG_HOVER)))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_tab(&tab_id, window, cx);
        }))
        .child(
            div()
                .text_color(if active { rgb(colors::ACCENT) } else { rgb(colors::INK_4) })
                .child(IconName::Globe),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::INK))
                        .truncate()
                        .child(tab.title().to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(colors::INK_4))
                        .truncate()
                        .child(tab.display_url()),
                ),
        )
        .into_any_element()
}

fn greeting_for_now(now: SystemTime, name: &str) -> String {
    let phase = match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let local_seconds = duration.as_secs() as i64;
            let day_seconds = local_seconds.rem_euclid(86_400);
            let hour = (day_seconds / 3_600) as u32;
            day_phase(hour)
        }
        Err(_) => "today",
    };

    if name.is_empty() {
        format!("Good {phase}")
    } else {
        format!("Good {phase}, {name}")
    }
}

fn day_phase(hour_utc: u32) -> &'static str {
    match hour_utc {
        5..=11 => "morning",
        12..=17 => "afternoon",
        _ => "evening",
    }
}

const SEARCH_BG: u32 = 0xffffffd9;
const ARROW_CHIP_BG: u32 = 0x281e140a;
const PILL_BG: u32 = 0xffffff8c;
const PILL_BG_HOVER: u32 = 0xffffffd9;
const CARD_BG: u32 = 0xffffffc7;
const CARD_BG_HOVER: u32 = 0xffffffeb;
const ADD_TILE_BG: u32 = 0xffffff7f;

fn card_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(25.0 / 360.0, 0.33, 0.12, 0.18),
        offset: point(px(0.0), px(8.0)),
        blur_radius: px(24.0),
        spread_radius: px(-10.0),
    }]
}

fn soft_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: hsla(0.0, 0.0, 1.0, 0.7),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(0.0),
            spread_radius: px(0.0),
        },
        BoxShadow {
            color: hsla(25.0 / 360.0, 0.33, 0.12, 0.08),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{day_phase, greeting_for_now};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn day_phase_assigns_morning_to_seven_am() {
        assert_eq!(day_phase(7), "morning");
    }

    #[test]
    fn day_phase_assigns_afternoon_to_one_pm() {
        assert_eq!(day_phase(13), "afternoon");
    }

    #[test]
    fn day_phase_assigns_evening_to_eleven_pm() {
        assert_eq!(day_phase(23), "evening");
    }

    #[test]
    fn greeting_includes_profile_name() {
        let two_pm = UNIX_EPOCH + Duration::from_secs(14 * 3_600);
        assert_eq!(greeting_for_now(two_pm, "Alex"), "Good afternoon, Alex");
    }

    #[test]
    fn greeting_omits_name_when_blank() {
        let nine_am = UNIX_EPOCH + Duration::from_secs(9 * 3_600);
        assert_eq!(greeting_for_now(nine_am, ""), "Good morning");
    }
}
