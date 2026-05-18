use crate::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    NativeSurfaceHandle, Pixels, Style, StyleRefinement, Styled, Window,
};
use refineable::Refineable;

/// A native platform surface hosted inside a GPUI layout box.
pub struct NativeSurface {
    id: ElementId,
    on_surface: Option<Box<dyn FnMut(NativeSurfaceHandle, Bounds<Pixels>, &mut Window, &mut App)>>,
    style: StyleRefinement,
}

/// Create a native platform surface element.
pub fn native_surface(
    id: impl Into<ElementId>,
    on_surface: impl FnMut(NativeSurfaceHandle, Bounds<Pixels>, &mut Window, &mut App) + 'static,
) -> NativeSurface {
    NativeSurface { id: id.into(), on_surface: Some(Box::new(on_surface)), style: Default::default() }
}

impl Element for NativeSurface {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
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
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(global_id) = global_id else {
            return;
        };
        let Some(on_surface) = self.on_surface.as_mut() else {
            return;
        };
        let Some(surface) = window.with_element_state::<NativeSurfaceState, _>(
            global_id,
            |state, window| {
                let surface = state
                    .and_then(|state| state.surface)
                    .or_else(|| window.create_native_surface());
                if let Some(surface) = surface {
                    window.sync_native_surface(&surface, bounds);
                    return (Some(surface.clone()), NativeSurfaceState { surface: Some(surface) });
                }
                (None, NativeSurfaceState { surface: None })
            },
        ) else {
            return;
        };
        on_surface(surface, bounds, window, cx);
    }
}

impl IntoElement for NativeSurface {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for NativeSurface {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct NativeSurfaceState {
    surface: Option<NativeSurfaceHandle>,
}
