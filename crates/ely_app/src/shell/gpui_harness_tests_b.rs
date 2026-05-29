use super::*;

/// Bisect probe layer 2: stack the full ElyShell wrapper chain that
/// sits between the window root and the overlay — root size_full,
/// absolute inset_0 flex container, flex_1 + flex_col main pane with
/// rounded/border/shadow/overflow_hidden, flex_1 content wrapper,
/// then the relative+overflow_hidden surface wrapper from
/// `render_web_surface`. If this passes, the bug is in something
/// `render_external_web_canvas` adds (not in the plain layout chain).
#[gpui::test]
async fn baseline_overlay_under_full_elyshell_wrapper_chain_receives_click(
    cx: &mut TestAppContext,
) {
    let click_count = Rc::new(RefCell::new(0u32));
    let counter_for_render = click_count.clone();

    struct DeepProbe {
        on_up_counter: Rc<RefCell<u32>>,
    }
    impl Render for DeepProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let counter = self.on_up_counter.clone();
            // Root: matches render_browser's outer div.
            div().size_full().child(
                // Absolute flex container matches render_browser's child layout.
                div().absolute().inset_0().p(px(16.0)).gap(px(12.0)).flex().child(
                    // Main pane: matches render_main_pane.
                    div()
                        .flex_1()
                        .h_full()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .rounded(px(18.0))
                        .border_1()
                        .overflow_hidden()
                        .child(
                            // Content wrapper: matches the flex_1 child of main_pane.
                            div().flex_1().overflow_hidden().child(
                                // Surface wrapper: matches render_web_surface root.
                                div().relative().size_full().min_w_0().overflow_hidden().child(
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
                                ),
                            ),
                        ),
                ),
            )
        }
    }

    let (_probe, cx) =
        cx.add_window_view(|_window, _cx| DeepProbe { on_up_counter: counter_for_render });
    cx.run_until_parked();

    cx.simulate_mouse_move(point(px(400.0), px(400.0)), None, Modifiers::default());
    cx.simulate_click(point(px(400.0), px(400.0)), Modifiers::default());
    cx.run_until_parked();

    assert_eq!(
        *click_count.borrow(),
        1,
        "Bisect layer 2: the full root → absolute-flex → main-pane → \
         content-wrapper → surface-wrapper chain (sans listeners) \
         should still deliver the click. If this fails, the bug is in \
         this wrapper chain itself; if it passes, the bug is in \
         something render_external_web_canvas adds (canvas tracker, \
         absolute content wrapper sibling, or a gpui-component widget)."
    );
}

/// Diagnostic probes (T13 bisect): each layer of the ElyShell render
/// tree was reproduced in isolation and **all passed**, confirming the
/// listener combo, layout wrapper chain, canvas sibling, entity
/// update side effects, and track_focus root listeners are NOT the
/// cause. The bug is elsewhere — likely in ElyShell::new's setup
/// (subscriptions, timer, InputState side effects) or in the
/// sync_address_input call inside navigate_active_tab that mutates
/// Input widget state which may invalidate the rendered_frame
/// between MouseMove and MouseUp. Kept for regression coverage.
/// T13 layer 6: replicate ElyShell::new's gpui-component widget
/// construction (InputState creation + subscription) on top of the
/// passing layout, then click. If this fails, the InputState entity
/// or its subscription is what breaks hit_test for descendant
/// occlude divs.
#[gpui::test]
async fn baseline_overlay_with_input_state_construction_receives_click(cx: &mut TestAppContext) {
    use gpui::AppContext;
    use gpui::Entity;
    use gpui::Subscription;
    use gpui_component::input::{InputEvent, InputState};

    cx.update(gpui_component::init);

    let click_count = Rc::new(RefCell::new(0u32));
    let counter_for_render = click_count.clone();

    struct ProbeWithInput {
        on_up_counter: Rc<RefCell<u32>>,
        _command_input: Entity<InputState>,
        _command_subscription: Subscription,
    }
    impl Render for ProbeWithInput {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let counter = self.on_up_counter.clone();
            div().relative().size_full().min_w_0().overflow_hidden().child(
                div()
                    .absolute()
                    .size_full()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_e, _w, _c| {})
                    .capture_any_mouse_up(move |_e, _w, _c| {
                        *counter.borrow_mut() += 1;
                    })
                    .on_mouse_move(|_e, _w, _c| {})
                    .on_scroll_wheel(|_e, _w, _c| {}),
            )
        }
    }

    let (_probe, cx) = cx.add_window_view(|window, cx| {
        let command_input = cx.new(|cx| InputState::new(window, cx).placeholder("test"));
        let command_subscription = cx.subscribe_in(
            &command_input,
            window,
            |_probe: &mut ProbeWithInput, _input, _event: &InputEvent, _window, _cx| {
                // mimic the shape ElyShell::new uses
            },
        );
        ProbeWithInput {
            on_up_counter: counter_for_render,
            _command_input: command_input,
            _command_subscription: command_subscription,
        }
    });
    cx.run_until_parked();

    cx.simulate_mouse_move(point(px(400.0), px(400.0)), None, Modifiers::default());
    cx.simulate_click(point(px(400.0), px(400.0)), Modifiers::default());
    cx.run_until_parked();

    assert_eq!(
        *click_count.borrow(),
        1,
        "Bisect layer 6: an InputState entity + a subscribe_in to it \
         must not break click delivery to a sibling occlude div. If \
         this fails, the InputState construction (which spawns \
         BlinkCursor, registers window-activation/focus/blur \
         observers) corrupts hit_test for the rest of the tree."
    );
}

/// T13 next-layer diagnostic: same shape as the passing baseline
/// (`.relative().size_full().min_w_0().overflow_hidden()` parent +
/// the input_overlay listener combo) but with `gpui_component::init`
/// called first. The real ElyShell test calls init; the probes
/// don't. If this fails, `gpui_component::init`'s side effect on
/// the App is what breaks the rendered_frame's hitbox registration
/// for descendant occlude divs.
#[gpui::test]
async fn baseline_overlay_after_gpui_component_init_receives_click(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);

    let click_count = Rc::new(RefCell::new(0u32));
    let counter_for_render = click_count.clone();

    struct AfterInitProbe {
        on_up_counter: Rc<RefCell<u32>>,
    }
    impl Render for AfterInitProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let counter = self.on_up_counter.clone();
            div().relative().size_full().min_w_0().overflow_hidden().child(
                div()
                    .absolute()
                    .size_full()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_e, _w, _c| {})
                    .capture_any_mouse_up(move |_e, _w, _c| {
                        *counter.borrow_mut() += 1;
                    })
                    .on_mouse_move(|_e, _w, _c| {})
                    .on_scroll_wheel(|_e, _w, _c| {}),
            )
        }
    }

    let (_probe, cx) =
        cx.add_window_view(|_window, _cx| AfterInitProbe { on_up_counter: counter_for_render });
    cx.run_until_parked();

    cx.simulate_mouse_move(point(px(400.0), px(400.0)), None, Modifiers::default());
    cx.simulate_click(point(px(400.0), px(400.0)), Modifiers::default());
    cx.run_until_parked();

    assert_eq!(
        *click_count.borrow(),
        1,
        "Bisect: gpui_component::init must not break click delivery to \
         an occlude div with the input_overlay listener combo. If this \
         fails, init registers some App-level state that interferes \
         with rendered_frame hitbox registration for descendants."
    );
}

/// Bisect probe layer 5: the full chain WITH the root-level
/// `track_focus + on_mouse_up(Left, bubble)` listeners ElyShell
/// puts on its outermost div. If track_focus's auto-focus MouseDown
/// handler, or the root's bubble-phase MouseUp listener, somehow
/// invalidates the rendered_frame between MouseDown and MouseUp,
/// input_overlay's hitbox will no longer match the listener
/// snapshot's id and `is_hovered` will return false. That's the
/// exact symptom we see: hover_point is None too, so it's not
/// capture-specific — every listener on input_overlay is missing
/// its hit.
#[gpui::test]
async fn baseline_overlay_under_root_with_track_focus_receives_click(cx: &mut TestAppContext) {
    use gpui::FocusHandle;

    let click_count = Rc::new(RefCell::new(0u32));
    let move_count = Rc::new(RefCell::new(0u32));
    let counter_for_up = click_count.clone();
    let counter_for_move = move_count.clone();

    struct TrackFocusProbe {
        focus: FocusHandle,
        on_up_counter: Rc<RefCell<u32>>,
        on_move_counter: Rc<RefCell<u32>>,
    }
    impl Render for TrackFocusProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let counter_up = self.on_up_counter.clone();
            let counter_move = self.on_move_counter.clone();
            div()
                .size_full()
                .track_focus(&self.focus)
                .on_mouse_up(MouseButton::Left, |_event, _window, _cx| {})
                .child(
                    div().absolute().inset_0().p(px(16.0)).gap(px(12.0)).flex().child(
                        div()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .rounded(px(18.0))
                            .border_1()
                            .overflow_hidden()
                            .child(
                                div().flex_1().overflow_hidden().child(
                                    div()
                                        .relative()
                                        .size_full()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .child(div().absolute().inset_0().child(div().size_full()))
                                        .child(
                                            canvas(move |_b, _w, _c| {}, |_, _, _, _| {})
                                                .absolute()
                                                .size_full(),
                                        )
                                        .child(
                                            div()
                                                .absolute()
                                                .size_full()
                                                .occlude()
                                                .on_mouse_down(MouseButton::Left, |_e, _w, _c| {})
                                                .capture_any_mouse_up(move |_e, _w, _c| {
                                                    *counter_up.borrow_mut() += 1;
                                                })
                                                .on_mouse_move(move |_e, _w, _c| {
                                                    *counter_move.borrow_mut() += 1;
                                                })
                                                .on_scroll_wheel(|_e, _w, _c| {}),
                                        ),
                                ),
                            ),
                    ),
                )
        }
    }

    let (_probe, cx) = cx.add_window_view(|_window, cx| TrackFocusProbe {
        focus: cx.focus_handle(),
        on_up_counter: counter_for_up,
        on_move_counter: counter_for_move,
    });
    cx.run_until_parked();

    cx.simulate_mouse_move(point(px(400.0), px(400.0)), None, Modifiers::default());
    cx.run_until_parked();
    cx.simulate_click(point(px(400.0), px(400.0)), Modifiers::default());
    cx.run_until_parked();

    assert!(
        *move_count.borrow() > 0,
        "Bisect layer 5: input_overlay's on_mouse_move never fired even \
         though the cursor was simulated over it. The same `is_hovered` \
         check fails for every listener, exactly mirroring the T7 red \
         test's observation that hover_point is None."
    );
    assert_eq!(
        *click_count.borrow(),
        1,
        "Bisect layer 5: capture_any_mouse_up did not fire under the \
         full track_focus + bubble-mouse_up root chain. This isolates \
         the culprit to the root-level listeners that ElyShell adds \
         around its widget tree."
    );
}
