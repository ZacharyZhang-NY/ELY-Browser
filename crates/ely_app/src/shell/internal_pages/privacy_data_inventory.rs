use ely_browser_core::{BrowserSnapshot, LocalDataInventory};
use ely_design_system::colors;
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, rgb};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};

use super::ElyShell;

pub(super) fn render_local_data_inventory(
    snapshot: &BrowserSnapshot,
    notice: Option<&str>,
    error: Option<&str>,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let inventory = snapshot.local_data_inventory;

    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::hairline()))
        .bg(rgb(colors::canvas_soft()))
        .px_4()
        .py_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(render_inventory_header(snapshot, inventory, cx))
        .when_some(file_message(notice, error), |this, message| this.child(message))
        .child(render_inventory_rows(inventory))
        .into_any_element()
}

fn render_inventory_header(
    snapshot: &BrowserSnapshot,
    inventory: LocalDataInventory,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_3()
                .child(div().text_color(rgb(colors::primary())).child(IconName::Inspector))
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
                                .child("Local Data"),
                        )
                        .child(div().text_xs().truncate().text_color(rgb(colors::muted())).child(
                            format!(
                                "{} Profile inventory for review, export, and deletion.",
                                snapshot.active_profile_name
                            ),
                        )),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(colors::hairline()))
                        .bg(rgb(colors::canvas()))
                        .px_3()
                        .py_2()
                        .text_xs()
                        .font_semibold()
                        .text_color(rgb(colors::ink()))
                        .child(format!("{} items", inventory.total_items())),
                )
                .child(
                    Button::new("export-local-data")
                        .ghost()
                        .xsmall()
                        .icon(IconName::File)
                        .label("Export")
                        .tooltip("Export Local Data")
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.export_local_data(window, cx);
                        })),
                ),
        )
        .into_any_element()
}

fn file_message(notice: Option<&str>, error: Option<&str>) -> Option<AnyElement> {
    notice
        .map(|message| render_file_message(message, colors::success()))
        .or_else(|| error.map(|message| render_file_message(message, colors::error())))
}

fn render_file_message(message: &str, color: u32) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(color))
        .px_3()
        .py_2()
        .text_xs()
        .font_semibold()
        .text_color(rgb(color))
        .child(message.to_string())
        .into_any_element()
}

fn render_inventory_rows(inventory: LocalDataInventory) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(inventory_row(IconName::Globe, "Open Tabs", inventory.open_tabs()))
        .child(inventory_row(IconName::Inbox, "Archived Tabs", inventory.archived_tabs()))
        .child(inventory_row(IconName::Undo2, "History", inventory.history_entries()))
        .child(inventory_row(IconName::BookOpen, "Bookmarks", inventory.bookmarks()))
        .child(inventory_row(IconName::File, "Notes", inventory.notes()))
        .child(inventory_row(IconName::CircleCheck, "Reading List", inventory.reading_list()))
        .child(inventory_row(IconName::Folder, "Downloads", inventory.downloads()))
        .child(inventory_row(IconName::Eye, "Site Permissions", inventory.site_permissions()))
        .child(inventory_row(
            IconName::Inspector,
            "Audit Events",
            inventory.site_permission_audit_events(),
        ))
        .into_any_element()
}

fn inventory_row(icon: IconName, label: &'static str, count: usize) -> AnyElement {
    div()
        .py_2()
        .border_t_1()
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
                .child(div().text_color(rgb(colors::muted_soft())).child(icon))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(rgb(colors::ink()))
                        .child(label),
                ),
        )
        .child(
            div()
                .flex_none()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::muted()))
                .child(count.to_string()),
        )
        .into_any_element()
}
