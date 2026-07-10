use ely_domain::WebViewId;

use super::SoftwareServoHost;
use crate::{RenderedFrame, ServoHostError, runtime_webview::HostWebViewDelegate};

/// Repaint and present coordination for [`SoftwareServoHost`].
///
/// Servo's rendering contract is: `notify_new_frame_ready` flags the
/// session, then a single `WebView::paint` + `RenderingContext::present`
/// pair lands that frame. These methods own that pair (plus the optional
/// RGBA readback used by the software path); the rest of the host
/// lifecycle stays in `runtime.rs`.
impl SoftwareServoHost {
    /// Paint and present the current surface without RGBA readback.
    pub fn paint_without_readback(&mut self, webview_id: &WebViewId) -> Result<(), ServoHostError> {
        self.paint_webview(webview_id, false).map(|_| ())
    }

    fn paint_webview(
        &mut self,
        webview_id: &WebViewId,
        capture_frame: bool,
    ) -> Result<Option<RenderedFrame>, ServoHostError> {
        let rendering_context = self.webview(webview_id)?.rendering_context.clone();
        rendering_context.make_current().map_err(|_| ServoHostError::RenderingContextNotCurrent)?;
        rendering_context.prepare_for_rendering();
        {
            let webview = self.webview(webview_id)?;
            consume_pending_then_paint(&webview.delegate, || webview.webview.paint());
        }
        let rendered_frame = if capture_frame {
            Some(Self::read_rendered_frame(rendering_context.as_ref())?)
        } else {
            None
        };
        rendering_context.present();
        Ok(rendered_frame)
    }

    pub fn paint_with_readback(&mut self, webview_id: &WebViewId) -> Result<(), ServoHostError> {
        let Some(rendered_frame) = self.paint_webview(webview_id, true)? else {
            return Err(ServoHostError::RenderedFrameUnavailable);
        };
        self.last_rendered_frame = Some(rendered_frame);
        Ok(())
    }
}

fn consume_pending_then_paint(delegate: &HostWebViewDelegate, paint: impl FnOnce()) {
    delegate.mark_frame_presented();
    paint();
}

#[cfg(test)]
mod tests {
    use ely_domain::ProfileId;

    use super::{HostWebViewDelegate, consume_pending_then_paint};
    use crate::runtime_permissions::PermissionStore;

    #[test]
    fn frame_arriving_during_paint_remains_pending() {
        let delegate = HostWebViewDelegate::new(ProfileId::new(), PermissionStore::default());
        delegate.mark_frame_ready();

        consume_pending_then_paint(&delegate, || delegate.mark_frame_ready());

        assert!(delegate.has_pending_frame());
    }
}
