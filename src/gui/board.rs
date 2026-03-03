use iced::{
    Color, Event, Point, Rectangle, Size, gradient::ColorStop, mouse, widget::canvas::{self, Action, Frame, Gradient, Path, Stroke, gradient::Linear}
};

use crate::{
    gui::{Message, theme},
    sgf::{Board, Cell},
};

pub struct BoardProgram<'a> {
    pub board: &'a Board,
    pub hover: Option<(usize, usize)>,
    pub last_move: Option<(usize, usize)>,
}

struct BoardMetrics {
    cell_size: f32,
    margin: f32,
    origin: Point,
    board_size: usize,
}

impl BoardMetrics {
    fn new(bounds: Size, board_size: usize) -> Self {
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
        Self { cell_size, margin, origin, board_size }
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
        Point {
            x: self.origin.x + self.margin + col as f32 * self.cell_size,
            y: self.origin.y + self.margin + row as f32 * self.cell_size,
        }
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

fn star_points(board_size: usize) -> &'static [(usize, usize)] {
    match board_size {
        19 => &[
            (3, 3), (3, 9), (3, 15),
            (9, 3), (9, 9), (9, 15),
            (15, 3), (15, 9), (15, 15),
        ],
        13 => &[(3, 3), (3, 9), (6, 6), (9, 3), (9, 9)],
        9 => &[(2, 2), (2, 6), (4, 4), (6, 2), (6, 6)],
        _ => &[],
    }
}

impl<'a> canvas::Program<Message> for BoardProgram<'a> {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
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
                Some(Action::publish(Message::BoardHovered { col, row }))
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                Some(Action::publish(Message::BoardHovered { col: None, row: None }))
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    if let Some((col, row)) = metrics.pixel_to_coord(pos) {
                        return Some(
                            Action::publish(Message::BoardClicked { col, row }).and_capture(),
                        );
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let metrics = BoardMetrics::new(bounds.size(), self.board.size);
        let size = self.board.size;

        // 1. Background — fill only the square board area
        let board_rect = metrics.board_rect();
        frame.fill_rectangle(
            Point { x: board_rect.x, y: board_rect.y },
            Size { width: board_rect.width, height: board_rect.height },
            theme::BOARD_WOOD,
        );

        // 2. Grid lines
        let make_stroke = |color| Stroke::default().with_color(color).with_width(1.0);
        for i in 0..size {
            let stroke = make_stroke(if i == 0 || i == size - 1 {
                theme::BORDER_LINE
            } else {
                theme::GRID_LINE
            });

            for (x1, y1, x2, y2) in [
                (0, i, size - 1, i),
                (i, 0, i, size - 1)
            ] {
                frame.stroke(&Path::line(metrics.coord_to_pixel(x1, y1), metrics.coord_to_pixel(x2, y2)), stroke);
            }
        }

        // 3. Star points
        let star_r = metrics.cell_size * 0.10;
        for &(col, row) in star_points(size) {
            let center = metrics.coord_to_pixel(col, row);
            frame.fill(&Path::circle(center, star_r), theme::STAR_POINT);
        }

        // 4. Stones
        let stone_r = metrics.cell_size * 0.46;
        for row in 0..size {
            for col in 0..size {
                let color = match self.board.cells[row][col] {
                    Cell::Empty => continue,
                    Cell::Black => theme::STONE_BLACK,
                    Cell::White => theme::STONE_WHITE,
                };
                let center = metrics.coord_to_pixel(col, row);
                // Shadow
                let shadow_center = Point {
                    x: center.x + stone_r * 0.12,
                    y: center.y + stone_r * 0.12,
                };
                frame.fill(&Path::circle(shadow_center, stone_r), theme::STONE_SHADOW);
                frame.fill(&Path::circle(center, stone_r), color);

                // Diffuse shading — diagonal linear gradient (light from top-left)
                let (highlight, shade) = match self.board.cells[row][col] {
                    Cell::Black => (theme::BLACK_HIGHLIGHT, theme::BLACK_SHADE),
                    Cell::White => (theme::WHITE_HIGHLIGHT, theme::WHITE_SHADE),
                    Cell::Empty => unreachable!(),
                };
                let grad = Gradient::Linear(Linear::new(
                        Point { x: center.x - stone_r, y: center.y - stone_r },
                        Point { x: center.x + stone_r, y: center.y + stone_r }
                    ).add_stops([
                        ColorStop { offset: 0.0, color: highlight },
                        ColorStop { offset: 1.0, color: shade }
                    ])
                );
                frame.fill(&Path::circle(center, stone_r), grad);

                // Thin outline for black stone
                if self.board.cells[row][col] == Cell::Black {
                    frame.stroke(
                        &Path::circle(center, stone_r),
                        Stroke::default()
                            .with_color(theme::BLACK_OUTLINE)
                            .with_width(1.0),
                    );
                }
                // Thin outline for white stone
                if self.board.cells[row][col] == Cell::White {
                    frame.stroke(
                        &Path::circle(center, stone_r),
                        Stroke::default()
                            .with_color(theme::WHITE_OUTLINE)
                            .with_width(1.0),
                    );
                }

            }
        }

        // 5. Last-move marker
        if let Some((col, row)) = self.last_move {
            let center = metrics.coord_to_pixel(col, row);
            let marker_r_inner = stone_r * 0.40;
            let marker_r_outer = stone_r * 0.55;
            let (marker_color_inner, marker_color_outer) = match self.board.cells[row][col] {
                Cell::Black => (theme::STONE_BLACK, theme::STONE_WHITE),
                Cell::White => (theme::STONE_WHITE, theme::STONE_BLACK),
                Cell::Empty => unreachable!()
            };
            frame.fill(&Path::circle(center, marker_r_outer), marker_color_outer);
            frame.fill(&Path::circle(center, marker_r_inner), marker_color_inner);
        }

        // 6. Ko marker
        if let Some((row, col)) = self.board.ko_point {
            let center = metrics.coord_to_pixel(col, row);
            let half = stone_r * 0.40;
            frame.stroke(
                &Path::rectangle(
                    Point { x: center.x - half, y: center.y - half },
                    Size { width: half * 2.0, height: half * 2.0 },
                ),
                Stroke::default().with_color(theme::KO_MARKER).with_width(2.0),
            );
        }

        // 7. Ghost stone (hover preview)
        if let Some((col, row)) = self.hover {
            if self.board.cells[row][col] == Cell::Empty {
                let center = metrics.coord_to_pixel(col, row);
                let ghost_color = match current_player(self.board) {
                    Cell::Black => Color { a: theme::GHOST_ALPHA, ..theme::STONE_BLACK },
                    Cell::White => Color { a: theme::GHOST_ALPHA, ..theme::STONE_WHITE },
                    Cell::Empty => unreachable!(),
                };
                frame.fill(&Path::circle(center, stone_r), ghost_color);
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

pub fn current_player(board: &Board) -> Cell {
    if board.move_number % 2 == 0 { Cell::Black } else { Cell::White }
}
