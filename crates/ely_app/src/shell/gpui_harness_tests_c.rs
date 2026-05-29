use super::*;

/// Bisect probe layer 4: same shape as the canvas-sibling probe but
/// the on_mouse_down listener calls `cx.update` on a self-entity
/// (mirroring `down_entity.update(cx, |shell, _| shell.focus_web_surface(window))`).
/// `entity.update` notifies subscribers; if that side effect during
/// mouse_down's bubble phase disturbs mouse dispatch — invalidates
/// the rendered_frame, regenerates hitboxes, or otherwise corrupts
/// the in-flight dispatch — the subsequent MouseUp will land on a
/// frame whose hitboxes no longer match the listeners' captured
/// snapshots, and this test will go red.
#[gpui::test]
async fn baseline_overlay_with_entity_update_in_mouse_down_receives_click(cx: &mut TestAppContext) {
    let click_count = Rc::new(RefCell::new(0u32));
    let counter_for_render = click_count.clone();

    struct EntityUpdateProbe {
        on_up_counter: Rc<RefCell<u32>>,
        tick: u32,
    }
    impl Render for EntityUpdateProbe {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let counter = self.on_up_counter.clone();
            let self_entity = cx.entity().clone();
            div()
                .relative()
                .size_full()
                .min_w_0()
                .overflow_hidden()
                .child(div().absolute().inset_0().child(div().size_full()))
                .child(
                    canvas(move |_bounds, _window, _cx| {}, |_, _, _, _| {}).absolute().size_full(),
                )
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .occlude()
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            self_entity.update(cx, |probe, _cx| {
                                probe.tick += 1;
                            });
                        })
                        .capture_any_mouse_up(move |_event, _window, _cx| {
                            *counter.borrow_mut() += 1;
                        })
                        .on_mouse_move(|_event, _window, _cx| {})
                        .on_scroll_wheel(|_event, _window, _cx| {}),
                )
        }
    }

    let (_probe, cx) = cx.add_window_view(|_window, _cx| EntityUpdateProbe {
        on_up_counter: counter_for_render,
        tick: 0,
    });
    cx.run_until_parked();

    cx.simulate_mouse_move(point(px(400.0), px(400.0)), None, Modifiers::default());
    cx.simulate_click(point(px(400.0), px(400.0)), Modifiers::default());
    cx.run_until_parked();

    assert_eq!(
        *click_count.borrow(),
        1,
        "Bisect layer 4: on_mouse_down's bubble fires entity.update \
         which automatically notifies subscribers. If this test fails, \
         the cx.notify side effect during in-flight dispatch corrupts \
         the rendered_frame for the immediately-following MouseUp."
    );
}

/// Bisect probe layer 3: add a canvas sibling BEFORE the overlay,
/// matching render_web_surface's viewport_tracker sibling exactly.
/// The canvas's prepaint callback fires during paint phase; if it
/// somehow disturbs hitbox registration or mouse_listeners ordering,
/// this test will go red and pinpoint the suspect.
#[gpui::test]
async fn baseline_overlay_with_canvas_sibling_receives_click(cx: &mut TestAppContext) {
    let click_count = Rc::new(RefCell::new(0u32));
    let counter_for_render = click_count.clone();

    struct CanvasSiblingProbe {
        on_up_counter: Rc<RefCell<u32>>,
    }
    impl Render for CanvasSiblingProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let counter = self.on_up_counter.clone();
            div()
                .relative()
                .size_full()
                .min_w_0()
                .overflow_hidden()
                .child(div().absolute().inset_0().child(div().size_full()))
                .child(
                    canvas(move |_bounds, _window, _cx| {}, |_, _, _, _| {}).absolute().size_full(),
                )
                .child(
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

    let (_probe, cx) =
        cx.add_window_view(|_window, _cx| CanvasSiblingProbe { on_up_counter: counter_for_render });
    cx.run_until_parked();

    cx.simulate_mouse_move(point(px(400.0), px(400.0)), None, Modifiers::default());
    cx.simulate_click(point(px(400.0), px(400.0)), Modifiers::default());
    cx.run_until_parked();

    assert_eq!(
        *click_count.borrow(),
        1,
        "Bisect layer 3: adding a canvas sibling (the shape \
         render_viewport_tracker uses) between the content wrapper and \
         the overlay should not break click delivery. If this fails, \
         the canvas element itself disturbs mouse dispatch — likely \
         via its prepaint callback's interaction with hit_test."
    );
}

#[gpui::test]
async fn baseline_overlay_with_full_listener_combo_receives_click(cx: &mut TestAppContext) {
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

    let (_probe, cx) =
        cx.add_window_view(|_window, _cx| ComboProbe { on_up_counter: counter_for_render });
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

    let (_probe, cx) =
        cx.add_window_view(|_window, _cx| Probe { on_up_counter: counter_for_render });
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

/// TDD red guard for T10: today every `WebSurfaceFrame::from_live_frame`
/// call allocates a fresh `Arc::new(RenderImage::new(...))` regardless
/// of whether the underlying pixels changed. At 60 fps on a 1080p
/// canvas that is `~8 MB / frame` of host-side RGBA cloning + a new
/// GPUI texture upload, the cost Linus + Karpathy + Jony all flagged
/// as the next material bottleneck after the file-system pipe.
///
/// The contract this test pins is the cheapest invariant we can hold
/// against today's `SoftwareRenderingContext`: two frames carrying
/// **byte-identical RGBA payloads must produce the same underlying
/// `Arc<RenderImage>`**. Today they do not — every `from_live_frame`
/// blindly reallocates. The fix path is either dedup the upload
/// against the last bytes or switch to direct platform-surface presentation.
///
/// Regression guard: with the single-slot `LAST_FRAME_IMAGE` cache in
/// `web_surface_frame.rs`, two `ServoLiveFrame` inputs carrying
/// byte-identical RGBA payloads now share the same `Arc<RenderImage>`.
/// Without this guard a regression that drops the cache silently
/// returns to ~960 MB/s of host-side RGBA cloning + per-frame GPUI
/// texture allocations.
#[test]
fn identical_live_frames_share_render_image_arc() -> Result<(), String> {
    let width = 16u32;
    let height = 8u32;
    let rgba_bytes = vec![0xAAu8; (width as usize) * (height as usize) * 4];

    let first = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test(width, height, rgba_bytes.clone()),
    )
    .map_err(|error| error.to_string())?;
    let second = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test(width, height, rgba_bytes),
    )
    .map_err(|error| error.to_string())?;

    let first_image = first
        .image
        .as_ref()
        .ok_or_else(|| "software path must produce an Arc<RenderImage>".to_string())?;
    let second_image = second
        .image
        .as_ref()
        .ok_or_else(|| "software path must produce an Arc<RenderImage>".to_string())?;
    assert!(
        Arc::ptr_eq(first_image, second_image),
        "TDD red: two ServoLiveFrames with byte-identical RGBA produced \
         distinct Arc<RenderImage> instances (first={:p}, second={:p}). \
         WebSurfaceFrame::from_parts must dedup the upload against the \
         previous frame's bytes, or the rendering pipeline must switch \
         to direct platform-surface presentation so per-frame host \
         allocations stop entirely.",
        Arc::as_ptr(first_image),
        Arc::as_ptr(second_image),
    );
    assert!(first.has_same_software_render_as(&second));
    Ok(())
}
