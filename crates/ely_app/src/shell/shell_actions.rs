use gpui::{Context, Window};

use crate::{
    CloseCurrentTab, DownloadCurrentPage, FocusAddressBar, FocusCommandMode, OpenDownloads,
    OpenHistory, OpenNewTab, OpenSettings, OpenTaskManager, ResetZoom, RestoreClosedTab,
    SelectNextTab, SelectPreviousTab, ToggleFavoriteTab, TogglePinnedTab, ZoomIn, ZoomOut,
};

use super::ElyShell;

impl ElyShell {
    pub(super) fn on_close_current_tab(
        &mut self,
        _: &CloseCurrentTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_active_tab(window, cx);
    }

    pub(super) fn on_focus_address_bar(
        &mut self,
        _: &FocusAddressBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_address_bar(window, cx);
    }

    pub(super) fn on_focus_command_mode(
        &mut self,
        _: &FocusCommandMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_command_mode(window, cx);
    }

    pub(super) fn on_open_new_tab(
        &mut self,
        _: &OpenNewTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_new_tab(window, cx);
    }

    pub(super) fn on_open_downloads(
        &mut self,
        _: &OpenDownloads,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_downloads(window, cx);
    }

    pub(super) fn on_download_current_page(
        &mut self,
        _: &DownloadCurrentPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.download_active_tab(window, cx);
    }

    pub(super) fn on_open_history(
        &mut self,
        _: &OpenHistory,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_history(window, cx);
    }

    pub(super) fn on_open_settings(
        &mut self,
        _: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings(window, cx);
    }

    pub(super) fn on_open_task_manager(
        &mut self,
        _: &OpenTaskManager,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_task_manager(window, cx);
    }

    pub(super) fn on_restore_closed_tab(
        &mut self,
        _: &RestoreClosedTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.restore_closed_tab(window, cx);
    }

    pub(super) fn on_reset_zoom(&mut self, _: &ResetZoom, _: &mut Window, cx: &mut Context<Self>) {
        self.reset_active_tab_zoom(cx);
    }

    pub(super) fn on_select_next_tab(
        &mut self,
        _: &SelectNextTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_next_tab(window, cx);
    }

    pub(super) fn on_select_previous_tab(
        &mut self,
        _: &SelectPreviousTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_previous_tab(window, cx);
    }

    pub(super) fn on_toggle_favorite_tab(
        &mut self,
        _: &ToggleFavoriteTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_active_tab_favorite(cx);
    }

    pub(super) fn on_toggle_pinned_tab(
        &mut self,
        _: &TogglePinnedTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_active_tab_pinned(cx);
    }

    pub(super) fn on_zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom_active_tab_in(cx);
    }

    pub(super) fn on_zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom_active_tab_out(cx);
    }
}
