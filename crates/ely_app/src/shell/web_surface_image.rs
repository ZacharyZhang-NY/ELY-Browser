use image::{ImageBuffer, Rgba, imageops::FilterType};

const WEB_SURFACE_IMAGE_MAX_EDGE: u32 = 1024;

pub(super) fn renderable_image_buffer(
    buffer: ImageBuffer<Rgba<u8>, Vec<u8>>,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let largest_edge = buffer.width().max(buffer.height());
    if largest_edge <= WEB_SURFACE_IMAGE_MAX_EDGE {
        return buffer;
    }

    image::imageops::resize(
        &buffer,
        scaled_image_dimension(buffer.width(), largest_edge),
        scaled_image_dimension(buffer.height(), largest_edge),
        FilterType::Triangle,
    )
}

fn scaled_image_dimension(dimension: u32, largest_edge: u32) -> u32 {
    let numerator = u64::from(dimension) * u64::from(WEB_SURFACE_IMAGE_MAX_EDGE);
    let rounded = (numerator + u64::from(largest_edge / 2)) / u64::from(largest_edge);
    match u32::try_from(rounded.max(1)) {
        Ok(value) => value,
        Err(_) => WEB_SURFACE_IMAGE_MAX_EDGE,
    }
}
