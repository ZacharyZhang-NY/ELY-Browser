use std::{
    ffi::CStr,
    os::raw::c_char,
    sync::{Arc, mpsc},
    time::Duration,
};

use core_video::pixel_buffer::CVPixelBuffer;
use metal::{MTLCommandBufferStatus, MTLPixelFormat, MTLTextureUsage, TextureDescriptor};
use parking_lot::Mutex;
use objc::{msg_send, runtime::Object, sel, sel_impl};

use super::{
    InstanceBufferPool, MetalRenderer, retain_frame_resources_until_completion,
};
use crate::{
    Bounds, ContentMask, Corners, DevicePixels, PaintSurface, ScaledPixels, Scene, SurfaceLease,
    point, size,
};

const COMPLETION_TIMEOUT: Duration = Duration::from_secs(5);
const COMPLETION_GATE_VALUE: u64 = 1;

/// A gated offscreen Metal submission that uses GPUI's production surface pipeline.
pub struct MetalSurfaceTestSubmission {
    gate: metal::SharedEvent,
    command_buffer: metal::CommandBuffer,
    completion_rx: mpsc::Receiver<()>,
    _target_texture: metal::Texture,
    finished: bool,
}

impl MetalSurfaceTestSubmission {
    /// Releases the GPU gate and waits for the production completion handler.
    pub fn finish(mut self) -> crate::Result<()> {
        self.gate.set_signaled_value(COMPLETION_GATE_VALUE);
        self.command_buffer.wait_until_completed();
        self.completion_rx.recv_timeout(COMPLETION_TIMEOUT)?;
        anyhow::ensure!(
            self.command_buffer.status() == MTLCommandBufferStatus::Completed,
            "Metal surface test submission ended with status {:?}: {}",
            self.command_buffer.status(),
            command_buffer_error(self.command_buffer.as_ref()),
        );
        self.finished = true;
        Ok(())
    }
}

impl Drop for MetalSurfaceTestSubmission {
    fn drop(&mut self) {
        if !self.finished {
            self.gate.set_signaled_value(COMPLETION_GATE_VALUE);
        }
    }
}

/// Submits a CoreVideo surface through GPUI's production BGRA Metal pipeline.
pub fn submit_surface_to_metal_for_test(
    image_buffer: CVPixelBuffer,
    lease: SurfaceLease,
) -> crate::Result<MetalSurfaceTestSubmission> {
    let width = u32::try_from(image_buffer.get_width())?;
    let height = u32::try_from(image_buffer.get_height())?;
    anyhow::ensure!(width > 0 && height > 0, "surface dimensions must be positive");
    let viewport_size = size(
        DevicePixels(i32::try_from(width)?),
        DevicePixels(i32::try_from(height)?),
    );
    let instance_buffer_pool = Arc::new(Mutex::new(InstanceBufferPool::default()));
    let mut renderer = MetalRenderer::new(instance_buffer_pool.clone());
    let target_texture = build_target_texture(&renderer.device, width, height);
    let mut scene = build_surface_scene(image_buffer, lease, width, height);
    scene.finish();
    let mut instance_buffer = instance_buffer_pool.lock().acquire(&renderer.device);
    let command_buffer = renderer.draw_primitives(
        &scene,
        &mut instance_buffer,
        target_texture.as_ref(),
        viewport_size,
    )?;
    let gate = renderer.device.new_shared_event();
    gate.set_signaled_value(0);
    command_buffer.encode_wait_for_event(&gate, COMPLETION_GATE_VALUE);
    let (completion_tx, completion_rx) = mpsc::channel();
    retain_frame_resources_until_completion(
        command_buffer.as_ref(),
        &scene,
        instance_buffer,
        instance_buffer_pool,
        Some(completion_tx),
    );
    command_buffer.commit();
    anyhow::ensure!(
        !matches!(
            command_buffer.status(),
            MTLCommandBufferStatus::Completed | MTLCommandBufferStatus::Error
        ),
        "gated Metal surface submission ended early with status {:?}: {error}",
        command_buffer.status(),
        error = command_buffer_error(command_buffer.as_ref()),
    );

    Ok(MetalSurfaceTestSubmission {
        gate,
        command_buffer,
        completion_rx,
        _target_texture: target_texture,
        finished: false,
    })
}

fn command_buffer_error(command_buffer: &metal::CommandBufferRef) -> String {
    #[expect(unsafe_code)]
    unsafe {
        let error: *mut Object = msg_send![command_buffer, error];
        if error.is_null() {
            return "no Metal error description".to_string();
        }
        let description: *mut Object = msg_send![error, localizedDescription];
        if description.is_null() {
            return "Metal error description was null".to_string();
        }
        let utf8: *const c_char = msg_send![description, UTF8String];
        if utf8.is_null() {
            return "Metal error description had no UTF-8 representation".to_string();
        }
        CStr::from_ptr(utf8).to_string_lossy().into_owned()
    }
}

fn build_target_texture(device: &metal::DeviceRef, width: u32, height: u32) -> metal::Texture {
    let descriptor = TextureDescriptor::new();
    descriptor.set_width(u64::from(width));
    descriptor.set_height(u64::from(height));
    descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    descriptor.set_storage_mode(metal::MTLStorageMode::Private);
    descriptor.set_usage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
    device.new_texture(&descriptor)
}

fn build_surface_scene(
    image_buffer: CVPixelBuffer,
    lease: SurfaceLease,
    width: u32,
    height: u32,
) -> Scene {
    let bounds = Bounds::new(
        point(ScaledPixels::default(), ScaledPixels::default()),
        size(ScaledPixels::from(width as f32), ScaledPixels::from(height as f32)),
    );
    let mut scene = Scene::default();
    scene.insert_primitive(PaintSurface {
        order: 0,
        bounds,
        content_mask: ContentMask { bounds },
        corner_radii: Corners::default(),
        image_buffer,
        lease: Some(lease),
    });
    scene
}
