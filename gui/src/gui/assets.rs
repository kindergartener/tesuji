//! Board image assets.
//!
//! Loads textures from `assets/` directory. All four PNG files must be present:
//! `wood.png`, `black.png`, `white.png`, `shadow.png`.

use std::path::Path;

use iced::widget::image;

const ASSETS_DIR: &str = "assets";
const WOOD_FILE: &str = "wood.png";
const BLACK_STONE_FILE: &str = "black.png";
const WHITE_STONE_FILE: &str = "white.png";
const SHADOW_FILE: &str = "shadow.png";

/// Pre-loaded image handles for the board renderer.
pub struct BoardAssets {
    pub wood: image::Handle,
    pub black_stone: image::Handle,
    pub white_stone: image::Handle,
    pub shadow: image::Handle,
}

impl BoardAssets {
    /// Load assets from `assets/` directory.
    ///
    /// # Panics
    ///
    /// Panics if any of the required PNG files are missing.
    pub fn load() -> Self {
        let dir = Path::new(ASSETS_DIR);

        let wood_path = dir.join(WOOD_FILE);
        let black_path = dir.join(BLACK_STONE_FILE);
        let white_path = dir.join(WHITE_STONE_FILE);
        let shadow_path = dir.join(SHADOW_FILE);

        assert!(
            wood_path.exists()
                && black_path.exists()
                && white_path.exists()
                && shadow_path.exists(),
            "Missing asset files in assets/. Required: {WOOD_FILE}, {BLACK_STONE_FILE}, {WHITE_STONE_FILE}, {SHADOW_FILE}"
        );

        Self {
            wood: image::Handle::from_path(wood_path),
            black_stone: image::Handle::from_path(black_path),
            white_stone: image::Handle::from_path(white_path),
            shadow: image::Handle::from_path(shadow_path),
        }
    }
}
