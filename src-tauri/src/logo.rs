//! The embedded logo art the `unkai-logo://` URI scheme serves.
//!
//! Split out of the desktop shell's `tray.rs` (the mobile build has
//! no tray): the PNGs are still baked into the binary so the style
//! picker never depends on a runtime resource path, and the webview
//! reaches them through the custom scheme registered in `lib.rs`.

/// Bytes of every per-style logo PNG, baked into the binary at
/// compile time so the picker doesn't depend on runtime resources.
/// 256 px is the right pick: large enough that downscales for the
/// 32 px tray and the 16/32 px Windows window icon stay sharp,
/// small enough that all 7 styles together add < 100 KB to the
/// binary.
pub mod logo_assets {
    pub const STORM: &[u8] = include_bytes!("../../logos/unkai-logo/png/storm/unkai-256.png");
    pub const DAWN: &[u8] = include_bytes!("../../logos/unkai-logo/png/dawn/unkai-256.png");
    pub const MINT: &[u8] = include_bytes!("../../logos/unkai-logo/png/mint/unkai-256.png");
    pub const SKY: &[u8] = include_bytes!("../../logos/unkai-logo/png/sky/unkai-256.png");
    pub const TWILIGHT: &[u8] = include_bytes!("../../logos/unkai-logo/png/twilight/unkai-256.png");
    pub const MONO_BLACK: &[u8] =
        include_bytes!("../../logos/unkai-logo/png/monochrome/unkai-mono-black.png");
    pub const MONO_WHITE: &[u8] =
        include_bytes!("../../logos/unkai-logo/png/monochrome/unkai-mono-white.png");

    // ── v2 logo set (added in #197 follow-up) ────────────────────
    // Same 256 px naming convention as v1; lives under the
    // separate `unkai-logo-v2` folder so the original art and
    // the new pack stay independently swappable.
    pub const COPPER: &[u8] = include_bytes!("../../logos/unkai-logo-v2/png/copper/unkai-256.png");
    pub const FOREST: &[u8] = include_bytes!("../../logos/unkai-logo-v2/png/forest/unkai-256.png");
    pub const MIDNIGHT: &[u8] =
        include_bytes!("../../logos/unkai-logo-v2/png/midnight/unkai-256.png");
    pub const OCEAN: &[u8] = include_bytes!("../../logos/unkai-logo-v2/png/ocean/unkai-256.png");
    pub const ROSE: &[u8] = include_bytes!("../../logos/unkai-logo-v2/png/rose/unkai-256.png");
    pub const SLATE: &[u8] = include_bytes!("../../logos/unkai-logo-v2/png/slate/unkai-256.png");
    pub const SUNSET: &[u8] = include_bytes!("../../logos/unkai-logo-v2/png/sunset/unkai-256.png");
}

/// Map a style slug to the embedded PNG bytes.  Unknown slug →
/// fall back to storm so a stray value (mistyped settings file,
/// future-renamed style) can never leave the tray with no icon.
pub fn logo_bytes_for(style: &str) -> &'static [u8] {
    match style {
        // v1 styles (atmospheric set)
        "dawn" => logo_assets::DAWN,
        "mint" => logo_assets::MINT,
        "sky" => logo_assets::SKY,
        "twilight" => logo_assets::TWILIGHT,
        "monochrome-black" => logo_assets::MONO_BLACK,
        "monochrome-white" => logo_assets::MONO_WHITE,
        // v2 styles (elemental set)
        "copper" => logo_assets::COPPER,
        "forest" => logo_assets::FOREST,
        "midnight" => logo_assets::MIDNIGHT,
        "ocean" => logo_assets::OCEAN,
        "rose" => logo_assets::ROSE,
        "slate" => logo_assets::SLATE,
        "sunset" => logo_assets::SUNSET,
        _ => logo_assets::STORM,
    }
}
