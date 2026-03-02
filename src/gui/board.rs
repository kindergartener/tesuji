use iced::{
    Color, Event, Point, Rectangle, Size,
    mouse,
    widget::canvas::{self, Action, Frame, Path, Stroke},
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
        Self { cell_size, margin, board_size }
    }

    fn coord_to_pixel(&self, col: usize, row: usize) -> Point {
        Point {
            x: self.margin + col as f32 * self.cell_size,
            y: self.margin + row as f32 * self.cell_size,
        }
    }

    fn pixel_to_coord(&self, pos: Point) -> Option<(usize, usize)> {
        let col_f = (pos.x - self.margin) / self.cell_size;
        let row_f = (pos.y - self.margin) / self.cell_size;

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

        // 1. Background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), theme::BOARD_WOOD);

        // 2. Grid lines
        let grid_stroke = Stroke::default().with_color(theme::GRID_LINE).with_width(1.0);
        for i in 0..size {
            let start = metrics.coord_to_pixel(0, i);
            let end = metrics.coord_to_pixel(size - 1, i);
            frame.stroke(&Path::line(start, end), grid_stroke.clone());

            let start = metrics.coord_to_pixel(i, 0);
            let end = metrics.coord_to_pixel(i, size - 1);
            frame.stroke(&Path::line(start, end), grid_stroke.clone());
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
                // Thin outline for white stones
                if self.board.cells[row][col] == Cell::White {
                    frame.stroke(
                        &Path::circle(center, stone_r),
                        Stroke::default()
                            .with_color(Color::from_rgb(0.6, 0.6, 0.58))
                            .with_width(0.5),
                    );
                }
            }
        }

        // 5. Last-move marker
        if let Some((col, row)) = self.last_move {
            let center = metrics.coord_to_pixel(col, row);
            let marker_r = stone_r * 0.35;
            let marker_color = match self.board.cells[row][col] {
                Cell::Black => Color::from_rgb(0.95, 0.95, 0.92),
                _ => theme::LAST_MOVE_MARKER,
            };
            frame.fill(&Path::circle(center, marker_r), marker_color);
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
