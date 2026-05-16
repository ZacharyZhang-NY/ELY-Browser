use std::path::{Path, PathBuf};

use directories::UserDirs;
use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::DownloadPolicy;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

#[derive(Clone)]
struct DownloadPolicyOption {
    icon: IconName,
    title: &'static str,
    detail: String,
    policy: DownloadPolicy,
}

impl ElyShell {
    pub(super) fn render_download_settings_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let options = download_policy_options();

        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_download_settings_header(cx))
                .child(render_download_policy_rows(&snapshot.active_download_policy, &options, cx)),
        )
    }
}

fn render_download_settings_header(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(div().text_size(px(26.0)).text_color(rgb(colors::ink())).child("Downloads"))
        .child(
            Button::new("reset-download-settings")
                .ghost()
                .xsmall()
                .icon(IconName::Undo2)
                .label("Reset")
                .tooltip("Restore Download Defaults")
                .on_click(cx.listener(|shell, _, _, cx| {
                    shell.reset_active_profile_download_settings(cx);
                })),
        )
        .into_any_element()
}

fn render_download_policy_rows(
    active_policy: &DownloadPolicy,
    options: &[DownloadPolicyOption],
    cx: &mut Context<ElyShell>,
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
            options.iter().enumerate().map(|(index, option)| {
                render_download_policy_row(index, option, active_policy, cx)
            }),
        )
        .into_any_element()
}

fn render_download_policy_row(
    index: usize,
    option: &DownloadPolicyOption,
    active_policy: &DownloadPolicy,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let selected = option.policy == *active_policy;
    let policy = option.policy.clone();

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
                        .text_color(rgb(download_policy_icon_color(selected)))
                        .child(download_policy_icon(option, selected)),
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
                                .child(option.title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::muted()))
                                .child(option.detail.clone()),
                        ),
                ),
        )
        .child(
            Button::new(("download-policy", index))
                .ghost()
                .xsmall()
                .selected(selected)
                .label(download_policy_button_label(selected))
                .tooltip(option.title)
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.set_active_profile_download_policy(policy.clone(), cx);
                })),
        )
        .into_any_element()
}

fn download_policy_options() -> Vec<DownloadPolicyOption> {
    let mut options = vec![DownloadPolicyOption {
        icon: IconName::Info,
        title: "Ask Every Time",
        detail: "Choose a location for each download.".to_string(),
        policy: DownloadPolicy::ask_every_time(),
    }];

    if let Some(path) = user_downloads_dir()
        && let Ok(policy) = DownloadPolicy::fixed_directory(path.clone())
    {
        options.push(DownloadPolicyOption {
            icon: IconName::Folder,
            title: "Downloads Folder",
            detail: path.display().to_string(),
            policy,
        });
    }

    options
}

fn user_downloads_dir() -> Option<PathBuf> {
    UserDirs::new().and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
}

fn download_policy_icon(option: &DownloadPolicyOption, selected: bool) -> IconName {
    if selected { IconName::CircleCheck } else { option.icon.clone() }
}

fn download_policy_icon_color(selected: bool) -> u32 {
    if selected { colors::primary() } else { colors::muted_soft() }
}

fn download_policy_button_label(selected: bool) -> &'static str {
    if selected { "Active" } else { "Select" }
}
