pub(crate) mod appearance_form;
pub(crate) mod brand_glyph;
pub(crate) mod command_footer;
pub(crate) mod command_overlay;
pub(crate) mod home;
pub(crate) mod plugin_detail_view;
pub(crate) mod plugin_labels;
pub(crate) mod settings_layout;
pub(crate) mod sidebar;
pub(crate) mod sidebar_header;
pub(crate) mod split_pane;
pub(crate) mod topbar;
pub(crate) mod typography;
pub(crate) mod wallpaper;

pub(crate) use appearance_form::render_appearance_form;
pub(crate) use brand_glyph::render_glyph_for;
pub(crate) use command_overlay::render_command_overlay;
pub(crate) use home::render_home_page;
pub(crate) use plugin_detail_view::render_plugin_detail_view;
pub(crate) use settings_layout::render_settings_landing;
pub(crate) use sidebar::{PANEL_BG, panel_shadow};
pub(crate) use sidebar_header::render_sidebar_header;
pub(crate) use split_pane::{
    pane_host_label, pane_path_label, pane_url_is_secure, render_compact_split_canvas,
    render_split_pane_header,
};
pub(crate) use topbar::render_topbar;
pub(crate) use typography::{SERIF_FAMILY, register_serif_fonts};
pub(crate) use wallpaper::render_wallpaper;
