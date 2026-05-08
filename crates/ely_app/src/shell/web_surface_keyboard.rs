use ely_domain::TabId;
use gpui::{Context, KeyDownEvent, Window};

use super::{ElyShell, ShellState, web_surface::is_external_web_url};

impl ElyShell {
    pub(super) fn on_external_web_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(text) = typed_text_from_key_down(event) else {
            return;
        };
        let Some((tab_id, requested_url)) = self.active_external_tab_target() else {
            return;
        };

        if self.type_text_in_external_web_viewport(tab_id, requested_url, text, cx) {
            cx.stop_propagation();
        }
    }

    fn active_external_tab_target(&self) -> Option<(TabId, String)> {
        let ShellState::Ready(core) = &self.state else {
            return None;
        };
        let tab = core.active_tab().ok()?;
        let requested_url = tab.url().as_str();
        if !is_external_web_url(requested_url) {
            return None;
        }

        Some((tab.id().clone(), requested_url.to_string()))
    }
}

fn typed_text_from_key_down(event: &KeyDownEvent) -> Option<&str> {
    let modifiers = &event.keystroke.modifiers;
    if modifiers.control || modifiers.platform || modifiers.function {
        return None;
    }

    let text = event.keystroke.key_char.as_deref()?;
    let mut chars = text.chars();
    let character = chars.next()?;
    if chars.next().is_some() || character.is_control() {
        return None;
    }

    Some(text)
}

#[cfg(test)]
mod tests {
    use gpui::{KeyDownEvent, Keystroke, Modifiers};

    use super::typed_text_from_key_down;

    #[test]
    fn typed_text_uses_printable_key_char() {
        let event = key_down("e", Some("e"), Modifiers::none());

        assert_eq!(typed_text_from_key_down(&event), Some("e"));
    }

    #[test]
    fn typed_text_keeps_shifted_characters() {
        let mut modifiers = Modifiers::none();
        modifiers.shift = true;
        let event = key_down("1", Some("!"), modifiers);

        assert_eq!(typed_text_from_key_down(&event), Some("!"));
    }

    #[test]
    fn typed_text_ignores_browser_shortcuts() {
        let mut modifiers = Modifiers::none();
        modifiers.platform = true;
        let event = key_down("l", None, modifiers);

        assert_eq!(typed_text_from_key_down(&event), None);
    }

    fn key_down(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> KeyDownEvent {
        KeyDownEvent {
            keystroke: Keystroke {
                modifiers,
                key: key.to_string(),
                key_char: key_char.map(str::to_string),
            },
            is_held: false,
        }
    }
}
