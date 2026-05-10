use ely_browser_core::BrowserSnapshot;
use gpui::{AnyElement, Context};

use super::ElyShell;
use crate::shell::chrome::{render_appearance_form, render_settings_shell};

impl ElyShell {
    pub(super) fn render_settings_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = render_appearance_form(self, snapshot, cx);
        render_settings_shell(snapshot, "ely://settings/appearance", content, cx)
    }
}
