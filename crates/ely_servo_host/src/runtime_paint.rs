use ely_domain::WebViewId;

use super::SoftwareServoHost;
use crate::{RenderedFrame, ServoHostError};

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
        self.paint_without_readback_with_completion(webview_id, true)
    }

    pub fn paint_without_readback_with_completion(
        &mut self,
        webview_id: &WebViewId,
        wait_for_completion: bool,
    ) -> Result<(), ServoHostError> {
        self.paint_webview(webview_id, false, wait_for_completion).map(|_| ())
    }

    fn paint_webview(
        &mut self,
        webview_id: &WebViewId,
        capture_frame: bool,
        wait_for_completion: bool,
    ) -> Result<Option<RenderedFrame>, ServoHostError> {
        let rendering_context = self.webview(webview_id)?.rendering_context.clone();
        rendering_context.make_current().map_err(|_| ServoHostError::RenderingContextNotCurrent)?;
        rendering_context.prepare_for_rendering();
        // Clear the pending-frame flag before `paint()` so barrier callers observe
        // the next Servo frame-ready notification for this paint.
        {
            let webview = self.webview(webview_id)?;
            webview.delegate.mark_frame_presented();
            webview.webview.paint();
        }
        if wait_for_completion {
            self.wait_for_paint_completion(webview_id);
        }
        let rendered_frame = if capture_frame {
            Some(Self::read_rendered_frame(rendering_context.as_ref())?)
        } else {
            None
        };
        rendering_context.present();
        self.webview(webview_id)?.delegate.mark_frame_presented();
        Ok(rendered_frame)
    }

    pub fn paint_with_readback(
        &mut self,
        webview_id: &WebViewId,
        wait_for_completion: bool,
    ) -> Result<(), ServoHostError> {
        let Some(rendered_frame) = self.paint_webview(webview_id, true, wait_for_completion)?
        else {
            return Err(ServoHostError::RenderedFrameUnavailable);
        };
        self.last_rendered_frame = Some(rendered_frame);
        Ok(())
    }
}
