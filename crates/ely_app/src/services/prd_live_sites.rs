use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

pub(crate) const PRD_TOP_SITE_CASES: &[LiveSiteCase] = &[
    LiveSiteCase { url: "https://github.com", title_fragment: "GitHub" },
    LiveSiteCase { url: "https://example.com", title_fragment: "Example Domain" },
    LiveSiteCase { url: "https://servo.org/", title_fragment: "Servo" },
];

pub(crate) const PRD_REFERENCE_SITE_CASES: &[LiveSiteCase] = &[
    LiveSiteCase {
        url: "https://blog.google/products-and-platforms/products/chrome/new-chrome-productivity-features/",
        title_fragment: "Chrome",
    },
    LiveSiteCase {
        url: "https://explore.microsoft.com/en-us/edge/features/vertical-tabs?form=MT0160",
        title_fragment: "Microsoft Edge",
    },
    LiveSiteCase {
        url: "https://resources.arc.net/hc/en-us/articles/19230755904151-Favorites-Top-Tabs-Across-Every-Space",
        title_fragment: "Favorites",
    },
    LiveSiteCase {
        url: "https://resources.arc.net/hc/en-us/articles/19228855311127-Auto-Archive-Clean-as-you-go",
        title_fragment: "Auto Archive",
    },
    LiveSiteCase { url: "https://vivaldi.com/features/workspaces/", title_fragment: "Workspaces" },
    LiveSiteCase {
        url: "https://help.vivaldi.com/desktop/tabs/tab-tiling/",
        title_fragment: "Tab Tiling",
    },
    LiveSiteCase { url: "https://gpui.rs/", title_fragment: "gpui" },
    LiveSiteCase { url: "https://docs.rs/gpui", title_fragment: "gpui" },
    LiveSiteCase { url: "https://zed.dev/blog/videogame", title_fragment: "Leveraging Rust" },
    LiveSiteCase {
        url: "https://github.com/longbridge/gpui-component/",
        title_fragment: "gpui-component",
    },
    LiveSiteCase {
        url: "https://github.com/zed-industries/awesome-gpui/",
        title_fragment: "awesome-gpui",
    },
    LiveSiteCase { url: "https://servo.org/", title_fragment: "Servo" },
    LiveSiteCase {
        url: "https://servo.org/blog/2026/04/13/servo-0.1.0-release/",
        title_fragment: "Servo",
    },
    LiveSiteCase { url: "https://developers.cloudflare.com/d1/", title_fragment: "Cloudflare" },
    LiveSiteCase {
        url: "https://developers.cloudflare.com/workers/platform/storage-options/",
        title_fragment: "Cloudflare",
    },
    LiveSiteCase {
        url: "https://developers.cloudflare.com/kv/concepts/how-kv-works/",
        title_fragment: "Cloudflare",
    },
    LiveSiteCase { url: "https://better-auth.com/blog/1-5", title_fragment: "Better Auth" },
    LiveSiteCase {
        url: "https://developers.cloudflare.com/d1/platform/limits/",
        title_fragment: "Cloudflare",
    },
    LiveSiteCase {
        url: "https://component-model.bytecodealliance.org/",
        title_fragment: "WebAssembly Component Model",
    },
    LiveSiteCase {
        url: "https://docs.wasmtime.dev/api/wasmtime/component/index.html",
        title_fragment: "wasmtime",
    },
    LiveSiteCase { url: "https://docs.wasmtime.dev/security.html", title_fragment: "Wasmtime" },
];

pub(crate) struct LiveSiteCase {
    pub(crate) url: &'static str,
    pub(crate) title_fragment: &'static str,
}

pub(crate) fn assert_prd_reference_urls_are_covered() -> Result<(), Box<dyn Error>> {
    let prd = fs::read_to_string(prd_path())?;
    let prd_urls = prd_reference_urls(&prd);
    let covered_urls = PRD_REFERENCE_SITE_CASES
        .iter()
        .map(|case| normalized_url(case.url))
        .collect::<BTreeSet<_>>();
    let missing_urls = prd_urls
        .iter()
        .filter(|url| !covered_urls.contains(url.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    assert!(missing_urls.is_empty(), "missing PRD live-site smoke cases: {missing_urls:?}");
    assert_eq!(prd_urls.len(), PRD_REFERENCE_SITE_CASES.len());
    Ok(())
}

fn prd_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("PRD.md")
}

fn prd_reference_urls(prd: &str) -> Vec<String> {
    prd.lines()
        .filter(|line| line.starts_with("[R"))
        .filter_map(|line| {
            let start = line.find("https://")?;
            let url = line[start..].split_whitespace().next()?;
            Some(normalized_url(url))
        })
        .collect()
}

fn normalized_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}
