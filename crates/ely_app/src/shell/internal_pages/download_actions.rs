use ely_domain::{DownloadEntry, DownloadState};
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{
    IconName, Sizable,
    button::{Button, ButtonVariants},
};

use super::ElyShell;

impl ElyShell {
    pub(super) fn render_download_actions(
        &mut self,
        index: usize,
        entry: &DownloadEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .w(px(64.0))
            .flex()
            .items_center()
            .justify_end()
            .gap_1()
            .when(matches!(entry.state(), DownloadState::InProgress), |this| {
                let pause_id = entry.id().clone();
                let cancel_id = entry.id().clone();

                this.child(
                    download_action_button("pause", index, IconName::Minus, "Pause Download")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.pause_download(&pause_id, cx);
                        }))
                        .into_any_element(),
                )
                .child(
                    download_action_button("cancel", index, IconName::Close, "Cancel Download")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.cancel_download(&cancel_id, cx);
                        }))
                        .into_any_element(),
                )
            })
            .when(matches!(entry.state(), DownloadState::Paused), |this| {
                let resume_id = entry.id().clone();
                let cancel_id = entry.id().clone();

                this.child(
                    download_action_button(
                        "resume",
                        index,
                        IconName::ArrowRight,
                        "Resume Download",
                    )
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        shell.resume_download(&resume_id, cx);
                    }))
                    .into_any_element(),
                )
                .child(
                    download_action_button("cancel", index, IconName::Close, "Cancel Download")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.cancel_download(&cancel_id, cx);
                        }))
                        .into_any_element(),
                )
            })
            .when(
                matches!(entry.state(), DownloadState::Cancelled | DownloadState::Failed),
                |this| {
                    let retry_id = entry.id().clone();

                    this.child(
                        download_action_button("retry", index, IconName::Undo2, "Retry Download")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.retry_download(&retry_id, cx);
                            }))
                            .into_any_element(),
                    )
                },
            )
            .when(
                matches!(entry.state(), DownloadState::Completed)
                    && entry.target_file_path().is_some(),
                |this| {
                    let open_id = entry.id().clone();
                    let reveal_id = entry.id().clone();

                    this.child(
                        download_action_button("open", index, IconName::ExternalLink, "Open File")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.open_download_file(&open_id, cx);
                            }))
                            .into_any_element(),
                    )
                    .child(
                        download_action_button(
                            "reveal",
                            index,
                            IconName::FolderOpen,
                            "Reveal in Finder",
                        )
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.reveal_download_file(&reveal_id, cx);
                        }))
                        .into_any_element(),
                    )
                },
            )
            .into_any_element()
    }
}

fn download_action_button(
    action: &'static str,
    index: usize,
    icon: IconName,
    tooltip: &'static str,
) -> Button {
    Button::new((action, index)).ghost().xsmall().icon(icon).tooltip(tooltip)
}
