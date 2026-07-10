use crate::{
    App, Bounds, Corners, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, ObjectFit, Pixels, Style, StyleRefinement, Styled, Window,
};
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
use refineable::Refineable;
use std::{fmt, sync::Arc};

/// An owner retained until the GPU finishes reading a surface frame.
///
/// Attach a lease with [`Surface::lease`] when the surface's producer may
/// recycle its backing storage independently of the CoreVideo pixel buffer.
#[derive(Clone)]
pub struct SurfaceLease(Arc<dyn Send + Sync>);

impl SurfaceLease {
    /// Create a lease from a thread-safe shared owner.
    pub fn from_arc<T>(owner: Arc<T>) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self(owner)
    }
}

impl fmt::Debug for SurfaceLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SurfaceLease")
            .field("strong_count", &Arc::strong_count(&self.0))
            .finish()
    }
}

/// A source of a surface's content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfaceSource {
    /// A macOS image buffer from CoreVideo
    #[cfg(target_os = "macos")]
    Surface(CVPixelBuffer),
}

#[cfg(target_os = "macos")]
impl From<CVPixelBuffer> for SurfaceSource {
    fn from(value: CVPixelBuffer) -> Self {
        SurfaceSource::Surface(value)
    }
}

/// A surface element.
pub struct Surface {
    source: SurfaceSource,
    lease: Option<SurfaceLease>,
    object_fit: ObjectFit,
    corner_radii: Option<Corners<Pixels>>,
    style: StyleRefinement,
}

/// Create a new surface element.
pub fn surface(source: impl Into<SurfaceSource>) -> Surface {
    Surface {
        source: source.into(),
        lease: None,
        object_fit: ObjectFit::Contain,
        corner_radii: None,
        style: Default::default(),
    }
}

impl Surface {
    /// Retain an owner until the GPU completes this surface frame.
    pub fn lease(mut self, lease: SurfaceLease) -> Self {
        self.lease = Some(lease);
        self
    }

    /// Set the object fit for the image.
    pub fn object_fit(mut self, object_fit: ObjectFit) -> Self {
        self.object_fit = object_fit;
        self
    }

    /// Set rounded clipping for the rendered surface.
    pub fn corner_radii(mut self, corner_radii: impl Into<Corners<Pixels>>) -> Self {
        self.corner_radii = Some(corner_radii.into());
        self
    }
}

impl Element for Surface {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] window: &mut Window,
        _: &mut App,
    ) {
        match &self.source {
            #[cfg(target_os = "macos")]
            SurfaceSource::Surface(surface) => {
                let size = crate::size(surface.get_width().into(), surface.get_height().into());
                let new_bounds = self.object_fit.get_bounds(bounds, size);
                let mut style = Style::default();
                style.refine(&self.style);
                let corner_radii = self
                    .corner_radii
                    .unwrap_or_else(|| style.corner_radii.to_pixels(window.rem_size()))
                    .clamp_radii_for_quad_size(new_bounds.size);
                window.paint_surface_with_lease(
                    new_bounds,
                    corner_radii,
                    surface.clone(),
                    self.lease.clone(),
                );
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}

impl IntoElement for Surface {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for Surface {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::SurfaceLease;

    struct DropProbe(Arc<AtomicUsize>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn surface_lease_retains_its_owner() {
        let drops = Arc::new(AtomicUsize::new(0));
        let owner = Arc::new(DropProbe(drops.clone()));
        let lease = SurfaceLease::from_arc(owner.clone());

        drop(owner);
        assert_eq!(drops.load(Ordering::SeqCst), 0);

        drop(lease);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
