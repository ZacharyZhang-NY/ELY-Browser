pub(super) const PRD_SITE_COMPATIBILITY_CASES: &[PrdSiteCompatibilityCase] = &[
    PrdSiteCompatibilityCase { url: "https://github.com", title_fragment: "GitHub" },
    PrdSiteCompatibilityCase { url: "https://example.com", title_fragment: "Example Domain" },
    PrdSiteCompatibilityCase { url: "https://servo.org/", title_fragment: "Servo" },
];

pub(super) const PRD_REFERENCE_SITE_COMPATIBILITY_CASES: &[PrdSiteCompatibilityCase] = &[
    PrdSiteCompatibilityCase {
        url: "https://blog.google/products-and-platforms/products/chrome/new-chrome-productivity-features/",
        title_fragment: "Chrome",
    },
    PrdSiteCompatibilityCase {
        url: "https://www.microsoft.com/en-us/edge/features/vertical-tabs",
        title_fragment: "Microsoft Edge",
    },
    PrdSiteCompatibilityCase {
        url: "https://resources.arc.net/hc/en-us/articles/19230755904151-Favorites-Top-Tabs-Across-Every-Space",
        title_fragment: "Favorites",
    },
    PrdSiteCompatibilityCase {
        url: "https://resources.arc.net/hc/en-us/articles/19228855311127-Auto-Archive-Clean-as-you-go",
        title_fragment: "Auto Archive",
    },
    PrdSiteCompatibilityCase {
        url: "https://vivaldi.com/features/workspaces/",
        title_fragment: "Workspaces",
    },
    PrdSiteCompatibilityCase {
        url: "https://help.vivaldi.com/desktop/tabs/tab-tiling/",
        title_fragment: "Tab Tiling",
    },
    PrdSiteCompatibilityCase { url: "https://www.gpui.rs/", title_fragment: "gpui" },
    PrdSiteCompatibilityCase { url: "https://docs.rs/gpui", title_fragment: "gpui" },
    PrdSiteCompatibilityCase {
        url: "https://zed.dev/blog/videogame",
        title_fragment: "Leveraging Rust",
    },
    PrdSiteCompatibilityCase {
        url: "https://github.com/longbridge/gpui-component/",
        title_fragment: "gpui-component",
    },
    PrdSiteCompatibilityCase {
        url: "https://github.com/zed-industries/awesome-gpui/",
        title_fragment: "awesome-gpui",
    },
    PrdSiteCompatibilityCase { url: "https://servo.org/", title_fragment: "Servo" },
    PrdSiteCompatibilityCase {
        url: "https://servo.org/blog/2026/04/13/servo-0.1.0-release/",
        title_fragment: "Servo",
    },
    PrdSiteCompatibilityCase {
        url: "https://developers.cloudflare.com/d1/",
        title_fragment: "Cloudflare",
    },
    PrdSiteCompatibilityCase {
        url: "https://developers.cloudflare.com/workers/platform/storage-options/",
        title_fragment: "Cloudflare",
    },
    PrdSiteCompatibilityCase {
        url: "https://developers.cloudflare.com/kv/concepts/how-kv-works/",
        title_fragment: "Cloudflare",
    },
    PrdSiteCompatibilityCase {
        url: "https://better-auth.com/blog/1-5",
        title_fragment: "Better Auth",
    },
    PrdSiteCompatibilityCase {
        url: "https://developers.cloudflare.com/d1/platform/limits/",
        title_fragment: "Cloudflare",
    },
    PrdSiteCompatibilityCase {
        url: "https://component-model.bytecodealliance.org/",
        title_fragment: "WebAssembly Component Model",
    },
    PrdSiteCompatibilityCase {
        url: "https://docs.wasmtime.dev/api/wasmtime/component/index.html",
        title_fragment: "wasmtime",
    },
    PrdSiteCompatibilityCase {
        url: "https://docs.wasmtime.dev/security.html",
        title_fragment: "Wasmtime",
    },
];

pub(super) const PRD_SITE_COMPATIBILITY_SIZES: &[FrameSize] = &[
    FrameSize { width: 640, height: 480 },
    FrameSize { width: 934, height: 657 },
    FrameSize { width: 1614, height: 980 },
];
pub(super) const PRD_REFERENCE_SITE_SIZE: FrameSize = FrameSize { width: 934, height: 657 };
pub(super) const SERVO_SCROLL_SITE: PrdSiteCompatibilityCase =
    PrdSiteCompatibilityCase { url: "https://servo.org/", title_fragment: "Servo" };
pub(super) const SERVO_SCROLL_SIZE: FrameSize = FrameSize { width: 934, height: 657 };
pub(super) const SERVO_SCROLL_OFFSET: ScrollOffset = ScrollOffset { x: 0, y: 480 };
pub(super) const SERVO_CLICK_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EClick%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3B%7D%3C%2Fstyle%3E%3Cbutton%20onclick%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Clicked%27%3Bthis.textContent%3D%27Clicked%27%3B%22%3ETap%3C%2Fbutton%3E";
pub(super) const SERVO_CLICK_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
pub(super) const SERVO_CLICK_POINT: ClickPoint = ClickPoint { x: 160, y: 120 };
pub(super) const SERVO_DRAG_URL: &str = "data:text/html,%3C%21doctype%20html%3E%3Ctitle%3EDrag%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3B%7D%3C%2Fstyle%3E%3Cbutton%20id%3Dbox%3EDrag%3C%2Fbutton%3E%3Cscript%3Elet%20dragging%3Dfalse%3Bconst%20box%3Ddocument.getElementById%28%27box%27%29%3BaddEventListener%28%27mousedown%27%2Cevent%3D%3E%7Bif%28event.target%3D%3D%3Dbox%29%7Bdragging%3Dtrue%3B%7D%7D%29%3BaddEventListener%28%27mousemove%27%2Cevent%3D%3E%7Bif%28dragging%26%26event.clientX%3E280%29%7Bdocument.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Dragged%27%3Bbox.textContent%3D%27Dragged%27%3B%7D%7D%29%3BaddEventListener%28%27mouseup%27%2C%28%29%3D%3E%7Bdragging%3Dfalse%3B%7D%29%3B%3C%2Fscript%3E";
pub(super) const SERVO_DRAG_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
pub(super) const SERVO_DRAG_FROM: ClickPoint = ClickPoint { x: 160, y: 120 };
pub(super) const SERVO_DRAG_TO: ClickPoint = ClickPoint { x: 320, y: 120 };
pub(super) const SERVO_TOUCH_URL: &str = "data:text/html,%3C%21doctype%20html%3E%3Ctitle%3ETouch%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3Btouch-action%3Amanipulation%3B%7D%3C%2Fstyle%3E%3Cbutton%20ontouchstart%3D%22document.body.dataset.touch%3D%27start%27%3B%22%20onclick%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Touched%27%3Bthis.textContent%3D%27Touched%27%3B%22%3ETap%3C%2Fbutton%3E";
pub(super) const SERVO_TOUCH_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
pub(super) const SERVO_TOUCH_POINT: ClickPoint = ClickPoint { x: 160, y: 120 };
pub(super) const SERVO_TEXT_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EText%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3Bfont%3A28px%20sans-serif%3B%7Dinput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A260px%3Bheight%3A70px%3Bfont%3A28px%20sans-serif%3B%7Doutput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A180px%3Bfont%3A32px%20sans-serif%3B%7D%3C%2Fstyle%3E%3Cinput%20id%3Dq%20autofocus%20oninput%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.getElementById%28%27out%27%29.textContent%3Dthis.value%3B%22%3E%3Coutput%20id%3Dout%3Eempty%3C%2Foutput%3E";
pub(super) const SERVO_TEXT_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
pub(super) const SERVO_TEXT_POINT: ClickPoint = ClickPoint { x: 160, y: 120 };
pub(super) const SERVO_TEXT_VALUE: &str = "ely42";

pub(super) struct PrdSiteCompatibilityCase {
    pub(super) url: &'static str,
    pub(super) title_fragment: &'static str,
}

#[derive(Clone, Copy)]
pub(super) struct FrameSize {
    pub(super) width: u64,
    pub(super) height: u64,
}

#[derive(Clone, Copy)]
pub(super) struct ScrollOffset {
    pub(super) x: i64,
    pub(super) y: i64,
}

impl ScrollOffset {
    pub(super) const ZERO: Self = Self { x: 0, y: 0 };
}

#[derive(Clone, Copy)]
pub(super) struct ClickPoint {
    pub(super) x: u64,
    pub(super) y: u64,
}

#[derive(Clone, Copy)]
pub(super) struct DragPoints {
    pub(super) from: ClickPoint,
    pub(super) to: ClickPoint,
}
