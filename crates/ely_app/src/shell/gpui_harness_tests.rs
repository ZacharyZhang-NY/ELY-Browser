//! GPUI test harness for the input pipeline.
//!
//! Twelve sidecar-side commits and one shell-side commit had all claimed to
//! fix "click does nothing" while the user kept reporting the same symptom.
//! The roundtable consensus: every store-layer test passed GREEN, every
//! sidecar integration test passed GREEN, but nothing in the repo exercised
//! the real GPUI event tree (`render_input_overlay` + window-level mouse
//! handlers + sidebar capture interactions). This module is that missing
//! holdout set.
//!
//! What we have so far:
//!   1. `baseline_overlay_div_receives_simulated_click` — proves GPUI's
//!      `.occlude()` + `capture_any_mouse_up` primitive works correctly in
//!      `TestAppContext`. If this regresses, the harness itself is broken.
//!   2. `baseline_overlay_with_full_listener_combo_receives_click` — proves
//!      the exact listener combo `render_input_overlay` uses (on_mouse_down +
//!      capture_any_mouse_up + on_mouse_move + on_scroll_wheel on a single
//!      `.occlude()` div) works in isolation.
//!   3. `ely_shell_external_canvas_lays_out_inside_window` — boots a real
//!      `ElyShell`, navigates to an external URL, and asserts the
//!      input_overlay's measured viewport bounds fit inside the visible
//!      window. This was the original ship-blocker: the overlay was being
//!      positioned at `y = window_height - 17`, entirely below the visible
//!      region, so every user click hit empty space above the overlay.

use std::cell::RefCell;
use std::rc::Rc;

use ely_domain::{TabId, UrlText};
use gpui::{
    Bounds, Context, IntoElement, Modifiers, MouseButton, ParentElement, Pixels, Render,
    Styled, TestAppContext, Window, div, point, px,
};
use gpui::InteractiveElement;

use super::ShellState;
use super::web_surface::WebSurfaceStore;

#[cfg(test)]
impl super::ElyShell {
    pub(super) fn web_surfaces_for_test(&self) -> &WebSurfaceStore {
        &self.web_surfaces
    }
}

#[gpui::test]
async fn ely_shell_external_canvas_lays_out_inside_window(cx: &mut TestAppContext) {
    cx.update(|cx| gpui_component::init(cx));

    let (shell, cx) = cx.add_window_view(|window, cx| super::ElyShell::new(window, cx));
    cx.run_until_parked();

    cx.update(|window, app_cx| {
        shell.update(app_cx, |shell, ctx| {
            shell.navigate_active_tab(
                UrlText::parse("https://example.com/".to_string()).expect("valid URL"),
                window,
                ctx,
            );
        });
    });
    cx.run_until_parked();

    let (_active_tab_id, active_tab_url, viewport_bounds) = active_tab_overlay_state(&shell, cx);
    assert!(
        active_tab_url.starts_with("https://"),
        "active tab URL must be external https for render_external_web_canvas \
         to render the input_overlay (got {active_tab_url:?})."
    );
    let bounds = viewport_bounds.unwrap_or_else(|| {
        panic!(
            "viewport_bounds for the active tab is None. The canvas tracker \
             in render_input_overlay's sibling never fired its layout \
             callback — render_external_web_canvas was not reached."
        )
    });
    let window_size = cx.update(|window, _| window.bounds().size);
    assert!(
        bounds.origin.y + bounds.size.height <= window_size.height + px(1.0)
            && bounds.origin.x + bounds.size.width <= window_size.width + px(1.0),
        "Layout regression: input_overlay viewport_bounds {bounds:?} extend \
         outside the {window_size:?} window. The canvas tracker measured a \
         layout that escapes the visible region — every user click in the \
         visible area now lands above (or beside) the overlay's hitbox. \
         The original ship-blocker was bounds.origin.y == window_height-17 \
         caused by content (the rendered web image) being a non-absolute \
         child of the relative wrapper, which doubled the parent's height \
         and pushed the overlay off-screen."
    );
}

#[gpui::test]
async fn baseline_overlay_with_full_listener_combo_receives_click(
    cx: &mut TestAppContext,
) {
    let click_count = Rc::new(RefCell::new(0u32));
    let counter_for_render = click_count.clone();

    struct ComboProbe {
        on_up_counter: Rc<RefCell<u32>>,
    }
    impl Render for ComboProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let counter = self.on_up_counter.clone();
            div().relative().size_full().child(
                div()
                    .absolute()
                    .size_full()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_event, _window, _cx| {})
                    .capture_any_mouse_up(move |_event, _window, _cx| {
                        *counter.borrow_mut() += 1;
                    })
                    .on_mouse_move(|_event, _window, _cx| {})
                    .on_scroll_wheel(|_event, _window, _cx| {}),
            )
        }
    }

    let (_probe, cx) = cx.add_window_view(|_window, _cx| ComboProbe {
        on_up_counter: counter_for_render,
    });
    cx.run_until_parked();

    cx.simulate_mouse_move(point(px(100.0), px(100.0)), None, Modifiers::default());
    cx.simulate_click(point(px(100.0), px(100.0)), Modifiers::default());
    cx.run_until_parked();

    assert_eq!(
        *click_count.borrow(),
        1,
        "GPUI baseline with input_overlay's full listener combo: click should \
         reach capture_any_mouse_up. If this fails, the listener combo itself \
         is the problem, not the surrounding shell."
    );
}

#[gpui::test]
async fn baseline_overlay_div_receives_simulated_click(cx: &mut TestAppContext) {
    let click_count = Rc::new(RefCell::new(0u32));
    let counter_for_render = click_count.clone();

    struct Probe {
        on_up_counter: Rc<RefCell<u32>>,
    }
    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let counter = self.on_up_counter.clone();
            div().relative().size_full().child(
                div().absolute().size_full().occlude().capture_any_mouse_up(
                    move |_event, _window, _cx| {
                        *counter.borrow_mut() += 1;
                    },
                ),
            )
        }
    }

    let (_probe, cx) = cx.add_window_view(|_window, _cx| Probe {
        on_up_counter: counter_for_render,
    });
    cx.run_until_parked();

    cx.simulate_mouse_move(point(px(100.0), px(100.0)), None, Modifiers::default());
    cx.simulate_click(point(px(100.0), px(100.0)), Modifiers::default());
    cx.run_until_parked();

    assert_eq!(
        *click_count.borrow(),
        1,
        "GPUI baseline: a div with .absolute().size_full().occlude() and \
         capture_any_mouse_up never received the simulated click. The test \
         harness or GPUI primitive is broken — ElyShell test results are \
         meaningless until this passes."
    );
}

fn active_tab_overlay_state(
    shell: &gpui::Entity<super::ElyShell>,
    cx: &mut gpui::VisualTestContext,
) -> (TabId, String, Option<Bounds<Pixels>>) {
    shell.read_with(cx, |shell, _cx| {
        let tab = match &shell.state {
            ShellState::Ready(core) => core.active_tab().expect("active tab exists"),
            ShellState::StartupError(message) => {
                panic!("ElyShell failed to start in test: {message}")
            }
        };
        let tab_id = tab.id().clone();
        let url = tab.url().as_str().to_string();
        let bounds = shell
            .web_surfaces_for_test()
            .surface_for_test(&tab_id)
            .and_then(|surface| surface.viewport_bounds);
        (tab_id, url, bounds)
    })
}
