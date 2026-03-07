use iced::{Color, color};

// Board
pub const BOARD_WOOD: Color = color!(0xF3D07E);
pub const BORDER_LINE: Color = color!(0x663918);
pub const GRID_LINE: Color = color!(0x663918, 0.6);
pub const STAR_POINT: Color = GRID_LINE;

// Stones
pub const STONE_BLACK: Color = color!(0x0B0B0B);
pub const STONE_WHITE: Color = color!(0xE5DDD0);
pub const STONE_SHADOW: Color = color!(0x000000, 0.25);
pub const BLACK_HIGHLIGHT: Color = color!(0xA7A39F, 0.50);
pub const BLACK_SHADE: Color = color!(0x000000, 0.25);
pub const BLACK_OUTLINE: Color = color!(0x1C1C1C, 0.5);
pub const WHITE_HIGHLIGHT: Color = color!(0xFFFFFF, 0.33);
pub const WHITE_SHADE: Color = color!(0xACA18B, 0.25);
pub const WHITE_OUTLINE: Color = color!(0x7B6637, 0.25);

// UI
pub const LAST_MOVE_MARKER: Color = color!(0xF92A71);
pub const KO_MARKER: Color = color!(0x61ADEE);
pub const GHOST_ALPHA: f32 = 0.40;

// Status bar
pub const STATUS_INFO: Color = color!(0xE4Bf7A);
pub const STATUS_WARNING: Color = color!(0xFF6B7F);
pub const STATUS_ERROR: Color = color!(0x5C0900);

// Tree panel
pub const TREE_CURSOR_RING: Color = KO_MARKER;

// Layout
pub const PANEL_SPACING: f32 = 6.0;
pub const PANEL_PADDING: u16 = 8;
pub const RIGHT_PANEL_WIDTH: f32 = 240.0;

// Game info panel
pub const INFO_LABEL: Color = color!(0x888888);
pub const ACTIVE_STONE_SIZE: f32 = 16.0;
pub const INACTIVE_STONE_SIZE: f32 = 11.0;
