use iced::Color;

// Board
pub const BOARD_WOOD: Color = Color::from_rgb(0.82, 0.66, 0.32);
pub const GRID_LINE: Color = Color::from_rgb(0.15, 0.10, 0.05);
pub const STAR_POINT: Color = GRID_LINE;

// Stones
pub const STONE_BLACK: Color = Color::from_rgb(0.08, 0.08, 0.08);
pub const STONE_WHITE: Color = Color::from_rgb(0.95, 0.95, 0.92);
pub const STONE_SHADOW: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.25);

// UI
pub const LAST_MOVE_MARKER: Color = Color::from_rgb(0.85, 0.15, 0.15);
pub const KO_MARKER: Color = Color::from_rgb(0.15, 0.45, 0.85);
pub const GHOST_ALPHA: f32 = 0.40;

// Status bar
pub const STATUS_INFO: Color = Color::from_rgb(0.2, 0.6, 0.2);
pub const STATUS_WARNING: Color = Color::from_rgb(0.8, 0.6, 0.0);
pub const STATUS_ERROR: Color = Color::from_rgb(0.8, 0.2, 0.2);
