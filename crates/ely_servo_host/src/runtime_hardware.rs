use ely_domain::WebViewId;

use super::SoftwareServoHost;
use crate::{IOSurfaceHandle, IOSurfaceIdentity, ServoHostError};

impl SoftwareServoHost {
    /// Marks IOSurface IDs reported ready after import by the app process.
    ///
    /// `IOSurfaceIsInUse == false` supplies the consumer-completion check used
    /// before the rendering context recycles an acknowledged surface.
    pub fn acknowledge_iosurfaces(
        &self,
        webview_id: &WebViewId,
        surface_ids: &[u64],
    ) -> Result<(), ServoHostError> {
        let webview = self.webview(webview_id)?;
        if let Some(hardware_context) = webview.hardware_context.as_ref() {
            hardware_context.acknowledge_iosurfaces(surface_ids);
        }
        Ok(())
    }

    /// Returns the most recently presented IOSurface identity for a hardware webview.
    pub fn peek_iosurface_identity(
        &self,
        webview_id: &WebViewId,
    ) -> Result<Option<IOSurfaceIdentity>, ServoHostError> {
        let webview = self.webview(webview_id)?;
        let Some(hardware_context) = webview.hardware_context.as_ref() else {
            return Ok(None);
        };
        hardware_context
            .peek_iosurface_identity()
            .map_err(|_| ServoHostError::HardwareSurfaceUnavailable { id: webview_id.clone() })
    }

    /// Creates a Mach send right for the most recently presented IOSurface.
    pub fn current_iosurface_handle(
        &self,
        webview_id: &WebViewId,
    ) -> Result<Option<IOSurfaceHandle>, ServoHostError> {
        let webview = self.webview(webview_id)?;
        let Some(hardware_context) = webview.hardware_context.as_ref() else {
            return Ok(None);
        };
        hardware_context
            .current_iosurface_mach_port()
            .map(Some)
            .map_err(|_| ServoHostError::HardwareSurfaceUnavailable { id: webview_id.clone() })
    }
}
