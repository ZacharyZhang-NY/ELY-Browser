use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{DownloadChecksum, DownloadEntry};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
    px, rgb,
};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::super::PendingDownloadFileAction;
use super::{
    ElyShell,
    download_labels::{
        download_entry_location_label, download_policy_label, download_security_label,
        download_size_label, download_state_label,
    },
    render_canvas_surface,
};

impl ElyShell {
    pub(super) fn render_downloads_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(
                    div()
                        .flex()
                        .items_end()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(26.0))
                                        .text_color(rgb(colors::INK))
                                        .child("Downloads"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(colors::MUTED))
                                        .child(snapshot.active_profile_name.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_end()
                                .gap_1()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div().text_xs().text_color(rgb(colors::MUTED)).child(
                                                format!(
                                                    "{} downloads",
                                                    snapshot.download_entries.len()
                                                ),
                                            ),
                                        )
                                        .when(!snapshot.download_entries.is_empty(), |this| {
                                            this.child(
                                                Button::new("clear-downloads")
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(IconName::Delete)
                                                    .tooltip("Clear Downloads")
                                                    .on_click(cx.listener(|shell, _, _, cx| {
                                                        shell
                                                            .request_clear_active_profile_downloads(
                                                                cx,
                                                            );
                                                    })),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_xs()
                                        .text_color(rgb(colors::MUTED))
                                        .child(IconName::Folder)
                                        .child(div().max_w(px(360.0)).truncate().child(
                                            download_policy_label(&snapshot.active_download_policy),
                                        )),
                                ),
                        ),
                )
                .when_some(self.download_action_error.clone(), |this, message| {
                    this.child(render_download_action_error(message))
                })
                .when(
                    self.download_clear_confirmation && !snapshot.download_entries.is_empty(),
                    |this| this.child(self.render_download_clear_confirmation(snapshot, cx)),
                )
                .when_some(self.download_security_confirmation.clone(), |this, pending_action| {
                    this.child(self.render_download_security_confirmation(&pending_action, cx))
                })
                .child(self.render_downloads_list(snapshot, cx)),
        )
    }

    fn render_download_clear_confirmation(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .rounded_md()
            .border_1()
            .border_color(rgb(colors::HAIRLINE))
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(div().min_w_0().text_xs().text_color(rgb(colors::BODY)).truncate().child(
                format!(
                    "Clear {} downloads for {}?",
                    snapshot.download_entries.len(),
                    snapshot.active_profile_name
                ),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("cancel-clear-downloads")
                            .ghost()
                            .xsmall()
                            .label("Cancel")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.cancel_clear_active_profile_downloads(cx);
                            })),
                    )
                    .child(
                        Button::new("confirm-clear-downloads")
                            .danger()
                            .xsmall()
                            .label("Clear")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.clear_active_profile_downloads(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_download_security_confirmation(
        &mut self,
        pending_action: &PendingDownloadFileAction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .rounded_md()
            .border_1()
            .border_color(rgb(colors::ERROR))
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(rgb(colors::ERROR))
                    .child(IconName::TriangleAlert)
                    .child(div().truncate().child(format!(
                        "Confirm {} for {}",
                        pending_action.action_label(),
                        pending_action.file_name()
                    ))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("cancel-security-download")
                            .ghost()
                            .xsmall()
                            .label("Cancel")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.cancel_download_security_prompt(cx);
                            })),
                    )
                    .child(
                        Button::new("confirm-security-download")
                            .danger()
                            .xsmall()
                            .label("Confirm")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.confirm_download_security_prompt(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_downloads_list(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.download_entries.is_empty() {
            return div()
                .flex_1()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .pt_5()
                .text_sm()
                .text_color(rgb(colors::MUTED))
                .child("Downloads are empty for this Profile.")
                .into_any_element();
        }

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_y_scrollbar()
            .border_t_1()
            .border_color(rgb(colors::HAIRLINE))
            .children(
                snapshot
                    .download_entries
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(index, entry)| self.render_download_row(index, entry, cx)),
            )
            .into_any_element()
    }

    fn render_download_row(
        &mut self,
        index: usize,
        entry: &DownloadEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(SharedString::from(format!("download-{index}")))
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
                    .child(div().text_color(rgb(colors::MUTED)).child(IconName::File))
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
                                    .child(entry.file_name().to_string()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(rgb(colors::MUTED))
                                    .child(entry.source_url().display_url()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(rgb(colors::MUTED))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .child(download_entry_location_label(entry)),
                                    )
                                    .when(entry.security().requires_prompt(), |this| {
                                        this.child(render_security_prompt(entry))
                                    })
                                    .when_some(entry.checksum(), |this, checksum| {
                                        this.child(render_checksum_label(checksum))
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_3()
                    .child(
                        div()
                            .min_w(px(92.0))
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(rgb(colors::BODY))
                                    .child(download_state_label(entry.state())),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(colors::MUTED))
                                    .child(download_size_label(entry)),
                            ),
                    )
                    .child(self.render_download_actions(index, entry, cx)),
            )
            .into_any_element()
    }
}

fn render_download_action_error(message: String) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::ERROR))
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_2()
        .text_xs()
        .text_color(rgb(colors::ERROR))
        .child(IconName::TriangleAlert)
        .child(message)
        .into_any_element()
}

fn render_security_prompt(entry: &DownloadEntry) -> AnyElement {
    let prompt_color =
        if entry.requires_security_confirmation() { colors::ERROR } else { colors::SUCCESS };
    let prompt_icon = if entry.requires_security_confirmation() {
        IconName::TriangleAlert
    } else {
        IconName::CircleCheck
    };

    div()
        .flex()
        .items_center()
        .gap_1()
        .text_color(rgb(prompt_color))
        .child(prompt_icon)
        .child(download_security_label(entry))
        .into_any_element()
}

fn render_checksum_label(checksum: &DownloadChecksum) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .text_color(rgb(colors::SUCCESS))
        .child(IconName::CircleCheck)
        .child(format!("SHA-256 {}", short_checksum(checksum.value())))
        .into_any_element()
}

fn short_checksum(value: &str) -> &str {
    value.get(..12).unwrap_or(value)
}
