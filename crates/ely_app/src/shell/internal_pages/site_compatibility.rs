use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{BrowserTab, ProfileKind, SiteOrigin, TabState};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{IconName, StyledExt, clipboard::Clipboard, scroll::ScrollableElement};

use super::{ElyShell, render_canvas_surface};

const BUILD_REVISION: &str = env!("ELY_BUILD_REVISION");
const GPUI_VERSION: &str = env!("ELY_GPUI_VERSION");
const SERVO_VERSION: &str = env!("ELY_SERVO_VERSION");

impl ElyShell {
    pub(super) fn render_site_compatibility_page(
        &mut self,
        snapshot: &BrowserSnapshot,
    ) -> AnyElement {
        let Some(active_tab) = active_tab(snapshot) else {
            return render_canvas_surface(
                div().size_full().p_8().flex().flex_col().gap_5().child(render_missing_tab()),
            );
        };

        let origin = origin_for_tab(active_tab);
        let report = diagnostic_report(snapshot, active_tab, origin.as_ref());

        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_compatibility_header(snapshot, active_tab, report.clone()))
                .child(render_compatibility_summary(snapshot, active_tab, origin.as_ref()))
                .child(render_compatibility_rows(snapshot, active_tab, origin.as_ref())),
        )
    }
}

fn render_missing_tab() -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::hairline()))
        .bg(rgb(colors::canvas_soft()))
        .px_4()
        .py_3()
        .flex()
        .items_center()
        .gap_3()
        .child(div().text_color(rgb(colors::error())).child(IconName::TriangleAlert))
        .child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::ink()))
                .child("Active tab is unavailable."),
        )
        .into_any_element()
}

fn render_compatibility_header(
    snapshot: &BrowserSnapshot,
    active_tab: &BrowserTab,
    report: String,
) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(px(26.0))
                        .text_color(rgb(colors::ink()))
                        .child("Site Compatibility"),
                )
                .child(div().text_sm().truncate().text_color(rgb(colors::muted())).child(format!(
                    "{} / {}",
                    snapshot.active_profile_name,
                    diagnostic_url_scope(active_tab)
                ))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .font_semibold()
                        .text_color(rgb(colors::muted()))
                        .child(IconName::Inspector)
                        .child("Diagnostics"),
                )
                .child(Clipboard::new("copy-site-compatibility-diagnostics").value(report)),
        )
        .into_any_element()
}

fn render_compatibility_summary(
    snapshot: &BrowserSnapshot,
    active_tab: &BrowserTab,
    origin: Option<&SiteOrigin>,
) -> AnyElement {
    let (title, detail, icon, color) = match active_tab.state() {
        TabState::Ready => (
            "Current page is ready",
            "Diagnostics omit URL path and query before copying.",
            IconName::CircleCheck,
            colors::success(),
        ),
        TabState::Loading => (
            "Current page is loading",
            "Capture diagnostics after the page reaches a stable state.",
            IconName::LoaderCircle,
            colors::primary(),
        ),
        TabState::Crashed => (
            "Current page crashed",
            "The copied report includes tab state and profile-scoped permissions.",
            IconName::TriangleAlert,
            colors::error(),
        ),
        TabState::Discarded => (
            "Current page is sleeping",
            "Wake the tab to refresh Servo rendering details.",
            IconName::EyeOff,
            colors::muted(),
        ),
        TabState::Archived => (
            "Current page is archived",
            "Restore the tab to refresh Servo rendering details.",
            IconName::Folder,
            colors::muted(),
        ),
    };

    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::hairline()))
        .bg(rgb(colors::canvas_soft()))
        .px_4()
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(div().text_color(rgb(color)).child(icon))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(rgb(colors::ink()))
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::muted()))
                                .child(detail),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .text_xs()
                .font_semibold()
                .child(
                    div()
                        .text_color(rgb(colors::muted()))
                        .child(format!("{} permissions", site_permission_count(snapshot, origin))),
                )
                .child(
                    div()
                        .text_color(rgb(colors::muted()))
                        .child(format!("{} audits", site_permission_audit_count(snapshot, origin))),
                ),
        )
        .into_any_element()
}

fn render_compatibility_rows(
    snapshot: &BrowserSnapshot,
    active_tab: &BrowserTab,
    origin: Option<&SiteOrigin>,
) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::hairline()))
        .child(compatibility_row(
            IconName::Globe,
            "URL Scope",
            diagnostic_url_scope(active_tab),
            "Scheme, host, and port only",
        ))
        .child(compatibility_row(
            IconName::CircleUser,
            "Profile",
            &snapshot.active_profile_name,
            profile_kind_label(&snapshot.active_profile_kind),
        ))
        .child(compatibility_row(
            IconName::GalleryVerticalEnd,
            "Space",
            &snapshot.active_space_name,
            "Active workspace context",
        ))
        .child(compatibility_row(
            IconName::CircleCheck,
            "Tab State",
            tab_state_label(active_tab.state()),
            active_tab.title(),
        ))
        .child(compatibility_row(
            IconName::Globe,
            "Servo",
            format!("servo {SERVO_VERSION}"),
            "Web rendering engine",
        ))
        .child(compatibility_row(
            IconName::Frame,
            "GPUI",
            format!("gpui {GPUI_VERSION}"),
            "Native shell renderer",
        ))
        .child(compatibility_row(IconName::GitHub, "Build", BUILD_REVISION, "Build source"))
        .child(compatibility_row(
            IconName::CircleCheck,
            "Site Permissions",
            site_permission_count(snapshot, origin).to_string(),
            "Profile-scoped configured decisions",
        ))
        .child(compatibility_row(
            IconName::Inspector,
            "Console Summary",
            "No console events captured",
            "Current Servo host bridge does not expose console events",
        ))
        .into_any_element()
}

fn compatibility_row(
    icon: IconName,
    label: &'static str,
    value: impl Into<String>,
    detail: impl Into<String>,
) -> AnyElement {
    let value = value.into();
    let detail = detail.into();

    div()
        .py_3()
        .border_b_1()
        .border_color(rgb(colors::hairline()))
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(div().text_color(rgb(colors::muted_soft())).child(icon))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .truncate()
                                .text_color(rgb(colors::ink()))
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::muted()))
                                .child(detail),
                        ),
                ),
        )
        .child(
            div()
                .max_w(px(360.0))
                .truncate()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::ink()))
                .child(value),
        )
        .into_any_element()
}

fn diagnostic_report(
    snapshot: &BrowserSnapshot,
    active_tab: &BrowserTab,
    origin: Option<&SiteOrigin>,
) -> String {
    [
        "ELY Browser Site Compatibility Diagnostics".to_string(),
        format!("Build: {BUILD_REVISION}"),
        format!("Servo: servo {SERVO_VERSION}"),
        format!("GPUI: gpui {GPUI_VERSION}"),
        format!("Space: {}", snapshot.active_space_name),
        format!("Profile: {}", snapshot.active_profile_name),
        format!("Profile kind: {}", profile_kind_label(&snapshot.active_profile_kind)),
        format!("Local diagnostics: {}", snapshot.diagnostic_events.len()),
        format!("URL scope: {}", diagnostic_url_scope(active_tab)),
        format!("Tab title: {}", active_tab.title()),
        format!("Tab state: {}", tab_state_label(active_tab.state())),
        format!("Site permissions: {}", site_permission_count(snapshot, origin)),
        format!("Permission audit events: {}", site_permission_audit_count(snapshot, origin)),
        "Console summary: No console events captured by the current Servo host bridge.".to_string(),
    ]
    .join("\n")
}

fn active_tab(snapshot: &BrowserSnapshot) -> Option<&BrowserTab> {
    snapshot.tabs.iter().find(|tab| tab.id() == &snapshot.active_tab_id)
}

fn origin_for_tab(tab: &BrowserTab) -> Option<SiteOrigin> {
    SiteOrigin::from_url(tab.url()).ok().flatten()
}

fn diagnostic_url_scope(tab: &BrowserTab) -> String {
    origin_for_tab(tab)
        .map_or_else(|| tab.url().as_str().to_string(), |origin| origin.as_str().to_string())
}

fn site_permission_count(snapshot: &BrowserSnapshot, origin: Option<&SiteOrigin>) -> usize {
    let Some(origin) = origin else {
        return 0;
    };

    snapshot.site_permissions.iter().filter(|entry| entry.origin() == origin).count()
}

fn site_permission_audit_count(snapshot: &BrowserSnapshot, origin: Option<&SiteOrigin>) -> usize {
    let Some(origin) = origin else {
        return 0;
    };

    snapshot.site_permission_audit_events.iter().filter(|event| event.origin() == origin).count()
}

fn profile_kind_label(profile_kind: &ProfileKind) -> &'static str {
    match profile_kind {
        ProfileKind::Standard => "Standard",
        ProfileKind::Private => "Private",
    }
}

fn tab_state_label(state: &TabState) -> &'static str {
    match state {
        TabState::Loading => "Loading",
        TabState::Ready => "Ready",
        TabState::Crashed => "Crashed",
        TabState::Discarded => "Sleeping",
        TabState::Archived => "Archived",
    }
}

#[cfg(test)]
mod tests {
    use ely_browser_core::{BrowserCore, InitialBrowserConfig};
    use ely_domain::{ProfileId, SpaceId, TabId, UrlText};

    use super::{BrowserTab, active_tab, diagnostic_report, diagnostic_url_scope, origin_for_tab};

    #[test]
    fn diagnostic_url_scope_omits_path_and_query() -> Result<(), Box<dyn std::error::Error>> {
        let tab = BrowserTab::new(
            TabId::new(),
            SpaceId::new(),
            ProfileId::new(),
            "Example",
            UrlText::parse("https://example.com/private/path?token=secret#hash")?,
        );

        assert_eq!(diagnostic_url_scope(&tab), "https://example.com");
        Ok(())
    }

    #[test]
    fn diagnostic_url_scope_keeps_internal_route() -> Result<(), Box<dyn std::error::Error>> {
        let tab = BrowserTab::new(
            TabId::new(),
            SpaceId::new(),
            ProfileId::new(),
            "Settings",
            UrlText::parse("ely://settings/advanced")?,
        );

        assert_eq!(diagnostic_url_scope(&tab), "ely://settings/advanced");
        Ok(())
    }

    #[test]
    fn diagnostic_report_includes_local_event_count() -> Result<(), Box<dyn std::error::Error>> {
        let core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let snapshot = core.snapshot()?;
        let active_tab = active_tab(&snapshot)
            .ok_or_else(|| std::io::Error::other("default browser starts with an active tab"))?;
        let origin = origin_for_tab(active_tab);
        let report = diagnostic_report(&snapshot, active_tab, origin.as_ref());

        assert!(report.contains("Local diagnostics: 1"));
        assert!(!report.contains("Diagnostics reporting:"));
        Ok(())
    }
}
