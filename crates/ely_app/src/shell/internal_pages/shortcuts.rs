use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use crate::shortcuts::{
    SHORTCUT_ACTIONS, ShortcutAction, ShortcutConflict, ShortcutPlatform, bindings_for_action,
    shortcut_conflicts,
};

use super::{ElyShell, render_canvas_surface};

const SHORTCUT_CATEGORIES: &[&str] = &["Command", "Tabs", "Library", "System", "Application"];

impl ElyShell {
    pub(super) fn render_shortcuts_page(&mut self, snapshot: &BrowserSnapshot) -> AnyElement {
        let conflicts = shortcut_conflicts();

        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_shortcuts_header(snapshot, conflicts.len()))
                .child(render_conflict_panel(&conflicts))
                .child(render_shortcut_categories(&conflicts)),
        )
    }
}

fn render_shortcuts_header(snapshot: &BrowserSnapshot, conflict_count: usize) -> AnyElement {
    let status = if conflict_count == 0 {
        "Ready".to_string()
    } else {
        format!("{conflict_count} conflicts")
    };

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
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Shortcuts"))
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
                .text_color(rgb(shortcut_status_color(conflict_count)))
                .child(shortcut_status_icon(conflict_count))
                .child(status),
        )
        .into_any_element()
}

fn render_conflict_panel(conflicts: &[ShortcutConflict]) -> AnyElement {
    let (icon, title, detail, color) = if conflicts.is_empty() {
        (
            IconName::CircleCheck,
            "No shortcut conflicts",
            "Every registered browser shortcut maps to a single action.",
            colors::SUCCESS,
        )
    } else {
        (
            IconName::TriangleAlert,
            "Shortcut conflicts",
            "Conflicting bindings need a new key before customization is enabled.",
            colors::ERROR,
        )
    };

    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::HAIRLINE))
        .bg(rgb(colors::CANVAS_SOFT))
        .px_4()
        .py_3()
        .flex()
        .items_start()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .items_start()
                .gap_3()
                .child(div().text_color(rgb(color)).child(icon))
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
                                .child(title),
                        )
                        .child(div().text_xs().text_color(rgb(colors::MUTED)).child(detail)),
                ),
        )
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(rgb(color))
                .child(conflicts.len().to_string()),
        )
        .into_any_element()
}

fn render_shortcut_categories(conflicts: &[ShortcutConflict]) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(
            SHORTCUT_CATEGORIES
                .iter()
                .map(|category| render_shortcut_category(category, conflicts)),
        )
        .into_any_element()
}

fn render_shortcut_category(category: &'static str, conflicts: &[ShortcutConflict]) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(render_category_header(category))
        .children(
            SHORTCUT_ACTIONS
                .iter()
                .copied()
                .filter(move |action| action.category() == category)
                .map(|action| render_shortcut_row(action, conflicts)),
        )
        .into_any_element()
}

fn render_category_header(category: &'static str) -> AnyElement {
    div()
        .pt_4()
        .pb_2()
        .text_xs()
        .font_semibold()
        .text_color(rgb(colors::MUTED))
        .child(category)
        .into_any_element()
}

fn render_shortcut_row(action: ShortcutAction, conflicts: &[ShortcutConflict]) -> AnyElement {
    let has_conflict = conflicts
        .iter()
        .any(|conflict| conflict.actions.iter().any(|conflict_action| conflict_action == &action));

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
                        .text_color(rgb(shortcut_row_icon_color(has_conflict)))
                        .child(shortcut_row_icon(has_conflict)),
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
                                .child(action.label()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(action.command().unwrap_or("Key binding only")),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_3()
                .text_xs()
                .child(shortcut_platform_label(action, ShortcutPlatform::Macos))
                .child(shortcut_platform_label(action, ShortcutPlatform::WindowsLinux))
                .child(shortcut_row_status(has_conflict)),
        )
        .into_any_element()
}

fn shortcut_platform_label(action: ShortcutAction, platform: ShortcutPlatform) -> AnyElement {
    div()
        .min_w(px(170.0))
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(colors::MUTED_SOFT)).child(platform.label()))
        .child(
            div()
                .font_semibold()
                .text_color(rgb(colors::INK))
                .child(shortcut_keys_label(action, platform)),
        )
        .into_any_element()
}

fn shortcut_row_status(has_conflict: bool) -> AnyElement {
    let (label, color) =
        if has_conflict { ("Conflict", colors::ERROR) } else { ("Ready", colors::SUCCESS) };

    div().min_w(px(72.0)).font_semibold().text_color(rgb(color)).child(label).into_any_element()
}

fn shortcut_keys_label(action: ShortcutAction, platform: ShortcutPlatform) -> String {
    let bindings = bindings_for_action(action, platform)
        .map(|binding| binding.display_keystroke())
        .collect::<Vec<_>>();

    if bindings.is_empty() {
        return "Unassigned".to_string();
    }

    bindings.join(" / ")
}

fn shortcut_status_color(conflict_count: usize) -> u32 {
    if conflict_count == 0 { colors::SUCCESS } else { colors::ERROR }
}

fn shortcut_status_icon(conflict_count: usize) -> IconName {
    if conflict_count == 0 { IconName::CircleCheck } else { IconName::TriangleAlert }
}

fn shortcut_row_icon(has_conflict: bool) -> IconName {
    if has_conflict { IconName::TriangleAlert } else { IconName::SquareTerminal }
}

fn shortcut_row_icon_color(has_conflict: bool) -> u32 {
    if has_conflict { colors::ERROR } else { colors::MUTED_SOFT }
}
