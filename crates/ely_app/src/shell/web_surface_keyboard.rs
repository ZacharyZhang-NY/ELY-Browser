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
        let Some((tab_id, requested_url)) = self.active_external_tab_target() else {
            return;
        };

        if let Some(text) = printable_text_from_key_down(event) {
            if self.type_text_in_external_web_viewport(tab_id, requested_url, text, cx) {
                cx.stop_propagation();
            }
            return;
        }

        if let Some(text) = special_key_text(event) {
            if self.type_text_in_external_web_viewport(tab_id, requested_url, text, cx) {
                cx.stop_propagation();
            }
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

fn printable_text_from_key_down(event: &KeyDownEvent) -> Option<&str> {
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

fn special_key_text(event: &KeyDownEvent) -> Option<&'static str> {
    if event.keystroke.modifiers.platform || event.keystroke.modifiers.control {
        return None;
    }
    match event.keystroke.key.as_str() {
        "enter" => Some("\n"),
        "backspace" => Some("\x08"),
        "tab" => Some("\t"),
        "escape" => Some("\x1b"),
        "delete" => Some("\x7f"),
        "left" => Some("\u{F702}"),
        "right" => Some("\u{F703}"),
        "up" => Some("\u{F700}"),
        "down" => Some("\u{F701}"),
        "home" => Some("\u{F729}"),
        "end" => Some("\u{F72B}"),
        "pageup" => Some("\u{F72C}"),
        "pagedown" => Some("\u{F72D}"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use gpui::{KeyDownEvent, Keystroke, Modifiers};

    use super::{printable_text_from_key_down, special_key_text};

    #[test]
    fn typed_text_uses_printable_key_char() {
        let event = key_down("e", Some("e"), Modifiers::none());

        assert_eq!(printable_text_from_key_down(&event), Some("e"));
    }

    #[test]
    fn typed_text_keeps_shifted_characters() {
        let mut modifiers = Modifiers::none();
        modifiers.shift = true;
        let event = key_down("1", Some("!"), modifiers);

        assert_eq!(printable_text_from_key_down(&event), Some("!"));
    }

    #[test]
    fn typed_text_ignores_browser_shortcuts() {
        let mut modifiers = Modifiers::none();
        modifiers.platform = true;
        let event = key_down("l", None, modifiers);

        assert_eq!(printable_text_from_key_down(&event), None);
    }

    #[test]
    fn special_key_enter() {
        let event = key_down("enter", None, Modifiers::none());
        assert_eq!(special_key_text(&event), Some("\n"));
    }

    #[test]
    fn special_key_backspace() {
        let event = key_down("backspace", None, Modifiers::none());
        assert_eq!(special_key_text(&event), Some("\x08"));
    }

    #[test]
    fn special_key_ignored_with_platform_modifier() {
        let mut modifiers = Modifiers::none();
        modifiers.platform = true;
        let event = key_down("enter", None, modifiers);
        assert_eq!(special_key_text(&event), None);
    }

    #[test]
    fn special_key_arrows() {
        let event = key_down("left", None, Modifiers::none());
        assert_eq!(special_key_text(&event), Some("\u{F702}"));

        let event = key_down("right", None, Modifiers::none());
        assert_eq!(special_key_text(&event), Some("\u{F703}"));
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
