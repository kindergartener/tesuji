use std::time::Instant;

use iced::{
    Color, Element, Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        self, Clipboard, Renderer as _, Shell, graphics::geometry, layout, renderer, widget::Tree,
    },
    alignment,
    gradient::ColorStop,
    mouse,
    widget::{
        canvas::{Frame, Gradient, Path, Stroke, Text, gradient::Linear},
        image,
    },
    window,
};

use crate::gui::{Message, assets::BoardAssets, theme};
use tesuji::sgf::{Board, Cell};

/// Stone radius as a fraction of cell size.
pub const STONE_RADIUS_RATIO: f32 = 0.48;

/// Stone image size as a multiple of stone radius (includes shadow margin).
pub const STONE_IMAGE_SCALE: f32 = 2.0;

/// Distance from border grid lines to center of coordinate labels,
/// as a fraction of cell size.
pub const LABEL_PADDING: f32 = 0.8;

// ── Drawing primitives (for benchmarking) ──

/// A gradient specification for a filled shape.
#[derive(Debug, Clone)]
pub struct GradientSpec {
    pub start: Point,
    pub end: Point,
    pub stops: Vec<ColorStop>,
}

/// A single drawing command produced by the board renderer.
#[derive(Debug, Clone)]
pub enum DrawPrimitive {
    FillRect {
        origin: Point,
        size: Size,
        color: Color,
    },
    StrokeLine {
        from: Point,
        to: Point,
        color: Color,
        width: f32,
    },
    FillCircle {
        center: Point,
        radius: f32,
        color: Color,
    },
    FillCircleGradient {
        center: Point,
        radius: f32,
        gradient: GradientSpec,
    },
    StrokeCircle {
        center: Point,
        radius: f32,
        color: Color,
        width: f32,
    },
    StrokeRect {
        origin: Point,
        size: Size,
        color: Color,
        width: f32,
    },
    DrawImage {
        bounds: Rectangle,
        handle: image::Handle,
    },
    DrawText {
        content: String,
        position: Point,
        size: f32,
        color: Color,
    },
}

/// Build all drawing primitives for the board without touching the GPU.
///
/// This is the pure, benchmarkable core of the board renderer. The actual
/// `draw()` method replays these primitives onto an iced `Frame`.
pub fn build_board_primitives(
    board: &Board,
    metrics: &BoardMetrics,
    hover: Option<(usize, usize)>,
    last_move: Option<(usize, usize)>,
) -> Vec<DrawPrimitive> {
    let size = board.size;
    let mut out = Vec::with_capacity(size * size + 50);

    // 1. Background
    let br = metrics.board_rect();
    out.push(DrawPrimitive::FillRect {
        origin: Point { x: br.x, y: br.y },
        size: Size {
            width: br.width,
            height: br.height,
        },
        color: theme::BOARD_WOOD,
    });

    // 2. Grid lines
    for i in 0..size {
        let color = if i == 0 || i == size - 1 {
            theme::BORDER_LINE
        } else {
            theme::GRID_LINE
        };
        // horizontal
        out.push(DrawPrimitive::StrokeLine {
            from: metrics.coord_to_pixel(0, i),
            to: metrics.coord_to_pixel(size - 1, i),
            color,
            width: 1.0,
        });
        // vertical
        out.push(DrawPrimitive::StrokeLine {
            from: metrics.coord_to_pixel(i, 0),
            to: metrics.coord_to_pixel(i, size - 1),
            color,
            width: 1.0,
        });
    }

    // 3. Star points
    let star_r = metrics.cell_size * 0.10;
    for &(col, row) in star_points(size) {
        out.push(DrawPrimitive::FillCircle {
            center: metrics.coord_to_pixel(col, row),
            radius: star_r,
            color: theme::STAR_POINT,
        });
    }

    // 4. Coordinate labels
    out.extend(build_labels(metrics));

    // 5. Stones
    let stone_r = metrics.cell_size * STONE_RADIUS_RATIO;
    for row in 0..size {
        for col in 0..size {
            let cell = board.cells[row][col];
            let color = match cell {
                Cell::Empty => continue,
                Cell::Black => theme::STONE_BLACK,
                Cell::White => theme::STONE_WHITE,
            };
            let center = metrics.coord_to_pixel(col, row);

            // Shadow
            let shadow_center = Point {
                x: center.x + stone_r * 0.20,
                y: center.y + stone_r * 0.20,
            };
            out.push(DrawPrimitive::FillCircle {
                center: shadow_center,
                radius: stone_r,
                color: theme::STONE_SHADOW,
            });
            out.push(DrawPrimitive::FillCircle {
                center,
                radius: stone_r,
                color,
            });

            // Gradient
            let (highlight, shade) = match cell {
                Cell::Black => (theme::BLACK_HIGHLIGHT, theme::BLACK_SHADE),
                Cell::White => (theme::WHITE_HIGHLIGHT, theme::WHITE_SHADE),
                Cell::Empty => unreachable!(),
            };
            out.push(DrawPrimitive::FillCircleGradient {
                center,
                radius: stone_r,
                gradient: GradientSpec {
                    start: Point {
                        x: center.x - stone_r,
                        y: center.y - stone_r,
                    },
                    end: Point {
                        x: center.x + stone_r,
                        y: center.y + stone_r,
                    },
                    stops: vec![
                        ColorStop {
                            offset: 0.0,
                            color: highlight,
                        },
                        ColorStop {
                            offset: 1.0,
                            color: shade,
                        },
                    ],
                },
            });

            // Outline
            let outline_color = match cell {
                Cell::Black => theme::BLACK_OUTLINE,
                Cell::White => theme::WHITE_OUTLINE,
                Cell::Empty => unreachable!(),
            };
            out.push(DrawPrimitive::StrokeCircle {
                center,
                radius: stone_r,
                color: outline_color,
                width: 1.0,
            });
        }
    }

    // 5. Last-move marker
    if let Some((col, row)) = last_move {
        let center = metrics.coord_to_pixel(col, row);
        let marker_r_inner = stone_r * 0.40;
        let marker_r_outer = stone_r * 0.55;
        let (inner_color, outer_color) = match board.cells[row][col] {
            Cell::Black => (theme::STONE_BLACK, theme::STONE_WHITE),
            Cell::White => (theme::STONE_WHITE, theme::STONE_BLACK),
            Cell::Empty => unreachable!(),
        };
        out.push(DrawPrimitive::FillCircle {
            center,
            radius: marker_r_outer,
            color: outer_color,
        });
        out.push(DrawPrimitive::FillCircle {
            center,
            radius: marker_r_inner,
            color: inner_color,
        });
    }

    // 6. Ko marker
    if let Some((row, col)) = board.ko_point {
        let center = metrics.coord_to_pixel(col, row);
        let half = stone_r * 0.40;
        out.push(DrawPrimitive::StrokeRect {
            origin: Point {
                x: center.x - half,
                y: center.y - half,
            },
            size: Size {
                width: half * 2.0,
                height: half * 2.0,
            },
            color: theme::KO_MARKER,
            width: 2.0,
        });
    }

    // 7. Ghost stone (hover preview)
    if let Some((col, row)) = hover {
        if board.cells[row][col] == Cell::Empty {
            let center = metrics.coord_to_pixel(col, row);
            let ghost_color = match current_player(board) {
                Cell::Black => Color {
                    a: theme::GHOST_ALPHA,
                    ..theme::STONE_BLACK
                },
                Cell::White => Color {
                    a: theme::GHOST_ALPHA,
                    ..theme::STONE_WHITE
                },
                Cell::Empty => unreachable!(),
            };
            out.push(DrawPrimitive::FillCircle {
                center,
                radius: stone_r,
                color: ghost_color,
            });
        }
    }

    out
}

/// Shadow offset as a fraction of stone image size.
pub const SHADOW_OFFSET_RATIO: f32 = 0.06;
pub const SHADOW_RADIUS_RATIO: f32 = 1.10;

/// Grouped drawing primitives for layered rendering.
///
/// Each group maps to a separate render layer to ensure correct z-ordering
/// (iced renders all images after all vector primitives within a single layer).
pub struct TexturedPrimitives {
    /// Wood background image.
    pub background: DrawPrimitive,
    /// Grid lines and star points (vector).
    pub grid: Vec<DrawPrimitive>,
    /// Shadow images (one per stone, offset from stone center).
    pub shadows: Vec<DrawPrimitive>,
    /// Stone images (one per stone).
    pub stones: Vec<DrawPrimitive>,
    /// Markers and overlays: last-move, ko, ghost stone (vector).
    pub overlays: Vec<DrawPrimitive>,
}

/// Build grouped drawing primitives using image assets.
pub fn build_board_primitives_textured(
    board: &Board,
    metrics: &BoardMetrics,
    hover: Option<(usize, usize)>,
    last_move: Option<(usize, usize)>,
    assets: &BoardAssets,
) -> TexturedPrimitives {
    let size = board.size;

    // 1. Background — single wood texture scaled to board area
    let background = DrawPrimitive::DrawImage {
        bounds: metrics.board_rect(),
        handle: assets.wood.clone(),
    };

    // 2. Grid lines
    let mut grid = Vec::with_capacity(size * 2 + 10);
    for i in 0..size {
        let color = if i == 0 || i == size - 1 {
            theme::BORDER_LINE
        } else {
            theme::GRID_LINE
        };
        grid.push(DrawPrimitive::StrokeLine {
            from: metrics.coord_to_pixel(0, i),
            to: metrics.coord_to_pixel(size - 1, i),
            color,
            width: 1.0,
        });
        grid.push(DrawPrimitive::StrokeLine {
            from: metrics.coord_to_pixel(i, 0),
            to: metrics.coord_to_pixel(i, size - 1),
            color,
            width: 1.0,
        });
    }

    // 3. Star points
    let star_r = metrics.cell_size * 0.10;
    for &(col, row) in star_points(size) {
        grid.push(DrawPrimitive::FillCircle {
            center: metrics.coord_to_pixel(col, row),
            radius: star_r,
            color: theme::STAR_POINT,
        });
    }

    // 4. Coordinate labels
    grid.extend(build_labels(metrics));

    // 5. Shadows and stones
    let stone_r = metrics.cell_size * STONE_RADIUS_RATIO;
    let stone_img_size = stone_r * STONE_IMAGE_SCALE;
    let shadow_img_size = metrics.cell_size * SHADOW_RADIUS_RATIO;
    let shadow_offset = stone_img_size * SHADOW_OFFSET_RATIO;
    let mut shadows = Vec::new();
    let mut stones = Vec::new();
    for row in 0..size {
        for col in 0..size {
            let cell = board.cells[row][col];
            let handle = match cell {
                Cell::Empty => continue,
                Cell::Black => &assets.black_stone,
                Cell::White => &assets.white_stone,
            };
            let center = metrics.coord_to_pixel(col, row);

            shadows.push(DrawPrimitive::DrawImage {
                bounds: Rectangle {
                    x: center.x - stone_img_size / 2.0 + shadow_offset,
                    y: center.y - stone_img_size / 2.0 + shadow_offset,
                    width: shadow_img_size,
                    height: shadow_img_size,
                },
                handle: assets.shadow.clone(),
            });

            stones.push(DrawPrimitive::DrawImage {
                bounds: Rectangle {
                    x: center.x - stone_img_size / 2.0,
                    y: center.y - stone_img_size / 2.0,
                    width: stone_img_size,
                    height: stone_img_size,
                },
                handle: handle.clone(),
            });
        }
    }

    // 5. Overlays
    let mut overlays = Vec::new();

    // Last-move marker
    if let Some((col, row)) = last_move {
        let center = metrics.coord_to_pixel(col, row);
        let marker_r_inner = stone_r * 0.40;
        let marker_r_outer = stone_r * 0.55;
        let (inner_color, outer_color) = match board.cells[row][col] {
            Cell::Black => (theme::STONE_BLACK, theme::STONE_WHITE),
            Cell::White => (theme::STONE_WHITE, theme::STONE_BLACK),
            Cell::Empty => unreachable!(),
        };
        overlays.push(DrawPrimitive::FillCircle {
            center,
            radius: marker_r_outer,
            color: outer_color,
        });
        overlays.push(DrawPrimitive::FillCircle {
            center,
            radius: marker_r_inner,
            color: inner_color,
        });
    }

    // Ko marker
    if let Some((row, col)) = board.ko_point {
        let center = metrics.coord_to_pixel(col, row);
        let half = stone_r * 0.40;
        overlays.push(DrawPrimitive::StrokeRect {
            origin: Point {
                x: center.x - half,
                y: center.y - half,
            },
            size: Size {
                width: half * 2.0,
                height: half * 2.0,
            },
            color: theme::KO_MARKER,
            width: 2.0,
        });
    }

    // Ghost stone (hover preview)
    if let Some((col, row)) = hover {
        if board.cells[row][col] == Cell::Empty {
            let center = metrics.coord_to_pixel(col, row);
            let ghost_color = match current_player(board) {
                Cell::Black => Color {
                    a: theme::GHOST_ALPHA,
                    ..theme::STONE_BLACK
                },
                Cell::White => Color {
                    a: theme::GHOST_ALPHA,
                    ..theme::STONE_WHITE
                },
                Cell::Empty => unreachable!(),
            };
            overlays.push(DrawPrimitive::FillCircle {
                center,
                radius: stone_r,
                color: ghost_color,
            });
        }
    }

    TexturedPrimitives {
        background,
        grid,
        shadows,
        stones,
        overlays,
    }
}

/// Replay a single vector drawing primitive onto an iced `Frame`.
/// Image primitives are skipped — they must be drawn via `image::Renderer`.
fn replay_vector_primitive(frame: &mut Frame, prim: &DrawPrimitive) {
    match prim {
        DrawPrimitive::FillRect {
            origin,
            size,
            color,
        } => {
            frame.fill_rectangle(*origin, *size, *color);
        }
        DrawPrimitive::StrokeLine {
            from,
            to,
            color,
            width,
        } => {
            frame.stroke(
                &Path::line(*from, *to),
                Stroke::default().with_color(*color).with_width(*width),
            );
        }
        DrawPrimitive::FillCircle {
            center,
            radius,
            color,
        } => {
            frame.fill(&Path::circle(*center, *radius), *color);
        }
        DrawPrimitive::FillCircleGradient {
            center,
            radius,
            gradient,
        } => {
            let grad = Gradient::Linear(
                Linear::new(gradient.start, gradient.end).add_stops(gradient.stops.iter().copied()),
            );
            frame.fill(&Path::circle(*center, *radius), grad);
        }
        DrawPrimitive::StrokeCircle {
            center,
            radius,
            color,
            width,
        } => {
            frame.stroke(
                &Path::circle(*center, *radius),
                Stroke::default().with_color(*color).with_width(*width),
            );
        }
        DrawPrimitive::StrokeRect {
            origin,
            size,
            color,
            width,
        } => {
            frame.stroke(
                &Path::rectangle(*origin, *size),
                Stroke::default().with_color(*color).with_width(*width),
            );
        }
        DrawPrimitive::DrawText {
            content,
            position,
            size,
            color,
        } => {
            let mut txt = Text::from(content.as_str());
            txt.position = *position;
            txt.size = (*size).into();
            txt.color = *color;
            txt.align_x = iced::widget::text::Alignment::Center;
            txt.align_y = alignment::Vertical::Center;
            frame.fill_text(txt);
        }
        DrawPrimitive::DrawImage { .. } => {
            // Images are drawn directly via image::Renderer, not through Frame.
        }
    }
}

/// Replay a single drawing primitive onto an iced `Frame` (including images).
fn replay_primitive(frame: &mut Frame, prim: &DrawPrimitive) {
    match prim {
        DrawPrimitive::DrawImage { bounds, handle } => {
            use iced::widget::canvas;
            frame.draw_image(*bounds, canvas::Image::new(handle.clone()));
        }
        other => replay_vector_primitive(frame, other),
    }
}

/// Replay all drawing primitives onto an iced `Frame`.
#[allow(dead_code)]
fn replay_primitives(frame: &mut Frame, primitives: &[DrawPrimitive]) {
    for prim in primitives {
        replay_primitive(frame, prim);
    }
}

// ── Custom Widget (uses Renderer::with_layer for correct z-ordering) ──

pub struct BoardWidget<'a> {
    pub board: &'a Board,
    pub hover: Option<(usize, usize)>,
    pub last_move: Option<(usize, usize)>,
    pub show_fps: bool,
    pub assets: &'a BoardAssets,
}

pub struct BoardMetrics {
    pub cell_size: f32,
    pub margin: f32,
    pub origin: Point,
    pub board_size: usize,
}

impl BoardMetrics {
    pub fn new(bounds: Size, board_size: usize) -> Self {
        let available = bounds.width.min(bounds.height);
        let margin = available * 0.05;
        let cell_size = if board_size > 1 {
            (available - 2.0 * margin) / (board_size as f32 - 1.0)
        } else {
            available - 2.0 * margin
        };
        let origin = Point {
            x: (bounds.width - available) / 2.0,
            y: (bounds.height - available) / 2.0,
        };
        Self {
            cell_size,
            margin,
            origin,
            board_size,
        }
    }

    /// The square region covered by the board (grid + margin padding).
    fn board_rect(&self) -> Rectangle {
        let side = self.margin * 2.0
            + if self.board_size > 1 {
                self.cell_size * (self.board_size as f32 - 1.0)
            } else {
                self.cell_size
            };
        Rectangle {
            x: self.origin.x,
            y: self.origin.y,
            width: side,
            height: side,
        }
    }

    fn coord_to_pixel(&self, col: usize, row: usize) -> Point {
        // Snap to half-pixel so 1px strokes land on exact pixel boundaries.
        let x = (self.origin.x + self.margin + col as f32 * self.cell_size).round() + 0.5;
        let y = (self.origin.y + self.margin + row as f32 * self.cell_size).round() + 0.5;
        Point { x, y }
    }

    fn pixel_to_coord(&self, pos: Point) -> Option<(usize, usize)> {
        let col_f = (pos.x - self.origin.x - self.margin) / self.cell_size;
        let row_f = (pos.y - self.origin.y - self.margin) / self.cell_size;

        let col = col_f.round() as i32;
        let row = row_f.round() as i32;

        if col < 0 || row < 0 || col >= self.board_size as i32 || row >= self.board_size as i32 {
            return None;
        }

        // Half-cell snapping threshold
        if (col_f - col as f32).abs() > 0.5 || (row_f - row as f32).abs() > 0.5 {
            return None;
        }

        Some((col as usize, row as usize))
    }
}

/// Go column letter for a given column index (0-based). Skips 'I'.
fn col_label(col: usize) -> char {
    let c = b'A' + col as u8;
    // Skip 'I' to avoid confusion with 'J'
    if c >= b'I' {
        (c + 1) as char
    } else {
        c as char
    }
}

/// Generate coordinate label primitives for all 4 sides of the board.
fn build_labels(metrics: &BoardMetrics) -> Vec<DrawPrimitive> {
    let size = metrics.board_size;
    let pad = metrics.cell_size * LABEL_PADDING;
    let font_size = (metrics.cell_size * 0.38).clamp(8.0, 16.0);
    let mut out = Vec::with_capacity(size * 4);

    let first = metrics.coord_to_pixel(0, 0);
    let last = metrics.coord_to_pixel(size - 1, size - 1);

    // Column labels (A-T) — top and bottom
    for col in 0..size {
        let p = metrics.coord_to_pixel(col, 0);
        let label = col_label(col).to_string();

        // Top
        out.push(DrawPrimitive::DrawText {
            content: label.clone(),
            position: Point {
                x: p.x,
                y: first.y - pad,
            },
            size: font_size,
            color: theme::LABEL_COLOR,
        });

        // Bottom
        let p_bot = metrics.coord_to_pixel(col, size - 1);
        out.push(DrawPrimitive::DrawText {
            content: label,
            position: Point {
                x: p_bot.x,
                y: last.y + pad,
            },
            size: font_size,
            color: theme::LABEL_COLOR,
        });
    }

    // Row labels (1-19, bottom-to-top) — left and right
    for row in 0..size {
        let p = metrics.coord_to_pixel(0, row);
        // Row 0 (top of screen) = highest number, row size-1 (bottom) = 1
        let label = (size - row).to_string();

        // Left
        out.push(DrawPrimitive::DrawText {
            content: label.clone(),
            position: Point {
                x: first.x - pad,
                y: p.y,
            },
            size: font_size,
            color: theme::LABEL_COLOR,
        });

        // Right
        let p_right = metrics.coord_to_pixel(size - 1, row);
        out.push(DrawPrimitive::DrawText {
            content: label,
            position: Point {
                x: p_right.x + pad,
                y: p_right.y,
            },
            size: font_size,
            color: theme::LABEL_COLOR,
        });
    }

    out
}

fn star_points(board_size: usize) -> &'static [(usize, usize)] {
    match board_size {
        19 => &[
            (3, 3),
            (3, 9),
            (3, 15),
            (9, 3),
            (9, 9),
            (9, 15),
            (15, 3),
            (15, 9),
            (15, 15),
        ],
        13 => &[(3, 3), (3, 9), (6, 6), (9, 3), (9, 9)],
        9 => &[(2, 2), (2, 6), (4, 4), (6, 2), (6, 6)],
        _ => &[],
    }
}

impl<'a> advanced::Widget<Message, iced::Theme, iced::Renderer> for BoardWidget<'a> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: layout::Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use advanced::image::Renderer as ImageRenderer;
        use geometry::Renderer as GeometryRenderer;

        let t0 = if self.show_fps {
            Some(Instant::now())
        } else {
            None
        };

        let bounds = layout.bounds();
        let metrics = BoardMetrics::new(bounds.size(), self.board.size);
        let layers = build_board_primitives_textured(
            self.board,
            &metrics,
            self.hover,
            self.last_move,
            self.assets,
        );

        // Apply the same translation the canvas widget uses so that
        // Frame-local (0,0) coordinates map to the widget's screen position.
        let translation = Vector::new(bounds.x, bounds.y);
        let local_clip = Rectangle::with_size(bounds.size());

        renderer.with_translation(translation, |renderer| {
            // Layer 1: Wood background image
            renderer.with_layer(local_clip, |renderer| {
                if let DrawPrimitive::DrawImage { bounds, handle } = &layers.background {
                    let img = advanced::image::Image::new(handle.clone());
                    renderer.draw_image(img, *bounds, local_clip);
                }
            });

            // Layer 2: Grid lines + star points
            renderer.with_layer(local_clip, |renderer| {
                let mut frame = Frame::new(renderer, bounds.size());
                for prim in &layers.grid {
                    replay_vector_primitive(&mut frame, prim);
                }
                renderer.draw_geometry(frame.into_geometry());
            });

            // Layer 3: Stone shadows
            renderer.with_layer(local_clip, |renderer| {
                for prim in &layers.shadows {
                    if let DrawPrimitive::DrawImage { bounds, handle } = prim {
                        let img = advanced::image::Image::new(handle.clone());
                        renderer.draw_image(img, *bounds, local_clip);
                    }
                }
            });

            // Layer 4: Stone images
            renderer.with_layer(local_clip, |renderer| {
                for prim in &layers.stones {
                    if let DrawPrimitive::DrawImage { bounds, handle } = prim {
                        let img = advanced::image::Image::new(handle.clone());
                        renderer.draw_image(img, *bounds, local_clip);
                    }
                }
            });

            // Layer 5: Markers and overlays (last-move, ko, ghost stone, FPS)
            renderer.with_layer(local_clip, |renderer| {
                let mut frame = Frame::new(renderer, bounds.size());
                for prim in &layers.overlays {
                    replay_vector_primitive(&mut frame, prim);
                }

                if let Some(t0) = t0 {
                    let elapsed = t0.elapsed();
                    let ms = elapsed.as_secs_f64() * 1000.0;
                    let label = format!("{ms:.1}ms");
                    let mut txt = Text::from(label);
                    txt.position = Point { x: 4.0, y: 4.0 };
                    txt.size = 12.0.into();
                    txt.color = Color::from_rgba(1.0, 0.2, 0.2, 0.9);
                    frame.fill_text(txt);
                }

                renderer.draw_geometry(frame.into_geometry());
            });
        });
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let metrics = BoardMetrics::new(bounds.size(), self.board.size);

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let coord = cursor
                    .position_in(bounds)
                    .and_then(|pos| metrics.pixel_to_coord(pos));
                let (col, row) = match coord {
                    Some((c, r)) => (Some(c), Some(r)),
                    None => (None, None),
                };
                shell.publish(Message::BoardHovered { col, row });
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                shell.publish(Message::BoardHovered {
                    col: None,
                    row: None,
                });
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    if let Some((col, row)) = metrics.pixel_to_coord(pos) {
                        shell.publish(Message::BoardClicked { col, row });
                        shell.capture_event();
                    }
                }
            }
            Event::Window(window::Event::Resized(_)) => {
                shell.publish(Message::BoardResized {
                    cell_size: metrics.cell_size,
                });
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> From<BoardWidget<'a>> for Element<'a, Message> {
    fn from(widget: BoardWidget<'a>) -> Self {
        Self::new(widget)
    }
}

pub fn current_player(board: &Board) -> Cell {
    if board.move_number % 2 == 0 {
        Cell::Black
    } else {
        Cell::White
    }
}
