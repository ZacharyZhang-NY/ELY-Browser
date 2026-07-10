use std::{collections::VecDeque, io::Cursor};

use super::take_permission_batch;
use crate::{
    live_protocol::LiveRequest,
    live_request::{MAX_REQUEST_LINE_BYTES, RequestLineRead, read_request_line},
};

#[test]
fn nine_permission_consumptions_are_sent_in_order_across_two_responses() {
    let mut pending = (1_u8..=9).collect::<VecDeque<_>>();

    assert_eq!(take_permission_batch(&mut pending), (1_u8..=8).collect::<Vec<_>>());
    assert_eq!(take_permission_batch(&mut pending), vec![9]);
}

#[test]
fn request_line_accepts_the_exact_byte_limit() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Cursor::new(padded_handshake_line(MAX_REQUEST_LINE_BYTES));
    let mut line = Vec::new();

    assert_eq!(read_request_line(&mut input, &mut line)?, RequestLineRead::Ready);
    assert_eq!(line.len(), MAX_REQUEST_LINE_BYTES);
    assert!(matches!(serde_json::from_slice::<LiveRequest>(&line)?, LiveRequest::Handshake { .. }));
    Ok(())
}

#[test]
fn request_line_rejects_limit_plus_one_and_preserves_the_next_frame()
-> Result<(), Box<dyn std::error::Error>> {
    let mut wire = padded_handshake_line(MAX_REQUEST_LINE_BYTES + 1);
    wire.extend_from_slice(br#"{"type":"shutdown"}"#);
    wire.push(b'\n');
    let mut input = Cursor::new(wire);
    let mut line = Vec::new();

    assert_eq!(read_request_line(&mut input, &mut line)?, RequestLineRead::TooLarge);
    assert_eq!(read_request_line(&mut input, &mut line)?, RequestLineRead::Ready);
    assert!(matches!(serde_json::from_slice::<LiveRequest>(&line)?, LiveRequest::Shutdown));
    Ok(())
}

#[test]
fn request_line_drains_a_distant_delimiter_before_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    let mut wire = padded_handshake_line(MAX_REQUEST_LINE_BYTES + 4_096);
    wire.extend_from_slice(br#"{"type":"shutdown"}"#);
    wire.push(b'\n');
    let mut input = Cursor::new(wire);
    let mut line = Vec::new();

    assert_eq!(read_request_line(&mut input, &mut line)?, RequestLineRead::TooLarge);
    assert_eq!(read_request_line(&mut input, &mut line)?, RequestLineRead::Ready);
    assert!(matches!(serde_json::from_slice::<LiveRequest>(&line)?, LiveRequest::Shutdown));
    Ok(())
}

#[test]
fn request_line_accepts_a_bounded_eof_terminated_frame() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Cursor::new(br#"{"type":"shutdown"}"#.to_vec());
    let mut line = Vec::new();

    assert_eq!(read_request_line(&mut input, &mut line)?, RequestLineRead::Ready);
    assert!(matches!(serde_json::from_slice::<LiveRequest>(&line)?, LiveRequest::Shutdown));
    assert_eq!(read_request_line(&mut input, &mut line)?, RequestLineRead::Eof);
    Ok(())
}

fn padded_handshake_line(bytes: usize) -> Vec<u8> {
    let mut line = br#"{"type":"handshake","protocol_version":3}"#.to_vec();
    assert!(bytes > line.len());
    line.resize(bytes - 1, b' ');
    line.push(b'\n');
    line
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
