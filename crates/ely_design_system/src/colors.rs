//! Theme-aware color tokens.
//!
//! Each token is a function (not a `const`) so the same call site can
//! resolve to the light or dark value depending on the current mode.
//! Mode lives in a thread-local that the GPUI render impl sets once
//! per frame via [`set_mode`]; the entire single-threaded render tree
//! reads the same value, so there is no per-element parameter to
//! thread through.
//!
//! Semantic accents (Accent, Violet, Rose, Mint) stay constant across
//! modes — their hue is the brand and looks correct against both warm
//! cream and warm graphite backdrops.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Light,
    Dark,
}

thread_local! {
    static CURRENT_MODE: Cell<Mode> = const { Cell::new(Mode::Light) };
}

/// Resolve the mode used by every subsequent [`Mode`]-keyed function
/// call on this thread. The Render impl calls this once at the top of
/// every frame so widgets that read e.g. [`ink`] return the right
/// shade without owning a `Mode` parameter.
pub fn set_mode(mode: Mode) {
    CURRENT_MODE.with(|cell| cell.set(mode));
}

#[must_use]
pub fn mode() -> Mode {
    CURRENT_MODE.with(Cell::get)
}

#[must_use]
fn pick(light: u32, dark: u32) -> u32 {
    match mode() {
        Mode::Light => light,
        Mode::Dark => dark,
    }
}

// Ink scale — text hierarchy with warm undertones. In dark mode the
// hierarchy inverts: brightest at the top, darkest at the bottom,
// keeping the same step contrast (~12 % luminance gaps) as the light
// scale so headings + secondary copy still pull the reader's eye in
// the right order.
#[must_use]
pub fn ink() -> u32 {
    pick(0x1d1c1a, 0xf2efe9)
}
#[must_use]
pub fn ink_2() -> u32 {
    pick(0x3a3733, 0xcfc8be)
}
#[must_use]
pub fn ink_3() -> u32 {
    pick(0x6b6660, 0xa39b8e)
}
#[must_use]
pub fn ink_4() -> u32 {
    pick(0x9a948c, 0x7c7568)
}
#[must_use]
pub fn ink_5() -> u32 {
    pick(0xc8c2b8, 0x4f4942)
}

// Glass surfaces — semi-transparent panels. Dark mode swaps the white
// for warm-graphite so a translucent panel sits naturally on a dark
// wallpaper without bleaching the underlying gradient.
#[must_use]
pub fn glass() -> u32 {
    pick(0xffffff9e, 0x1f1d1b9e)
}
#[must_use]
pub fn glass_2() -> u32 {
    pick(0xffffffc7, 0x1f1d1bc7)
}
#[must_use]
pub fn glass_3() -> u32 {
    pick(0xffffffeb, 0x1f1d1beb)
}
#[must_use]
pub fn tint() -> u32 {
    pick(0xffffff6b, 0x1f1d1b6b)
}

// Stroke & divider — subtle borders. Dark mode uses warm-white at
// matching opacity so panels keep their inner highlight.
#[must_use]
pub fn stroke() -> u32 {
    pick(0x281e1414, 0xf2efe914)
}
#[must_use]
pub fn stroke_2() -> u32 {
    pick(0x281e140a, 0xf2efe90a)
}
#[must_use]
pub fn divider() -> u32 {
    pick(0x281e140f, 0xf2efe90f)
}
#[must_use]
pub fn hairline() -> u32 {
    pick(0xe6e5e0, 0x312d28)
}
#[must_use]
pub fn hairline_strong() -> u32 {
    pick(0xcfcdc4, 0x49443c)
}

// Accent — warm clay palette. Stays constant across modes; the hue is
// the brand and reads correctly against both creamy + graphite
// backdrops with the same contrast ladder.
#[must_use]
pub fn accent() -> u32 {
    0xc96442
}
#[must_use]
pub fn accent_light() -> u32 {
    0xd97757
}
#[must_use]
pub fn accent_hover() -> u32 {
    pick(0xc9644214, 0xc9644229)
}

// Secondary accents.
#[must_use]
pub fn violet() -> u32 {
    0x7c6cf7
}
#[must_use]
pub fn rose() -> u32 {
    0xec6a8e
}
#[must_use]
pub fn mint() -> u32 {
    0x4fb59a
}

// Canvas backgrounds (opaque fallbacks for non-wallpaper surfaces).
// Dark canvas uses a warm graphite that holds up against the dark
// wallpapers without crushing contrast.
#[must_use]
pub fn canvas() -> u32 {
    pick(0xf0eee9, 0x141312)
}
#[must_use]
pub fn canvas_soft() -> u32 {
    pick(0xfafaf7, 0x1a1816)
}
#[must_use]
pub fn surface_card() -> u32 {
    pick(0xffffff, 0x24211f)
}

// Semantic — same hue across modes, lightness shifted slightly in
// dark mode so the chip reads at the same legibility weight.
#[must_use]
pub fn success() -> u32 {
    pick(0x2f7d68, 0x6cc8a8)
}
#[must_use]
pub fn error() -> u32 {
    pick(0xcf2d56, 0xff7095)
}

// Legacy aliases — preserved during migration from the old constant
// surface. Internal call sites still reach for `colors::body()`,
// `colors::muted()`, etc., so keep the names available.
#[must_use]
pub fn primary() -> u32 {
    accent()
}
#[must_use]
pub fn primary_active() -> u32 {
    pick(0xb55a3c, 0xe2785a)
}
#[must_use]
pub fn body() -> u32 {
    ink_2()
}
#[must_use]
pub fn muted() -> u32 {
    ink_3()
}
#[must_use]
pub fn muted_soft() -> u32 {
    ink_4()
}
#[must_use]
pub fn surface_strong() -> u32 {
    hairline()
}

#[cfg(test)]
mod tests {
    use super::{Mode, ink, mode, set_mode};

    #[test]
    fn mode_toggle_flips_ink() {
        set_mode(Mode::Light);
        let light = ink();
        set_mode(Mode::Dark);
        let dark = ink();
        assert_ne!(light, dark, "dark ink must differ from light ink");
        assert_eq!(mode(), Mode::Dark);
        // Restore the default so subsequent tests on this thread aren't
        // dragged into dark mode.
        set_mode(Mode::Light);
    }
}
