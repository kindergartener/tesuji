pub mod board;
pub mod io;
pub mod theme;

use std::path::PathBuf;

use iced::{
    Element, Length, Task,
    widget::{button, canvas::Canvas, column, container, row, text},
};

use crate::{
    EditCommand, Editor,
    gui::board::{BoardProgram, current_player},
    parse_sgf,
    sgf::{Board, Cell, GameTree, SGFProperty, node::GoCoord},
    write_sgf,
};

pub struct GuiApp {
    pub editor: Editor,
    pub file_path: Option<PathBuf>,
    pub cached_board: Board,
    pub active_game_index: usize,
    pub status_message: Option<StatusMessage>,
    pub hover_coord: Option<(usize, usize)>,
}

pub struct StatusMessage {
    pub text: String,
    pub kind: StatusKind,
}

pub enum StatusKind {
    Info,
    Warning,
    Error,
}

impl StatusMessage {
    fn info(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: StatusKind::Info }
    }
    fn warning(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: StatusKind::Warning }
    }
    fn error(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: StatusKind::Error }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    // File I/O
    OpenFileRequested,
    SaveFileRequested,
    SaveAsRequested,
    FileOpened(Result<(PathBuf, String), String>),
    FileSaved(Result<PathBuf, String>),

    // Board interaction
    BoardClicked { col: usize, row: usize },
    BoardHovered { col: Option<usize>, row: Option<usize> },
    PassRequested,

    // Tree navigation
    NavigateNext,
    NavigatePrev,
    NavigateBranch(usize),

    // Game management
    NewGameRequested,
    SelectGame(usize),
    DismissStatus,
}

impl GuiApp {
    pub fn new() -> (Self, Task<Message>) {
        let tree = new_game_tree();
        let editor = Editor::new(tree);
        let cached_board = Board::from_tree(&editor.tree, editor.cursor);
        (
            Self {
                editor,
                file_path: None,
                cached_board,
                active_game_index: 0,
                status_message: None,
                hover_coord: None,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenFileRequested => {
                return io::open_file_task();
            }
            Message::SaveFileRequested => {
                let content = write_sgf(&self.editor.tree);
                if let Some(path) = self.file_path.clone() {
                    return io::save_file_task(path, content);
                } else {
                    return io::save_as_file_task(content);
                }
            }
            Message::SaveAsRequested => {
                let content = write_sgf(&self.editor.tree);
                return io::save_as_file_task(content);
            }
            Message::FileOpened(Ok((path, text))) => match parse_sgf(&text) {
                Ok(tree) => {
                    let n_games = tree.roots.len();
                    self.editor.apply(EditCommand::Load(tree));
                    self.file_path = Some(path);
                    self.cached_board = Board::from_tree(&self.editor.tree, self.editor.cursor);
                    if n_games > 1 {
                        self.status_message = Some(StatusMessage::warning(format!(
                            "File contains {n_games} games — showing game 1"
                        )));
                    }
                }
                Err(e) => {
                    self.status_message = Some(StatusMessage::error(e.to_string()));
                }
            },
            Message::FileOpened(Err(e)) => {
                self.status_message = Some(StatusMessage::error(e));
            }
            Message::FileSaved(Ok(path)) => {
                self.file_path = Some(path.clone());
                self.status_message = Some(StatusMessage::info(format!(
                    "Saved to {}",
                    path.display()
                )));
            }
            Message::FileSaved(Err(e)) => {
                self.status_message = Some(StatusMessage::error(format!("Save failed: {e}")));
            }
            Message::BoardClicked { col, row } => match try_place_stone(self, col, row) {
                Ok(cmd) => {
                    self.editor.apply(cmd);
                    self.cached_board = Board::from_tree(&self.editor.tree, self.editor.cursor);
                }
                Err(msg) => {
                    self.status_message = Some(StatusMessage::error(msg));
                }
            },
            Message::BoardHovered { col, row } => {
                self.hover_coord = col.zip(row);
            }
            Message::PassRequested => {
                let color = current_player(&self.cached_board);
                let prop = match color {
                    Cell::Black => SGFProperty::B(GoCoord::pass()),
                    Cell::White => SGFProperty::W(GoCoord::pass()),
                    Cell::Empty => unreachable!(),
                };
                self.editor.apply(EditCommand::AddMove(prop));
                self.cached_board = Board::from_tree(&self.editor.tree, self.editor.cursor);
            }
            Message::NavigateNext => {
                self.editor.apply(EditCommand::NavigateNext);
                self.cached_board = Board::from_tree(&self.editor.tree, self.editor.cursor);
            }
            Message::NavigatePrev => {
                self.editor.apply(EditCommand::NavigatePrev);
                self.cached_board = Board::from_tree(&self.editor.tree, self.editor.cursor);
            }
            Message::NavigateBranch(n) => {
                self.editor.apply(EditCommand::NavigateBranch(n));
                self.cached_board = Board::from_tree(&self.editor.tree, self.editor.cursor);
            }
            Message::NewGameRequested => {
                let tree = new_game_tree();
                self.editor.apply(EditCommand::Load(tree));
                self.file_path = None;
                self.cached_board = Board::from_tree(&self.editor.tree, self.editor.cursor);
                self.status_message = None;
            }
            Message::SelectGame(n) => {
                if let Some(&root) = self.editor.tree.roots.get(n) {
                    self.active_game_index = n;
                    self.editor.cursor = root;
                    self.cached_board = Board::from_tree(&self.editor.tree, self.editor.cursor);
                }
            }
            Message::DismissStatus => {
                self.status_message = None;
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let board = &self.cached_board;
        let move_num = board.move_number;
        let player = current_player(board);
        let player_name = match player {
            Cell::Black => "Black",
            Cell::White => "White",
            Cell::Empty => unreachable!(),
        };

        // Compute last move coord from the current cursor node
        let last_move = last_move_coord(&self.editor);

        // Toolbar
        let toolbar = row![
            button("Open").on_press(Message::OpenFileRequested),
            button("Save").on_press(Message::SaveFileRequested),
            button("Save As").on_press(Message::SaveAsRequested),
            button("New Game").on_press(Message::NewGameRequested),
        ]
        .spacing(8);

        // Board canvas
        let program = BoardProgram {
            board,
            hover: self.hover_coord,
            last_move,
        };
        let board_canvas = Canvas::new(program)
            .width(Length::Fill)
            .height(Length::Fill);
        let board_container = container(board_canvas)
            .width(Length::Fill)
            .height(Length::Fill);

        // Nav row
        let nav_info = format!("Move {move_num} · {player_name} to play");
        let nav_row = row![
            button("◀ Prev").on_press(Message::NavigatePrev),
            text(nav_info).size(14),
            button("Next ▶").on_press(Message::NavigateNext),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        // Capture row
        let capture_info = format!(
            "Captured: ● {}  ○ {}",
            board.captured_white,
            board.captured_black,
        );
        let capture_row = row![
            text(capture_info).size(14),
            button("Pass").on_press(Message::PassRequested),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        // Status bar
        let maybe_status: Element<'_, Message> = if let Some(status) = &self.status_message {
            let color = match status.kind {
                StatusKind::Info => theme::STATUS_INFO,
                StatusKind::Warning => theme::STATUS_WARNING,
                StatusKind::Error => theme::STATUS_ERROR,
            };
            let prefix = match status.kind {
                StatusKind::Info => "ℹ",
                StatusKind::Warning => "⚠",
                StatusKind::Error => "✗",
            };
            container(
                row![
                    text(format!("{} {}", prefix, status.text))
                        .size(13)
                        .color(color),
                    button("✕").on_press(Message::DismissStatus),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .padding(4)
            .into()
        } else {
            text("").into()
        };

        column![
            toolbar,
            board_container,
            nav_row,
            capture_row,
            maybe_status,
        ]
        .spacing(6)
        .padding(8)
        .into()
    }
}

fn new_game_tree() -> GameTree {
    use crate::sgf::node::{FileFormat, GameType, Komi};
    let mut tree = GameTree::new();
    tree.node_mut(tree.roots[0]).properties = vec![
        SGFProperty::GM(GameType::Go),
        SGFProperty::FF(FileFormat::FF4),
        SGFProperty::SZ(19),
        SGFProperty::KM(Komi::default()),
    ];
    tree
}

fn last_move_coord(editor: &Editor) -> Option<(usize, usize)> {
    let node = editor.tree.node(editor.cursor);
    for prop in &node.properties {
        match prop {
            SGFProperty::B(coord) | SGFProperty::W(coord) => {
                if !coord.is_pass() {
                    let col = coord.first() as usize - b'a' as usize;
                    let row = coord.second() as usize - b'a' as usize;
                    return Some((col, row));
                }
            }
            _ => {}
        }
    }
    None
}

fn try_place_stone(app: &GuiApp, col: usize, row: usize) -> Result<EditCommand, String> {
    let board = &app.cached_board;
    let size = board.size;

    if col >= size || row >= size {
        return Err(format!("({col},{row}) is off the board"));
    }

    if board.cells[row][col] != Cell::Empty {
        return Err("Intersection is occupied".into());
    }

    if board.ko_point == Some((row, col)) {
        return Err("Illegal move: ko".into());
    }

    let color = current_player(board);
    if would_be_suicide(board, col, row, color) {
        return Err("Illegal move: suicide".into());
    }

    let coord = GoCoord::from_colrow(col, row);
    let prop = match color {
        Cell::Black => SGFProperty::B(coord),
        Cell::White => SGFProperty::W(coord),
        Cell::Empty => unreachable!(),
    };
    Ok(EditCommand::AddMove(prop))
}

/// Simulate placing `color` at `(col, row)` and check if it would be a suicide.
/// Returns true if the group formed would have zero liberties after captures.
fn would_be_suicide(board: &Board, col: usize, row: usize, color: Cell) -> bool {
    // Clone the board state for simulation
    let mut sim_cells = board.cells;
    let size = board.size;

    sim_cells[row][col] = color;

    // Apply opponent captures (same logic as apply_captures)
    let opponent = match color {
        Cell::Black => Cell::White,
        Cell::White => Cell::Black,
        Cell::Empty => return false,
    };

    for &(nr, nc) in &orthogonal_neighbors(row, col, size) {
        if sim_cells[nr][nc] != opponent {
            continue;
        }
        let group = find_group_in(&sim_cells, nr, nc, size);
        if count_liberties_in(&sim_cells, &group, size) == 0 {
            for (gr, gc) in group {
                sim_cells[gr][gc] = Cell::Empty;
            }
        }
    }

    // After captures, check if the placed group has any liberties
    let placed_group = find_group_in(&sim_cells, row, col, size);
    count_liberties_in(&sim_cells, &placed_group, size) == 0
}

fn orthogonal_neighbors(row: usize, col: usize, size: usize) -> Vec<(usize, usize)> {
    let mut n = Vec::with_capacity(4);
    if row > 0 { n.push((row - 1, col)); }
    if row + 1 < size { n.push((row + 1, col)); }
    if col > 0 { n.push((row, col - 1)); }
    if col + 1 < size { n.push((row, col + 1)); }
    n
}

fn find_group_in(cells: &[[Cell; 19]; 19], row: usize, col: usize, size: usize) -> Vec<(usize, usize)> {
    let color = cells[row][col];
    let mut visited = [[false; 19]; 19];
    let mut stack = vec![(row, col)];
    let mut group = Vec::new();
    while let Some((r, c)) = stack.pop() {
        if visited[r][c] { continue; }
        visited[r][c] = true;
        if cells[r][c] != color { continue; }
        group.push((r, c));
        for (nr, nc) in orthogonal_neighbors(r, c, size) {
            if !visited[nr][nc] && cells[nr][nc] == color {
                stack.push((nr, nc));
            }
        }
    }
    group
}

fn count_liberties_in(cells: &[[Cell; 19]; 19], group: &[(usize, usize)], size: usize) -> usize {
    use std::collections::HashSet;
    let mut liberties = HashSet::new();
    for &(r, c) in group {
        for (nr, nc) in orthogonal_neighbors(r, c, size) {
            if cells[nr][nc] == Cell::Empty {
                liberties.insert((nr, nc));
            }
        }
    }
    liberties.len()
}
