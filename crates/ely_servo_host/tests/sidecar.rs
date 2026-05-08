#![cfg(feature = "servo-engine")]

use std::error::Error;

#[path = "sidecar/support.rs"]
mod support;

use support::*;

#[test]
fn sidecar_opens_and_renders_prd_sites_to_rgba_files() -> Result<(), Box<dyn Error>> {
    for case in PRD_SITE_COMPATIBILITY_CASES {
        for size in PRD_SITE_COMPATIBILITY_SIZES {
            snapshot_prd_site(case, *size, ScrollOffset::ZERO)?;
        }
    }

    Ok(())
}

#[test]
fn sidecar_opens_and_renders_prd_reference_sites_to_rgba_files() -> Result<(), Box<dyn Error>> {
    for case in PRD_REFERENCE_SITE_COMPATIBILITY_CASES {
        snapshot_prd_site(case, PRD_REFERENCE_SITE_SIZE, ScrollOffset::ZERO)?;
    }

    Ok(())
}

#[test]
fn sidecar_scrolls_prd_site_with_servo_input() -> Result<(), Box<dyn Error>> {
    let scrolled_report =
        snapshot_prd_site(&SERVO_SCROLL_SITE, SERVO_SCROLL_SIZE, SERVO_SCROLL_OFFSET)?;

    assert_eq!(report_field_as_i64(&scrolled_report, "scroll_x")?, SERVO_SCROLL_OFFSET.x);
    assert_eq!(report_field_as_i64(&scrolled_report, "scroll_y")?, SERVO_SCROLL_OFFSET.y);
    assert_eq!(report_field_as_u64(&scrolled_report, "width")?, SERVO_SCROLL_SIZE.width);
    assert!(report_field_as_bool(&scrolled_report, "scroll_changed_frame")?);

    Ok(())
}

#[test]
fn sidecar_clicks_page_with_servo_mouse_input() -> Result<(), Box<dyn Error>> {
    let initial_report = snapshot_click_probe(None)?;
    let clicked_report = snapshot_click_probe(Some(SERVO_CLICK_POINT))?;

    assert_eq!(report_field_as_u64(&clicked_report, "click_x")?, SERVO_CLICK_POINT.x);
    assert_eq!(report_field_as_u64(&clicked_report, "click_y")?, SERVO_CLICK_POINT.y);
    assert!(report_field_as_bool(&clicked_report, "click_changed_frame")?);
    assert_ne!(
        report_field_as_u64(&initial_report, "sample_hash")?,
        report_field_as_u64(&clicked_report, "sample_hash")?
    );

    Ok(())
}

#[test]
fn sidecar_drags_page_with_servo_mouse_input() -> Result<(), Box<dyn Error>> {
    let initial_report = snapshot_drag_probe(None)?;
    let drag_points = DragPoints { from: SERVO_DRAG_FROM, to: SERVO_DRAG_TO };
    let dragged_report = snapshot_drag_probe(Some(drag_points))?;

    assert_eq!(report_field_as_u64(&dragged_report, "drag_from_x")?, SERVO_DRAG_FROM.x);
    assert_eq!(report_field_as_u64(&dragged_report, "drag_from_y")?, SERVO_DRAG_FROM.y);
    assert_eq!(report_field_as_u64(&dragged_report, "drag_to_x")?, SERVO_DRAG_TO.x);
    assert_eq!(report_field_as_u64(&dragged_report, "drag_to_y")?, SERVO_DRAG_TO.y);
    assert!(report_field_as_bool(&dragged_report, "drag_changed_frame")?);
    assert_ne!(
        report_field_as_u64(&initial_report, "sample_hash")?,
        report_field_as_u64(&dragged_report, "sample_hash")?
    );

    Ok(())
}

#[test]
fn sidecar_touches_page_with_servo_touch_input() -> Result<(), Box<dyn Error>> {
    let initial_report = snapshot_touch_probe(None)?;
    let touched_report = snapshot_touch_probe(Some(SERVO_TOUCH_POINT))?;

    assert_eq!(report_field_as_u64(&touched_report, "touch_x")?, SERVO_TOUCH_POINT.x);
    assert_eq!(report_field_as_u64(&touched_report, "touch_y")?, SERVO_TOUCH_POINT.y);
    assert!(report_field_as_bool(&touched_report, "touch_changed_frame")?);
    assert_ne!(
        report_field_as_u64(&initial_report, "sample_hash")?,
        report_field_as_u64(&touched_report, "sample_hash")?
    );

    Ok(())
}

#[test]
fn sidecar_types_text_with_servo_keyboard_input() -> Result<(), Box<dyn Error>> {
    let initial_report = snapshot_text_probe(None)?;
    let typed_report = snapshot_text_probe(Some(SERVO_TEXT_VALUE))?;

    assert_eq!(
        report_field_as_u64(&typed_report, "typed_text_byte_count")?,
        SERVO_TEXT_VALUE.len() as u64
    );
    assert!(report_field_as_bool(&typed_report, "text_changed_frame")?);
    assert_ne!(
        report_field_as_u64(&initial_report, "sample_hash")?,
        report_field_as_u64(&typed_report, "sample_hash")?
    );

    Ok(())
}
