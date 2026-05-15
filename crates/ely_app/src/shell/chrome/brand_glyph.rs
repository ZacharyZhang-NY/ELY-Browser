use ely_design_system::colors;
use gpui::{
    AnyElement, FontWeight, IntoElement, ParentElement, Styled, div, hsla, linear_color_stop,
    linear_gradient, px, rgb, rgba,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Brand {
    Notion,
    YouTube,
    Dribbble,
    X,
    Vercel,
    Behance,
    Figma,
    Slack,
    GitHub,
    Linear,
    Reading,
    News,
}

impl Brand {
    pub(crate) fn from_host(host: &str) -> Option<Self> {
        let normalized = host.trim().to_lowercase();
        let normalized = normalized.strip_prefix("www.").unwrap_or(&normalized);

        if normalized.ends_with("notion.so") || normalized.ends_with("notion.com") {
            return Some(Self::Notion);
        }
        if normalized.ends_with("youtube.com") || normalized.ends_with("youtu.be") {
            return Some(Self::YouTube);
        }
        if normalized.ends_with("dribbble.com") {
            return Some(Self::Dribbble);
        }
        if normalized.ends_with("x.com") || normalized.ends_with("twitter.com") {
            return Some(Self::X);
        }
        if normalized.ends_with("vercel.com") {
            return Some(Self::Vercel);
        }
        if normalized.ends_with("behance.net") {
            return Some(Self::Behance);
        }
        if normalized.ends_with("figma.com") {
            return Some(Self::Figma);
        }
        if normalized.ends_with("slack.com") {
            return Some(Self::Slack);
        }
        if normalized.ends_with("github.com") || normalized.ends_with("github.io") {
            return Some(Self::GitHub);
        }
        if normalized.ends_with("linear.app") {
            return Some(Self::Linear);
        }
        if normalized.ends_with("medium.com")
            || normalized.ends_with("substack.com")
            || normalized.ends_with("nytimes.com")
            || normalized.ends_with("ft.com")
        {
            return Some(Self::News);
        }
        if normalized.ends_with("readwise.io")
            || normalized.ends_with("instapaper.com")
            || normalized.ends_with("pocket.com")
        {
            return Some(Self::Reading);
        }
        None
    }
}

pub(crate) fn render_brand_glyph(brand: Brand, size: f32) -> AnyElement {
    match brand {
        Brand::Notion => render_notion(size),
        Brand::YouTube => render_youtube(size),
        Brand::Dribbble => render_dribbble(size),
        Brand::X => render_x(size),
        Brand::Vercel => render_vercel(size),
        Brand::Behance => render_behance(size),
        Brand::Figma => render_figma(size),
        Brand::Slack => render_slack(size),
        Brand::GitHub => render_github(size),
        Brand::Linear => render_linear(size),
        Brand::Reading => render_reading(size),
        Brand::News => render_news(size),
    }
}

pub(crate) fn render_glyph_for(
    host: Option<&str>,
    fallback_initial: &str,
    size: f32,
) -> AnyElement {
    if let Some(host) = host
        && let Some(brand) = Brand::from_host(host)
    {
        return render_brand_glyph(brand, size);
    }
    render_fallback(fallback_initial, size)
}

/// Single-color accent for a brand — used by surfaces (split-pane headers,
/// tab indicators) that need a readable dot rather than the full brand mark.
/// Matches the design's `accent` prop on each pane.
pub(crate) fn brand_accent_color(brand: Brand) -> u32 {
    match brand {
        Brand::Notion => 0x111111,
        Brand::YouTube => 0xff3b2d,
        Brand::Dribbble => 0xea4c89,
        Brand::X => 0x1d1c1a,
        Brand::Vercel => 0x1d1c1a,
        Brand::Behance => 0x1769ff,
        Brand::Figma => 0x7c6cf7,
        Brand::Slack => 0xe01e5a,
        Brand::GitHub => 0x1d1c1a,
        Brand::Linear => 0x5e6ad2,
        Brand::Reading => 0xc96442,
        Brand::News => 0x3a3733,
    }
}

/// Best-effort accent for any host. Falls back to the warm system accent so
/// unknown sites still render the design's colored dot instead of looking
/// stranded.
pub(crate) fn accent_color_for_host(host: Option<&str>) -> u32 {
    host.and_then(Brand::from_host).map(brand_accent_color).unwrap_or(colors::ACCENT)
}

fn render_notion(size: f32) -> AnyElement {
    flat_square(size, 0xffffff, fontable(size, 0.65, 700.0, 0x111111, "N"))
}

fn render_youtube(size: f32) -> AnyElement {
    flat_square(size, 0xff3b2d, fontable(size, 0.55, 700.0, 0xffffff, "▶"))
}

fn render_dribbble(size: f32) -> AnyElement {
    div()
        .size(px(size))
        .rounded_full()
        .bg(rgb(0xea4c89))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.6))
        .font_weight(FontWeight(800.0))
        .text_color(rgb(0xffffff))
        .child("•")
        .into_any_element()
}

fn render_x(size: f32) -> AnyElement {
    flat_square(size, 0x000000, fontable(size, 0.55, 700.0, 0xffffff, "𝕏"))
}

fn render_vercel(size: f32) -> AnyElement {
    flat_square(size, 0xffffff, fontable(size, 0.55, 700.0, 0x000000, "▲"))
}

fn render_behance(size: f32) -> AnyElement {
    flat_square(size, 0x1769ff, fontable(size, 0.55, 800.0, 0xffffff, "Bē"))
}

fn render_figma(size: f32) -> AnyElement {
    div()
        .size(px(size))
        .rounded(px(size * 0.23))
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(13.0 / 360.0, 0.88, 0.55, 1.0), 0.0),
            linear_color_stop(hsla(266.0 / 360.0, 1.0, 0.67, 1.0), 1.0),
        ))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.55))
        .font_weight(FontWeight(700.0))
        .text_color(rgb(0xffffff))
        .child("F")
        .into_any_element()
}

fn render_slack(size: f32) -> AnyElement {
    div()
        .size(px(size))
        .rounded(px(size * 0.23))
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(196.0 / 360.0, 0.85, 0.59, 1.0), 0.0),
            linear_color_stop(hsla(343.0 / 360.0, 0.78, 0.49, 1.0), 1.0),
        ))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.55))
        .font_weight(FontWeight(800.0))
        .text_color(rgb(0xffffff))
        .child("#")
        .into_any_element()
}

fn render_github(size: f32) -> AnyElement {
    flat_square(size, 0x1d1c1a, fontable(size, 0.6, 700.0, 0xffffff, "◯"))
}

fn render_linear(size: f32) -> AnyElement {
    div()
        .size(px(size))
        .rounded_full()
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(234.0 / 360.0, 0.59, 0.61, 1.0), 0.0),
            linear_color_stop(hsla(20.0 / 360.0, 0.05, 0.10, 1.0), 1.0),
        ))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.55))
        .font_weight(FontWeight(600.0))
        .text_color(rgb(0xffffff))
        .child("L")
        .into_any_element()
}

fn render_reading(size: f32) -> AnyElement {
    flat_square(size, 0xfaeed8, fontable(size, 0.55, 600.0, 0xc96442, "¶"))
}

fn render_news(size: f32) -> AnyElement {
    flat_square(size, 0x1d1c1a, fontable(size, 0.55, 700.0, 0xffffff, "N"))
}

fn render_fallback(initial: &str, size: f32) -> AnyElement {
    let glyph = initial
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "·".to_string());

    div()
        .size(px(size))
        .rounded(px(size * 0.23))
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(341.0 / 360.0, 0.78, 0.85, 1.0), 0.0),
            linear_color_stop(hsla(228.0 / 360.0, 1.0, 0.86, 1.0), 1.0),
        ))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.5))
        .font_weight(FontWeight(600.0))
        .text_color(rgb(colors::INK))
        .child(glyph)
        .into_any_element()
}

fn flat_square(size: f32, fill: u32, content: AnyElement) -> AnyElement {
    div()
        .size(px(size))
        .rounded(px(size * 0.23))
        .bg(rgb(fill))
        .border_1()
        .border_color(rgba(0x0000000f))
        .flex()
        .items_center()
        .justify_center()
        .child(content)
        .into_any_element()
}

fn fontable(size: f32, scale: f32, weight: f32, color: u32, glyph: &str) -> AnyElement {
    div()
        .text_size(px(size * scale))
        .font_weight(FontWeight(weight))
        .text_color(rgb(color))
        .child(glyph.to_string())
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::Brand;

    #[test]
    fn matches_canonical_brand_hosts() {
        assert_eq!(Brand::from_host("notion.so"), Some(Brand::Notion));
        assert_eq!(Brand::from_host("youtube.com"), Some(Brand::YouTube));
        assert_eq!(Brand::from_host("dribbble.com"), Some(Brand::Dribbble));
        assert_eq!(Brand::from_host("x.com"), Some(Brand::X));
        assert_eq!(Brand::from_host("twitter.com"), Some(Brand::X));
        assert_eq!(Brand::from_host("vercel.com"), Some(Brand::Vercel));
        assert_eq!(Brand::from_host("behance.net"), Some(Brand::Behance));
        assert_eq!(Brand::from_host("figma.com"), Some(Brand::Figma));
        assert_eq!(Brand::from_host("slack.com"), Some(Brand::Slack));
        assert_eq!(Brand::from_host("github.com"), Some(Brand::GitHub));
        assert_eq!(Brand::from_host("linear.app"), Some(Brand::Linear));
    }

    #[test]
    fn matches_subdomains_and_www_prefix() {
        assert_eq!(Brand::from_host("WWW.figma.com"), Some(Brand::Figma));
        assert_eq!(Brand::from_host("design.figma.com"), Some(Brand::Figma));
        assert_eq!(Brand::from_host("api.github.com"), Some(Brand::GitHub));
    }

    #[test]
    fn matches_news_and_reading_clusters() {
        assert_eq!(Brand::from_host("medium.com"), Some(Brand::News));
        assert_eq!(Brand::from_host("nytimes.com"), Some(Brand::News));
        assert_eq!(Brand::from_host("readwise.io"), Some(Brand::Reading));
    }

    #[test]
    fn rejects_unknown_hosts() {
        assert_eq!(Brand::from_host("example.com"), None);
        assert_eq!(Brand::from_host(""), None);
    }
}
