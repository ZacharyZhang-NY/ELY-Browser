use ely_browser_core::BrowserSnapshot;
use gpui::{AnyElement, Context};

use super::ElyShell;
use crate::shell::chrome::render_appearance_form;

impl ElyShell {
    pub(super) fn render_appearance_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_appearance_form(self, snapshot, cx)
    }
}
