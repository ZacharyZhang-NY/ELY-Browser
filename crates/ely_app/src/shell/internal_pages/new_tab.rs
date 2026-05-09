use ely_browser_core::BrowserSnapshot;
use gpui::{AnyElement, Context};

use super::ElyShell;
use crate::shell::chrome::render_home_page;

impl ElyShell {
    pub(super) fn render_new_tab_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_home_page(snapshot, cx)
    }
}
