use ely_domain::{ProfileId, TabId, UrlText, WebViewId};

use crate::ServoHostError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebViewState {
    Created,
    Loading,
    Complete,
    Sleeping,
    Crashed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedFrameSummary {
    width: u32,
    height: u32,
    opaque_pixel_count: u64,
    non_white_pixel_count: u64,
    content_pixel_count: u64,
    sample_hash: u64,
}

impl RenderedFrameSummary {
    #[must_use]
    pub fn from_rgba_bytes(width: u32, height: u32, rgba_bytes: &[u8]) -> Self {
        let mut opaque_pixel_count = 0;
        let mut non_white_pixel_count = 0;
        let mut content_pixel_count = 0;
        let mut sample_hash = 0xcbf29ce484222325_u64;

        for (index, pixel) in rgba_bytes.chunks_exact(4).enumerate() {
            let [red, green, blue, alpha] = [pixel[0], pixel[1], pixel[2], pixel[3]];
            if alpha > 0 {
                opaque_pixel_count += 1;
            }
            if alpha > 0 && (red < 245 || green < 245 || blue < 245) {
                non_white_pixel_count += 1;
            }
            if alpha > 0 && (red < 220 || green < 220 || blue < 220) {
                content_pixel_count += 1;
            }
            if index % 97 == 0 {
                for byte in pixel {
                    sample_hash ^= u64::from(*byte);
                    sample_hash = sample_hash.wrapping_mul(0x100000001b3);
                }
            }
        }

        Self {
            width,
            height,
            opaque_pixel_count,
            non_white_pixel_count,
            content_pixel_count,
            sample_hash,
        }
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn opaque_pixel_count(&self) -> u64 {
        self.opaque_pixel_count
    }

    #[must_use]
    pub fn non_white_pixel_count(&self) -> u64 {
        self.non_white_pixel_count
    }

    #[must_use]
    pub fn content_pixel_count(&self) -> u64 {
        self.content_pixel_count
    }

    #[must_use]
    pub fn sample_hash(&self) -> u64 {
        self.sample_hash
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedFrame {
    width: u32,
    height: u32,
    rgba_bytes: Vec<u8>,
    summary: RenderedFrameSummary,
}

impl RenderedFrame {
    #[must_use]
    pub fn from_rgba_bytes(width: u32, height: u32, rgba_bytes: Vec<u8>) -> Self {
        let summary = RenderedFrameSummary::from_rgba_bytes(width, height, &rgba_bytes);
        Self { width, height, rgba_bytes, summary }
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn rgba_bytes(&self) -> &[u8] {
        &self.rgba_bytes
    }

    #[must_use]
    pub fn summary(&self) -> &RenderedFrameSummary {
        &self.summary
    }

    #[must_use]
    pub fn opaque_pixel_count(&self) -> u64 {
        self.summary.opaque_pixel_count()
    }

    #[must_use]
    pub fn non_white_pixel_count(&self) -> u64 {
        self.summary.non_white_pixel_count()
    }

    #[must_use]
    pub fn content_pixel_count(&self) -> u64 {
        self.summary.content_pixel_count()
    }

    #[must_use]
    pub fn sample_hash(&self) -> u64 {
        self.summary.sample_hash()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebViewSnapshot {
    webview_id: WebViewId,
    tab_id: TabId,
    profile_id: ProfileId,
    state: WebViewState,
    url: Option<String>,
    title: Option<String>,
    has_pending_frame: bool,
}

impl WebViewSnapshot {
    #[must_use]
    pub fn new(
        webview_id: WebViewId,
        tab_id: TabId,
        profile_id: ProfileId,
        state: WebViewState,
        url: Option<String>,
        title: Option<String>,
        has_pending_frame: bool,
    ) -> Self {
        Self { webview_id, tab_id, profile_id, state, url, title, has_pending_frame }
    }

    #[must_use]
    pub fn webview_id(&self) -> &WebViewId {
        &self.webview_id
    }

    #[must_use]
    pub fn tab_id(&self) -> &TabId {
        &self.tab_id
    }

    #[must_use]
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[must_use]
    pub fn state(&self) -> &WebViewState {
        &self.state
    }

    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn has_pending_frame(&self) -> bool {
        self.has_pending_frame
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRequest {
    pub webview_id: WebViewId,
    pub tab_id: TabId,
    pub url: UrlText,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScrollRequest {
    pub webview_id: WebViewId,
    pub delta_x: i32,
    pub delta_y: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionRequest {
    pub webview_id: WebViewId,
    pub tab_id: TabId,
    pub profile_id: ProfileId,
    pub feature: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PermissionDecision {
    AllowOnce,
    AllowAlways,
    DenyAlways,
}

pub trait ServoHost {
    fn create_webview(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
    ) -> Result<WebViewId, ServoHostError>;

    fn navigate(&mut self, request: NavigationRequest) -> Result<(), ServoHostError>;

    fn scroll(&mut self, request: ScrollRequest) -> Result<(), ServoHostError>;

    fn set_permission(
        &mut self,
        request: PermissionRequest,
        decision: PermissionDecision,
    ) -> Result<(), ServoHostError>;

    fn state(&self, webview_id: &WebViewId) -> Result<WebViewState, ServoHostError>;

    fn snapshot(&self, webview_id: &WebViewId) -> Result<WebViewSnapshot, ServoHostError>;

    fn tick(&mut self) -> bool;

    fn paint(&mut self, webview_id: &WebViewId) -> Result<(), ServoHostError>;

    fn last_rendered_frame(&self) -> Result<RenderedFrame, ServoHostError>;
}
