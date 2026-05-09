// Ink scale — text hierarchy with warm undertones
pub const INK: u32 = 0x1d1c1a;
pub const INK_2: u32 = 0x3a3733;
pub const INK_3: u32 = 0x6b6660;
pub const INK_4: u32 = 0x9a948c;
pub const INK_5: u32 = 0xc8c2b8;

// Glass surfaces — semi-transparent whites (use with rgba)
pub const GLASS: u32 = 0xffffff9e; // 62% white
pub const GLASS_2: u32 = 0xffffffc7; // 78% white
pub const GLASS_3: u32 = 0xffffffeb; // 92% white
pub const TINT: u32 = 0xffffff6b; // 42% white

// Stroke & divider — subtle borders (use with rgba)
pub const STROKE: u32 = 0x281e1414; // 8% warm dark
pub const STROKE_2: u32 = 0x281e140a; // 4% warm dark
pub const DIVIDER: u32 = 0x281e140f; // 6% warm dark

// Accent — warm clay palette
pub const ACCENT: u32 = 0xc96442;
pub const ACCENT_LIGHT: u32 = 0xd97757;
pub const ACCENT_HOVER: u32 = 0xc9644214; // 8% opacity for row highlights

// Secondary accents
pub const VIOLET: u32 = 0x7c6cf7;
pub const ROSE: u32 = 0xec6a8e;
pub const MINT: u32 = 0x4fb59a;

// Canvas backgrounds (opaque fallbacks)
pub const CANVAS: u32 = 0xf0eee9;
pub const CANVAS_SOFT: u32 = 0xfafaf7;
pub const SURFACE_CARD: u32 = 0xffffff;

// Semantic
pub const SUCCESS: u32 = 0x2f7d68;
pub const ERROR: u32 = 0xcf2d56;

// Legacy aliases — preserved for compatibility during migration
pub const PRIMARY: u32 = ACCENT;
pub const PRIMARY_ACTIVE: u32 = 0xb55a3c;
pub const BODY: u32 = INK_2;
pub const MUTED: u32 = INK_3;
pub const MUTED_SOFT: u32 = INK_4;
pub const HAIRLINE: u32 = 0xe6e5e0;
pub const HAIRLINE_STRONG: u32 = 0xcfcdc4;
pub const SURFACE_STRONG: u32 = 0xe6e5e0;
