use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use crate::shortcuts::{
    SHORTCUT_ACTIONS, ShortcutAction, ShortcutConflict, ShortcutPlatform, ShortcutProfile,
};

use super::{ElyShell, render_canvas_surface};

const SHORTCUT_CATEGORIES: &[&str] = &["Command", "Tabs", "Library", "System", "Application"];

impl ElyShell {
    pub(super) fn render_shortcuts_page(
        &mut self,
        _snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let conflicts = self.shortcut_profile.conflicts();

        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_shortcuts_header(cx))
                .child(render_shortcut_file_message(
                    self.shortcut_file_notice.as_deref(),
                    self.shortcut_file_error.as_deref(),
                ))
                .when(!conflicts.is_empty(), |this| this.child(render_conflict_panel(&conflicts)))
                .child(render_shortcut_categories(&self.shortcut_profile, &conflicts)),
        )
    }
}

fn render_shortcuts_header(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(div().text_size(px(26.0)).text_color(rgb(colors::ink())).child("Shortcuts"))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("export-shortcuts")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ArrowUp)
                        .label("Export")
                        .tooltip("Export Shortcuts")
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.export_shortcuts(window, cx);
                        })),
                )
                .child(
                    Button::new("import-shortcuts")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ArrowDown)
                        .label("Import")
                        .tooltip("Import Shortcuts")
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.choose_shortcut_import(window, cx);
                        })),
                ),
        )
        .into_any_element()
}

fn render_shortcut_file_message(notice: Option<&str>, error: Option<&str>) -> AnyElement {
    let Some((message, color, icon)) = error
        .map(|message| (message, colors::error(), IconName::TriangleAlert))
        .or_else(|| notice.map(|message| (message, colors::success(), IconName::CircleCheck)))
    else {
        return div().hidden().into_any_element();
    };

    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(color))
        .bg(rgb(colors::canvas_soft()))
        .px_4()
        .py_2()
        .flex()
        .items_center()
        .gap_2()
        .text_xs()
        .font_semibold()
        .text_color(rgb(color))
        .child(icon)
        .child(message.to_string())
        .into_any_element()
}

fn render_conflict_panel(conflicts: &[ShortcutConflict]) -> AnyElement {
    let (icon, title, detail, color) = if conflicts.is_empty() {
        (
            IconName::CircleCheck,
            "No shortcut conflicts",
            "Every registered browser shortcut maps to a single action.",
            colors::success(),
        )
    } else {
        (
            IconName::TriangleAlert,
            "Shortcut conflicts",
            "Conflicting bindings need a new key before customization is enabled.",
            colors::error(),
        )
    };

    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::hairline()))
        .bg(rgb(colors::canvas_soft()))
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
                                .text_color(rgb(colors::ink()))
                                .child(title),
                        )
                        .child(div().text_xs().text_color(rgb(colors::muted())).child(detail)),
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

fn render_shortcut_categories(
    profile: &ShortcutProfile,
    conflicts: &[ShortcutConflict],
) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::hairline()))
        .children(
            SHORTCUT_CATEGORIES
                .iter()
                .map(|category| render_shortcut_category(profile, category, conflicts)),
        )
        .into_any_element()
}

fn render_shortcut_category(
    profile: &ShortcutProfile,
    category: &'static str,
    conflicts: &[ShortcutConflict],
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(render_category_header(category))
        .children(
            SHORTCUT_ACTIONS
                .iter()
                .copied()
                .filter(move |action| action.category() == category)
                .map(|action| render_shortcut_row(profile, action, conflicts)),
        )
        .into_any_element()
}

fn render_category_header(category: &'static str) -> AnyElement {
    div()
        .pt_4()
        .pb_2()
        .text_xs()
        .font_semibold()
        .text_color(rgb(colors::muted()))
        .child(category)
        .into_any_element()
}

fn render_shortcut_row(
    profile: &ShortcutProfile,
    action: ShortcutAction,
    conflicts: &[ShortcutConflict],
) -> AnyElement {
    let has_conflict = conflicts
        .iter()
        .any(|conflict| conflict.actions.iter().any(|conflict_action| conflict_action == &action));

    div()
        .py_3()
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
                                .text_color(rgb(colors::ink()))
                                .child(action.label()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::muted()))
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
                .child(shortcut_platform_label(profile, action, ShortcutPlatform::Macos))
                .child(shortcut_platform_label(profile, action, ShortcutPlatform::WindowsLinux))
                .when(has_conflict, |this| this.child(shortcut_row_status())),
        )
        .into_any_element()
}

fn shortcut_platform_label(
    profile: &ShortcutProfile,
    action: ShortcutAction,
    platform: ShortcutPlatform,
) -> AnyElement {
    div()
        .min_w(px(170.0))
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(colors::muted_soft())).child(platform.label()))
        .child(
            div()
                .font_semibold()
                .text_color(rgb(colors::ink()))
                .child(profile.display_bindings_for_action(action, platform)),
        )
        .into_any_element()
}

fn shortcut_row_status() -> AnyElement {
    div()
        .min_w(px(72.0))
        .font_semibold()
        .text_color(rgb(colors::error()))
        .child("Conflict")
        .into_any_element()
}

fn shortcut_row_icon(has_conflict: bool) -> IconName {
    if has_conflict { IconName::TriangleAlert } else { IconName::SquareTerminal }
}

fn shortcut_row_icon_color(has_conflict: bool) -> u32 {
    if has_conflict { colors::error() } else { colors::muted_soft() }
}
