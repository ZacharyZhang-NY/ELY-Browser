use ely_browser_core::BrowserSnapshot;
use gpui::{AnyElement, Context};

use super::ElyShell;
use crate::shell::chrome::render_settings_landing;

impl ElyShell {
    pub(super) fn render_settings_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_settings_landing(snapshot, "ely://settings", cx)
    }
}
