pub mod board;
pub mod hotkeys;
pub mod io;
pub mod theme;
pub mod tree_panel;

use std::path::PathBuf;

use iced::{
    Color, Element, Length, Task,
    widget::{button, canvas::Canvas, column, container, row, text},
};

use crate::{
    EditCommand, Editor,
    gui::{
        board::{BoardProgram, current_player},
        tree_panel::TreePanelProgram,
    },
    parse_sgf,
    sgf::{
        Board, Cell, GameTree, NodeId, SGFProperty,
        count_liberties, find_group, orthogonal_neighbors,
        node::GoCoord,
    },
    write_sgf,
};

pub struct GuiApp {
    pub editor: Editor,
    pub file_path: Option<PathBuf>,
    pub cached_board: Board,
    /// Board history for incremental back-navigation.
    /// Each entry is the board state *before* applying the node at the
    /// corresponding depth along the root→cursor path.
    pub board_history: Vec<Board>,
    pub active_game_index: usize,
    pub status_message: Option<StatusMessage>,
    pub hover_coord: Option<(usize, usize)>,
    pub confirm_delete: bool,
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

    // Delete node
    DeleteNodeConfirmed,
    DeleteNodeCancelled,

    // Tree navigation
    NavigateNext,
    NavigatePrev,
    NavigateNextVariation,
    NavigatePrevVariation,
    NavigateFirstVariation,
    NavigateLastVariation,
    NavigateFirst,
    NavigateLast,
    NavigateBranch(usize),
    NavigateToNode(NodeId),

    // Undo / Redo
    UndoRequested,
    RedoRequested,

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
                board_history: Vec::new(),
                active_game_index: 0,
                status_message: None,
                hover_coord: None,
                confirm_delete: false,
            },
            Task::none(),
        )
    }

    /// Recompute the board from scratch and rebuild the history stack.
    fn recompute_board(&mut self) {
        let (board, history) = board_with_history(&self.editor.tree, self.editor.cursor);
        self.cached_board = board;
        self.board_history = history;
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenFileRequested => {
                return io::open_file_task();
            }
            Message::SaveFileRequested => {
                let content = write_sgf(&self.editor.tree);
                if let Some(ref path) = self.file_path {
                    return io::save_file_task(path.clone(), content);
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
                    self.recompute_board();
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
            Message::BoardClicked { col, row } => {
                // Check if clicking on the last-move marker -> trigger delete confirmation
                if let Some((last_col, last_row)) = last_move_coord(&self.editor)
                    && col == last_col && row == last_row
                {
                    self.confirm_delete = true;
                    return Task::none();
                }

                match try_place_stone(self, col, row) {
                    Ok(prop) => {
                        // Check for existing child with the same move (auto-variation)
                        let move_coord = match &prop {
                            SGFProperty::B(c) | SGFProperty::W(c) => *c,
                            _ => unreachable!(),
                        };
                        let existing = self
                            .editor
                            .tree
                            .node(self.editor.cursor)
                            .children
                            .iter()
                            .find(|&&id| {
                                self.editor.tree.node(id).properties.iter().any(|p| match p {
                                    SGFProperty::B(c) | SGFProperty::W(c) => *c == move_coord,
                                    _ => false,
                                })
                            })
                            .copied();
                        if let Some(child_id) = existing {
                            // Navigate to existing variation: push board, apply child node
                            self.board_history.push(self.cached_board.clone());
                            self.editor.cursor = child_id;
                            self.cached_board.apply_node(self.editor.tree.node(child_id));
                        } else {
                            // New move: push board, apply command (which advances cursor)
                            self.board_history.push(self.cached_board.clone());
                            self.editor.apply(EditCommand::AddMove(prop));
                            self.cached_board
                                .apply_node(self.editor.tree.node(self.editor.cursor));
                        }
                    }
                    Err(msg) => {
                        self.status_message = Some(StatusMessage::error(msg));
                    }
                }
            }
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
                self.board_history.push(self.cached_board.clone());
                self.editor.apply(EditCommand::AddMove(prop));
                self.cached_board
                    .apply_node(self.editor.tree.node(self.editor.cursor));
            }
            Message::DeleteNodeConfirmed => {
                self.confirm_delete = false;
                self.editor.apply(EditCommand::DeleteCurrentNode);
                self.recompute_board();
            }
            Message::DeleteNodeCancelled => {
                self.confirm_delete = false;
            }
            Message::NavigateNext => {
                let old_cursor = self.editor.cursor;
                self.editor.apply(EditCommand::NavigateNext);
                if self.editor.cursor != old_cursor {
                    // Incremental forward: push current board, apply next node
                    self.board_history.push(self.cached_board.clone());
                    self.cached_board
                        .apply_node(self.editor.tree.node(self.editor.cursor));
                }
            }
            Message::NavigatePrev => {
                let old_cursor = self.editor.cursor;
                self.editor.apply(EditCommand::NavigatePrev);
                if self.editor.cursor != old_cursor {
                    // Incremental backward: pop from history
                    if let Some(prev_board) = self.board_history.pop() {
                        self.cached_board = prev_board;
                    } else {
                        self.recompute_board();
                    }
                }
            }
            Message::NavigateNextVariation => {
                self.editor.apply(EditCommand::NavigateNextVariation);
                self.recompute_board();
            }
            Message::NavigatePrevVariation => {
                self.editor.apply(EditCommand::NavigatePrevVariation);
                self.recompute_board();
            }
            Message::NavigateFirstVariation => {
                self.editor.apply(EditCommand::NavigateFirstVariation);
                self.recompute_board();
            }
            Message::NavigateLastVariation => {
                self.editor.apply(EditCommand::NavigateLastVariation);
                self.recompute_board();
            }
            Message::NavigateFirst => {
                self.editor.apply(EditCommand::NavigateFirst);
                self.recompute_board();
            }
            Message::NavigateLast => {
                self.editor.apply(EditCommand::NavigateLast);
                self.recompute_board();
            }
            Message::NavigateBranch(n) => {
                let old_cursor = self.editor.cursor;
                self.editor.apply(EditCommand::NavigateBranch(n));
                if self.editor.cursor != old_cursor {
                    self.board_history.push(self.cached_board.clone());
                    self.cached_board
                        .apply_node(self.editor.tree.node(self.editor.cursor));
                }
            }
            Message::NavigateToNode(id) => {
                self.editor.apply(EditCommand::NavigateToNode(id));
                self.recompute_board();
            }
            Message::UndoRequested => {
                self.editor.apply(EditCommand::Undo);
                self.recompute_board();
            }
            Message::RedoRequested => {
                self.editor.apply(EditCommand::Redo);
                self.recompute_board();
            }
            Message::NewGameRequested => {
                let tree = new_game_tree();
                self.editor.apply(EditCommand::Load(tree));
                self.file_path = None;
                self.recompute_board();
                self.status_message = None;
            }
            Message::SelectGame(n) => {
                if let Some(&root) = self.editor.tree.roots.get(n) {
                    self.active_game_index = n;
                    self.editor.apply(EditCommand::NavigateToNode(root));
                    self.recompute_board();
                }
            }
            Message::DismissStatus => {
                self.status_message = None;
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        hotkeys::subscription()
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

        // Game tree panel
        let game_root = self.editor.tree.roots
            .get(self.active_game_index)
            .copied()
            .unwrap_or(0);
        let tree_program = TreePanelProgram {
            tree: &self.editor.tree,
            root: game_root,
            cursor: self.editor.cursor,
        };
        let tree_panel = container(
            Canvas::new(tree_program)
                .width(Length::Fixed(200.0))
                .height(Length::Fill),
        )
        .height(Length::Fill);

        let main_row = row![board_container, tree_panel]
            .height(Length::Fill)
            .spacing(4);

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

        let normal_content: Element<'_, Message> = column![
            toolbar,
            main_row,
            nav_row,
            capture_row,
            maybe_status,
        ]
        .spacing(6)
        .padding(8)
        .into();

        if self.confirm_delete {
            let backdrop = container(text(""))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(
                        0.0, 0.0, 0.0, 0.45,
                    ))),
                    ..Default::default()
                });

            let dialog = container(
                column![
                    text("Delete this node and all its descendants?").size(15),
                    row![
                        button("Delete").on_press(Message::DeleteNodeConfirmed),
                        button("Cancel").on_press(Message::DeleteNodeCancelled),
                    ]
                    .spacing(8),
                ]
                .spacing(12)
                .padding(16),
            )
            .style(|_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color::WHITE)),
                border: iced::Border {
                    color: Color::from_rgb(0.5, 0.5, 0.5),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

            let overlay = container(dialog)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center);

            iced::widget::stack([normal_content, backdrop.into(), overlay.into()]).into()
        } else {
            normal_content
        }
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

/// Build a board snapshot *and* a history stack for every position along
/// the root→cursor path. The history allows O(1) backward navigation.
fn board_with_history(tree: &GameTree, cursor: NodeId) -> (Board, Vec<Board>) {
    let mut path: Vec<NodeId> = Vec::new();
    let mut current = cursor;
    loop {
        path.push(current);
        match tree.node(current).parent {
            Some(parent) => current = parent,
            None => break,
        }
    }
    path.reverse();

    let mut board = Board::from_tree(tree, path[0]); // root node
    let mut history: Vec<Board> = Vec::with_capacity(path.len().saturating_sub(1));

    for &id in &path[1..] {
        history.push(board.clone());
        board.apply_node(tree.node(id));
    }

    (board, history)
}

/// Returns the SGFProperty for placing a stone at (col, row), or an error.
fn try_place_stone(app: &GuiApp, col: usize, row: usize) -> Result<SGFProperty, String> {
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
    Ok(prop)
}

/// Simulate placing `color` at `(col, row)` and check if it would be a suicide.
/// Returns true if the group formed would have zero liberties after captures.
fn would_be_suicide(board: &Board, col: usize, row: usize, color: Cell) -> bool {
    let mut sim_cells = board.cells;
    let size = board.size;

    sim_cells[row][col] = color;

    let opponent = match color {
        Cell::Black => Cell::White,
        Cell::White => Cell::Black,
        Cell::Empty => return false,
    };

    // Track which opponent cells have already been checked to avoid
    // re-walking the same group when multiple neighbors belong to it.
    let mut checked = [[false; 19]; 19];

    let nbrs = orthogonal_neighbors(row, col, size);
    for &(nr, nc) in nbrs.as_slice() {
        if sim_cells[nr][nc] != opponent || checked[nr][nc] {
            continue;
        }
        let group = find_group(&sim_cells, nr, nc, size);
        // Mark all group members as checked
        for &(gr, gc) in &group {
            checked[gr][gc] = true;
        }
        if count_liberties(&sim_cells, &group, size) == 0 {
            for (gr, gc) in group {
                sim_cells[gr][gc] = Cell::Empty;
            }
        }
    }

    // After captures, check if the placed group has any liberties
    let placed_group = find_group(&sim_cells, row, col, size);
    count_liberties(&sim_cells, &placed_group, size) == 0
}
