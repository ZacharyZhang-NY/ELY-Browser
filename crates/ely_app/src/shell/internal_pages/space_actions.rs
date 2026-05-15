use ely_design_system::colors;
use ely_domain::SpaceId;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, rgb};
use gpui_component::{
    Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};

use super::super::ElyShell;

pub(super) fn render_space_actions(
    index: usize,
    space_count: usize,
    space_id: SpaceId,
    active: bool,
    confirming_trash: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if confirming_trash {
        return render_trash_confirmation(cx);
    }

    let can_move_up = index > 0;
    let can_move_down = index + 1 < space_count;

    div()
        .flex()
        .items_center()
        .gap_2()
        .child(render_space_order_button(
            ("move-space-up", index),
            space_id.clone(),
            IconName::ArrowUp,
            "Move Space Up",
            can_move_up,
            true,
            cx,
        ))
        .child(render_space_order_button(
            ("move-space-down", index),
            space_id.clone(),
            IconName::ArrowDown,
            "Move Space Down",
            can_move_down,
            false,
            cx,
        ))
        .child(render_space_export_button(index, space_id.clone(), cx))
        .child(render_space_switch_action(index, space_id.clone(), active, cx))
        .child(render_request_trash_button(index, space_id, space_count, cx))
        .into_any_element()
}

fn render_space_order_button(
    id: (&'static str, usize),
    space_id: SpaceId,
    icon: IconName,
    tooltip: &'static str,
    enabled: bool,
    moves_up: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    Button::new(id)
        .small()
        .ghost()
        .icon(icon)
        .tooltip(tooltip)
        .disabled(!enabled)
        .on_click(cx.listener(move |shell, _, _, cx| {
            if moves_up {
                shell.move_space_up(&space_id, cx);
            } else {
                shell.move_space_down(&space_id, cx);
            }
        }))
        .into_any_element()
}

fn render_space_export_button(
    index: usize,
    space_id: SpaceId,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    Button::new(("export-space", index))
        .small()
        .ghost()
        .icon(IconName::File)
        .label("Export")
        .tooltip("Export .elyspace")
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.export_space(&space_id, window, cx);
        }))
        .into_any_element()
}

fn render_request_trash_button(
    index: usize,
    space_id: SpaceId,
    space_count: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    Button::new(("trash-space", index))
        .small()
        .ghost()
        .icon(IconName::Delete)
        .tooltip("Move Space to Trash")
        .disabled(space_count <= 1)
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.request_space_trash(space_id.clone(), cx);
        }))
        .into_any_element()
}

fn render_trash_confirmation(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(Button::new("cancel-space-trash").small().ghost().label("Cancel").on_click(
            cx.listener(|shell, _, _, cx| {
                shell.cancel_space_trash(cx);
            }),
        ))
        .child(
            Button::new("confirm-space-trash")
                .small()
                .danger()
                .icon(IconName::Delete)
                .label("Trash")
                .tooltip("Move Space to Trash")
                .on_click(cx.listener(|shell, _, window, cx| {
                    shell.trash_pending_space(window, cx);
                })),
        )
        .into_any_element()
}

fn render_space_switch_action(
    index: usize,
    space_id: SpaceId,
    active: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if active {
        return div()
            .text_xs()
            .font_semibold()
            .text_color(rgb(colors::success()))
            .child("Active")
            .into_any_element();
    }

    Button::new(("switch-space", index))
        .small()
        .primary()
        .icon(IconName::Check)
        .label("Switch")
        .tooltip("Switch Space")
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_space(&space_id, window, cx);
        }))
        .into_any_element()
}
