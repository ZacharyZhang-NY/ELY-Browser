use std::collections::VecDeque;

use super::take_permission_batch;

#[test]
fn nine_permission_consumptions_are_sent_in_order_across_two_responses() {
    let mut pending = (1_u8..=9).collect::<VecDeque<_>>();

    assert_eq!(take_permission_batch(&mut pending), (1_u8..=8).collect::<Vec<_>>());
    assert_eq!(take_permission_batch(&mut pending), vec![9]);
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
mod hardware {
    use super::super::{HardwarePollAction, hardware_poll_action};

    #[test]
    fn newly_ready_surface_replays_before_pending_frame() {
        assert_eq!(
            hardware_poll_action(true, false, true, false, true, true),
            HardwarePollAction::ReplaySurface
        );
        assert_eq!(
            hardware_poll_action(true, false, false, false, true, true),
            HardwarePollAction::PaintFrame
        );
    }

    #[test]
    fn awaiting_ready_surface_backpressures_pending_frame() {
        assert_eq!(
            hardware_poll_action(true, true, false, false, true, false),
            HardwarePollAction::Empty
        );
    }

    #[test]
    fn missing_surface_replays_before_first_ready() {
        assert_eq!(
            hardware_poll_action(true, false, false, true, true, false),
            HardwarePollAction::ReplaySurface
        );
    }
}
