use std::time::SystemTime;

use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{HistoryEntry, ReadingListEntry};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient, px, rgb,
    rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::{SERIF_FAMILY, render_glyph_for};

use super::style::{card_bg, card_shadow};
use super::time::relative_time_label;

pub(crate) fn render_recap(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    if snapshot.history_entries.is_empty() && snapshot.reading_list.is_empty() {
        return div().into_any_element();
    }

    let now = SystemTime::now();

    div()
        .grid()
        .grid_cols(8)
        .gap(px(12.0))
        .child(div().col_span(5).child(render_continue_card(snapshot, now, cx)))
        .child(div().col_span(3).child(render_activity_card(snapshot, now, cx)))
        .into_any_element()
}

fn render_continue_card(
    snapshot: &BrowserSnapshot,
    now: SystemTime,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let recent_history: Vec<&HistoryEntry> =
        snapshot.history_entries.iter().rev().take(3).collect();
    let featured = snapshot.reading_list.first();
    let reading_total = snapshot.reading_list.len();

    div()
        .rounded(px(16.0))
        .bg(rgba(card_bg()))
        .shadow(card_shadow())
        .p(px(16.0))
        .flex()
        .gap(px(16.0))
        .child(div().flex_1().min_w_0().child(render_continue_history(recent_history, now, cx)))
        .child(div().w(px(240.0)).flex_shrink_0().child(render_reading_list_cover(
            featured,
            reading_total,
            cx,
        )))
        .into_any_element()
}

fn render_continue_history(
    entries: Vec<&HistoryEntry>,
    now: SystemTime,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::ink()))
                .child("Continue where you left off"),
        )
        .child(if entries.is_empty() {
            render_empty_history_state()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .children(
                    entries
                        .into_iter()
                        .enumerate()
                        .map(|(index, entry)| render_history_row(index, entry, now, cx)),
                )
                .into_any_element()
        })
        .into_any_element()
}

fn render_empty_history_state() -> AnyElement {
    div()
        .text_size(px(12.0))
        .text_color(rgb(colors::ink_4()))
        .child("Your recently visited pages will appear here.")
        .into_any_element()
}

fn render_history_row(
    index: usize,
    entry: &HistoryEntry,
    now: SystemTime,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let url = entry.url().clone();
    let title = entry.title().to_string();
    let host = entry.url().host().map(|host| host.to_string());
    let host_label = host.clone().unwrap_or_default();
    let when = relative_time_label(now, entry.visited_at());
    let initial = title.chars().next().unwrap_or('?').to_string();

    div()
        .id(SharedString::from(format!("continue-row-{index}")))
        .flex()
        .items_center()
        .gap(px(12.0))
        .py(px(4.0))
        .cursor_pointer()
        .hover(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.open_internal_tab(url.as_str(), window, cx);
        }))
        .child(render_glyph_for(host.as_deref(), &initial, 26.0))
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
                        .text_color(rgb(colors::ink()))
                        .truncate()
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgb(colors::ink_4()))
                        .truncate()
                        .child(format!("{host_label} · {when}")),
                ),
        )
        .into_any_element()
}

fn render_reading_list_cover(
    featured: Option<&ReadingListEntry>,
    reading_total: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let title = featured
        .map(|entry| entry.title().to_string())
        .unwrap_or_else(|| "Add an article to your reading list".to_string());
    let subtitle =
        featured.map(|entry| entry.display_url()).unwrap_or_else(|| "Reading List".to_string());
    let url = featured.map(|entry| entry.source_url().clone());

    let mut cover = div()
        .id(SharedString::from("reading-list-cover"))
        .min_h(px(200.0))
        .rounded(px(12.0))
        .relative()
        .overflow_hidden()
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(20.0 / 360.0, 0.36, 0.78, 1.0), 0.0),
            linear_color_stop(hsla(220.0 / 360.0, 0.30, 0.71, 1.0), 1.0),
        ))
        .cursor_pointer()
        .hover(|style| style.opacity(0.96))
        .active(|style| style.opacity(0.88))
        .child(div().absolute().inset_0().bg(linear_gradient(
            225.0,
            linear_color_stop(hsla(345.0 / 360.0, 1.0, 0.85, 0.55), 0.0),
            linear_color_stop(hsla(345.0 / 360.0, 1.0, 0.85, 0.0), 0.6),
        )))
        .child(div().absolute().inset_0().bg(linear_gradient(
            45.0,
            linear_color_stop(hsla(228.0 / 360.0, 0.52, 0.62, 0.55), 0.0),
            linear_color_stop(hsla(228.0 / 360.0, 0.52, 0.62, 0.0), 0.7),
        )));

    if let Some(target) = url {
        cover = cover.on_click(cx.listener(move |shell, _, window, cx| {
            shell.open_internal_tab(target.as_str(), window, cx);
        }));
    } else {
        cover = cover.on_click(cx.listener(|shell, _, window, cx| {
            shell.open_internal_tab("ely://reading-list", window, cx);
        }));
    }

    cover
        .child(
            div()
                .absolute()
                .top(px(8.0))
                .right(px(8.0))
                .px(px(10.0))
                .py(px(4.0))
                .rounded(px(999.0))
                .bg(rgba(BADGE_BG))
                .text_size(px(10.5))
                .text_color(rgb(0xffffff))
                .child(format!("Reading List · {}", reading_total)),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .p(px(14.0))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .font_family(SERIF_FAMILY)
                        .text_size(px(18.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(0xffffff))
                        .child(title),
                )
                .child(div().text_size(px(11.0)).text_color(rgb(0xe6e3de)).child(subtitle)),
        )
        .into_any_element()
}

fn render_activity_card(
    snapshot: &BrowserSnapshot,
    now: SystemTime,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let entries: Vec<&HistoryEntry> = snapshot.history_entries.iter().rev().take(3).collect();

    div()
        .rounded(px(16.0))
        .bg(rgba(card_bg()))
        .shadow(card_shadow())
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::ink()))
                .child("Recent activity"),
        )
        .child(if entries.is_empty() {
            render_empty_activity_state()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .children(
                    entries
                        .into_iter()
                        .enumerate()
                        .map(|(index, entry)| render_activity_row(index, entry, now, cx)),
                )
                .into_any_element()
        })
        .child(render_view_all_link(cx))
        .into_any_element()
}

fn render_empty_activity_state() -> AnyElement {
    div()
        .text_size(px(12.0))
        .text_color(rgb(colors::ink_4()))
        .child("Activity from your tabs will land here.")
        .into_any_element()
}

fn render_activity_row(
    index: usize,
    entry: &HistoryEntry,
    now: SystemTime,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let url = entry.url().clone();
    let title = entry.title().to_string();
    let display_url = entry.url().as_str().to_string();
    let host = entry.url().host().map(|host| host.to_string());
    let initial = title.chars().next().unwrap_or('?').to_string();
    let when = relative_time_label(now, entry.visited_at());

    div()
        .id(SharedString::from(format!("activity-row-{index}")))
        .flex()
        .items_start()
        .gap(px(10.0))
        .cursor_pointer()
        .hover(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.open_internal_tab(url.as_str(), window, cx);
        }))
        .child(render_glyph_for(host.as_deref(), &initial, 24.0))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div().text_size(px(11.5)).text_color(rgb(colors::ink_3())).child("You visited"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::ink()))
                        .truncate()
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(rgb(colors::ink_4()))
                        .truncate()
                        .child(display_url),
                ),
        )
        .child(div().text_size(px(10.5)).text_color(rgb(colors::ink_4())).child(when))
        .into_any_element()
}

fn render_view_all_link(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("activity-view-all"))
        .pt(px(10.0))
        .border_t_1()
        .border_color(rgba(colors::divider()))
        .flex()
        .items_center()
        .gap(px(4.0))
        .text_size(px(11.5))
        .text_color(rgb(colors::ink_3()))
        .cursor_pointer()
        .hover(|style| style.text_color(rgb(colors::ink())))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.open_internal_tab("ely://history", window, cx);
        }))
        .child("View all history")
        .child(IconName::ArrowRight)
        .into_any_element()
}

const BADGE_BG: u32 = 0x00000059;
