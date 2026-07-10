use std::sync::Arc;

use super::resolve_render_image;

#[test]
fn cache_key_includes_frame_dimensions() -> Result<(), super::WebSurfaceError> {
    let bytes = vec![0, 0, 0, 255, 255, 255, 255, 255];
    let first = resolve_render_image(1, 2, bytes.clone(), 41)?;
    let reshaped = resolve_render_image(2, 1, bytes, 41)?;

    assert!(!Arc::ptr_eq(&first, &reshaped));
    Ok(())
}

#[test]
fn cache_verifies_bytes_after_hash_match() -> Result<(), super::WebSurfaceError> {
    let first = resolve_render_image(1, 1, vec![1, 2, 3, 255], 99)?;
    let collision = resolve_render_image(1, 1, vec![3, 2, 1, 255], 99)?;

    assert!(!Arc::ptr_eq(&first, &collision));
    Ok(())
}

#[test]
fn identical_frame_reuses_render_image() -> Result<(), super::WebSurfaceError> {
    let bytes = vec![1, 2, 3, 255];
    let first = resolve_render_image(1, 1, bytes.clone(), 7)?;
    let repeated = resolve_render_image(1, 1, bytes, 7)?;

    assert!(Arc::ptr_eq(&first, &repeated));
    Ok(())
}
