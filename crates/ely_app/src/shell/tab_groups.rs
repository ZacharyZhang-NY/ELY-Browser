use std::collections::BTreeSet;

use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{BrowserTab, SplitId, SplitLayout, TabGroup, TabGroupId};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, rgb,
};
use gpui_component::{IconName, StyledExt};

use super::{ElyShell, ShellState};

impl ElyShell {
    pub(super) fn render_sidebar_tab_rows(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        let mut row_state = SidebarRowState::new(snapshot);

        for tab in snapshot.tabs.iter().filter(|tab| sidebar_tab_is_visible(tab)) {
            let Some(group_id) = tab.group_id() else {
                self.push_sidebar_tab_row(&mut rows, snapshot, tab, &mut row_state, cx);
                continue;
            };

            let Some(group) = snapshot.tab_groups.iter().find(|group| group.id() == group_id)
            else {
                self.push_sidebar_tab_row(&mut rows, snapshot, tab, &mut row_state, cx);
                continue;
            };

            if row_state.rendered_group_ids.insert(group_id.clone()) {
                self.push_tab_group_rows(&mut rows, snapshot, group, &mut row_state, cx);
            }
        }

        rows
    }

    pub(super) fn toggle_tab_group(&mut self, group_id: &TabGroupId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.toggle_tab_group_collapsed(group_id).is_ok()
        {
            cx.notify();
        }
    }

    fn push_tab_group_rows(
        &mut self,
        rows: &mut Vec<AnyElement>,
        snapshot: &BrowserSnapshot,
        group: &TabGroup,
        row_state: &mut SidebarRowState,
        cx: &mut Context<Self>,
    ) {
        let group_tabs = group_tabs(snapshot, group);
        if group_tabs.is_empty() {
            return;
        }

        rows.push(self.render_tab_group_row(
            group,
            group_tabs.len(),
            row_state.active_group_id.as_ref() == Some(group.id()),
            cx,
        ));

        if group.collapsed() {
            return;
        }

        for tab in group_tabs {
            self.push_sidebar_tab_row(rows, snapshot, tab, row_state, cx);
        }
    }

    fn push_sidebar_tab_row(
        &mut self,
        rows: &mut Vec<AnyElement>,
        snapshot: &BrowserSnapshot,
        tab: &BrowserTab,
        row_state: &mut SidebarRowState,
        cx: &mut Context<Self>,
    ) {
        if let Some(split_id) = tab.split_id()
            && let Some(layout) = saved_split_layout(snapshot, split_id)
        {
            if row_state.rendered_split_ids.insert(split_id.clone()) {
                let active = row_state.active_split_id.as_ref() == Some(split_id);
                if let Some(row) = self.render_saved_split_row(layout, active, cx) {
                    rows.push(row);
                }
            }
            return;
        }

        rows.push(self.render_tab_row(tab, tab.id() == &snapshot.active_tab_id, cx));
    }

    fn render_tab_group_row(
        &mut self,
        group: &TabGroup,
        tab_count: usize,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let group_id = group.id().clone();
        let background = if active { colors::SURFACE_CARD } else { colors::CANVAS };
        let border = if active { colors::PRIMARY } else { colors::HAIRLINE };
        let icon = if group.collapsed() { IconName::Folder } else { IconName::FolderOpen };
        let state_label = if group.collapsed() { "Collapsed" } else { "Expanded" };

        div()
            .id(SharedString::from(format!("tab-group-{}", group.id().as_str())))
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(background))
            .px_3()
            .py_2()
            .gap_2()
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::SURFACE_CARD)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.toggle_tab_group(&group_id, cx);
            }))
            .child(div().text_color(rgb(group.color_hex())).child(icon))
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
                            .child(group.name().to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(colors::MUTED))
                            .child(format!("{tab_count} tabs - {state_label}")),
                    ),
            )
            .into_any_element()
    }
}

struct SidebarRowState {
    active_group_id: Option<TabGroupId>,
    active_split_id: Option<SplitId>,
    rendered_group_ids: BTreeSet<TabGroupId>,
    rendered_split_ids: BTreeSet<SplitId>,
}

impl SidebarRowState {
    fn new(snapshot: &BrowserSnapshot) -> Self {
        Self {
            active_group_id: active_group_id(snapshot),
            active_split_id: active_split_id(snapshot),
            rendered_group_ids: BTreeSet::new(),
            rendered_split_ids: BTreeSet::new(),
        }
    }
}

fn active_group_id(snapshot: &BrowserSnapshot) -> Option<TabGroupId> {
    snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &snapshot.active_tab_id)
        .and_then(|tab| tab.group_id().cloned())
}

fn active_split_id(snapshot: &BrowserSnapshot) -> Option<SplitId> {
    snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &snapshot.active_tab_id)
        .and_then(|tab| tab.split_id().cloned())
}

fn saved_split_layout<'a>(
    snapshot: &'a BrowserSnapshot,
    split_id: &SplitId,
) -> Option<&'a SplitLayout> {
    snapshot.split_layouts.iter().find(|layout| layout.id() == split_id && layout.saved())
}

fn group_tabs<'a>(snapshot: &'a BrowserSnapshot, group: &TabGroup) -> Vec<&'a BrowserTab> {
    snapshot
        .tabs
        .iter()
        .filter(|tab| sidebar_tab_is_visible(tab))
        .filter(|tab| tab.group_id() == Some(group.id()))
        .collect()
}

fn sidebar_tab_is_visible(tab: &BrowserTab) -> bool {
    !tab.flags().favorite && !tab.flags().pinned
}
