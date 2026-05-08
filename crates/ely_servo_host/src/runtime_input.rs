use servo::{
    DevicePoint, InputEvent, Key, KeyState, KeyboardEvent, Location, Modifiers, MouseButton,
    MouseButtonAction, MouseButtonEvent, MouseMoveEvent, TouchEvent, TouchEventType, TouchId,
    WebView, WebViewPoint,
};

use crate::keyboard::keyboard_code_for_character;

pub(super) fn send_mouse_click(webview: &WebView, x: u32, y: u32) {
    let point = point(x, y);
    webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(point)));
    send_mouse_button(webview, MouseButtonAction::Down, point);
    send_mouse_button(webview, MouseButtonAction::Up, point);
}

pub(super) fn send_mouse_drag(webview: &WebView, from_x: u32, from_y: u32, to_x: u32, to_y: u32) {
    let from = point(from_x, from_y);
    let to = point(to_x, to_y);
    webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(from)));
    send_mouse_button(webview, MouseButtonAction::Down, from);
    webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(to)));
    send_mouse_button(webview, MouseButtonAction::Up, to);
}

pub(super) fn send_touch_tap(webview: &WebView, x: u32, y: u32) {
    let point = point(x, y);
    let touch_id = TouchId(1);
    for event_type in [TouchEventType::Down, TouchEventType::Up] {
        webview.notify_input_event(InputEvent::Touch(TouchEvent::new(event_type, touch_id, point)));
    }
}

pub(super) fn send_keyboard_text(webview: &WebView, text: &str) {
    for character in text.chars() {
        let key = Key::Character(character.to_string());
        let code = keyboard_code_for_character(character);
        webview.notify_input_event(InputEvent::Keyboard(KeyboardEvent::new_without_event(
            KeyState::Down,
            key.clone(),
            code,
            Location::Standard,
            Modifiers::empty(),
            false,
            false,
        )));
        webview.notify_input_event(InputEvent::Keyboard(KeyboardEvent::new_without_event(
            KeyState::Up,
            key,
            code,
            Location::Standard,
            Modifiers::empty(),
            false,
            false,
        )));
    }
}

fn send_mouse_button(webview: &WebView, action: MouseButtonAction, point: WebViewPoint) {
    webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
        action,
        MouseButton::Left,
        point,
    )));
}

fn point(x: u32, y: u32) -> WebViewPoint {
    WebViewPoint::Device(DevicePoint::new(x as f32, y as f32))
}
