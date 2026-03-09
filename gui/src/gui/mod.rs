pub mod assets;
pub mod board;
pub mod hotkeys;
pub mod io;
pub mod theme;
pub mod tree_panel;

use std::path::PathBuf;

use iced::{
    Color, Element, Length, Task,
    widget::{Space, button, canvas::Canvas, column, container, row, rule, text},
};

use tesuji::sgf::node::GoCoord;
use tesuji::sgf::{
    Board, Cell, GameTree, NodeId, SGFProperty, count_liberties, find_group, orthogonal_neighbors,
};
use tesuji::{EditCommand, Editor, parse_sgf, write_sgf};

use crate::gui::{
    assets::BoardAssets,
    board::{BoardWidget, current_player},
    tree_panel::TreePanelProgram,
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
    pub show_fps: bool,
    pub assets: BoardAssets,
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
        Self {
            text: text.into(),
            kind: StatusKind::Info,
        }
    }
    fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Warning,
        }
    }
    fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Error,
        }
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
    BoardClicked {
        col: usize,
        row: usize,
    },
    BoardHovered {
        col: Option<usize>,
        row: Option<usize>,
    },
    BoardResized {
        cell_size: f32,
    },
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
    ToggleFps,
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
                show_fps: false,
                assets: BoardAssets::load(),
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
                self.status_message =
                    Some(StatusMessage::info(format!("Saved to {}", path.display())));
            }
            Message::FileSaved(Err(e)) => {
                self.status_message = Some(StatusMessage::error(format!("Save failed: {e}")));
            }
            Message::BoardClicked { col, row } => {
                // Check if clicking on the last-move marker -> trigger delete confirmation
                if let Some((last_col, last_row)) = last_move_coord(&self.editor)
                    && col == last_col
                    && row == last_row
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
                                self.editor
                                    .tree
                                    .node(id)
                                    .properties
                                    .iter()
                                    .any(|p| match p {
                                        SGFProperty::B(c) | SGFProperty::W(c) => *c == move_coord,
                                        _ => false,
                                    })
                            })
                            .copied();
                        if let Some(child_id) = existing {
                            // Navigate to existing variation: push board, apply child node
                            self.board_history.push(self.cached_board.clone());
                            self.editor.cursor = child_id;
                            self.cached_board
                                .apply_node(self.editor.tree.node(child_id));
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
            Message::BoardResized { cell_size: _ } => {
                // Reserved for future use (e.g. resolution-dependent rendering).
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
            Message::ToggleFps => {
                self.show_fps = !self.show_fps;
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        hotkeys::subscription()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let board = &self.cached_board;
        let player = current_player(board);

        // Compute last move coord from the current cursor node
        let last_move = last_move_coord(&self.editor);

        // ── Left column: board widget ──
        let board_widget = BoardWidget {
            board,
            hover: self.hover_coord,
            last_move,
            show_fps: self.show_fps,
            assets: &self.assets,
        };
        let board_element: Element<'_, Message> = board_widget.into();
        let left_col = container(board_element)
            .width(Length::Fill)
            .height(Length::Fill);

        // ── Right column: info panels ──
        let game_root = self
            .editor
            .tree
            .roots
            .get(self.active_game_index)
            .copied()
            .unwrap_or(0);
        let info = extract_game_info(&self.editor.tree, game_root);

        // Game info panel
        let game_info_panel = self.view_game_info(&info, board, player);

        // Game tree panel
        let tree_program = TreePanelProgram {
            tree: &self.editor.tree,
            root: game_root,
            cursor: self.editor.cursor,
        };
        let tree_panel = container(
            Canvas::new(tree_program)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(|_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::from_rgb(0.93, 0.93, 0.93))),
            ..Default::default()
        })
        .height(Length::Fill);

        // Engine console placeholder
        let engine_panel = container(text("Engine Console").size(12).color(theme::INFO_LABEL))
            .padding(theme::PANEL_PADDING);

        // Game controls panel
        let controls_panel = self.view_game_controls();

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
            Space::new().into()
        };

        let right_col: Element<'_, Message> = column![
            game_info_panel,
            rule::horizontal(1),
            tree_panel,
            rule::horizontal(1),
            engine_panel,
            rule::horizontal(1),
            controls_panel,
            maybe_status,
        ]
        .spacing(theme::PANEL_SPACING)
        .width(Length::Fixed(theme::RIGHT_PANEL_WIDTH))
        .into();

        let main_row = row![left_col, right_col]
            .height(Length::Fill)
            .spacing(theme::PANEL_SPACING);

        let normal_content: Element<'_, Message> = container(main_row)
            .padding(theme::PANEL_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
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

    /// Build the game info panel widget.
    ///
    /// Layout:
    /// ```text
    /// <w-name> <w-rank> ● ○ <b-rank> <b-name>
    ///     <w-capt>   Captured   <b-capt>
    ///     <w-hand>   Handicap   <b-hand>
    ///             <komi>   Komi
    /// ```
    fn view_game_info(
        &self,
        info: &GameInfo,
        board: &Board,
        current: Cell,
    ) -> Element<'_, Message> {
        let white_active = current == Cell::White;
        let black_active = current == Cell::Black;

        let w_stone_size = if white_active {
            theme::ACTIVE_STONE_SIZE
        } else {
            theme::INACTIVE_STONE_SIZE
        };
        let b_stone_size = if black_active {
            theme::ACTIVE_STONE_SIZE
        } else {
            theme::INACTIVE_STONE_SIZE
        };

        let w_name: String = if info.white_name.is_empty() {
            "White".into()
        } else {
            info.white_name.clone()
        };
        let b_name: String = if info.black_name.is_empty() {
            "Black".into()
        } else {
            info.black_name.clone()
        };

        // Row 1: <w-name> <w-rank> ● ○ <b-rank> <b-name>
        let mut player_row = row![].spacing(4).align_y(iced::Alignment::Center);

        player_row = player_row.push(text(w_name).size(13));
        if !info.white_rank.is_empty() {
            player_row = player_row.push(
                text(info.white_rank.clone())
                    .size(11)
                    .color(theme::INFO_LABEL),
            );
        }
        player_row = player_row.push(text("●").size(w_stone_size));
        player_row = player_row.push(Space::new().width(Length::Fill));
        player_row = player_row.push(text("○").size(b_stone_size));
        if !info.black_rank.is_empty() {
            player_row = player_row.push(
                text(info.black_rank.clone())
                    .size(11)
                    .color(theme::INFO_LABEL),
            );
        }
        player_row = player_row.push(text(b_name).size(13));

        // Row 2: captures
        let capture_row = row![
            text(board.captured_black.to_string()).size(13),
            Space::new().width(Length::Fill),
            text("Captured").size(11).color(theme::INFO_LABEL),
            Space::new().width(Length::Fill),
            text(board.captured_white.to_string()).size(13),
        ]
        .align_y(iced::Alignment::Center);

        // Row 3: handicap (only if present)
        let handicap_row: Option<Element<'_, Message>> = info.handicap.map(|h| {
            row![
                text(h.to_string()).size(13),
                Space::new().width(Length::Fill),
                text("Handicap").size(11).color(theme::INFO_LABEL),
                Space::new().width(Length::Fill),
                text(h.to_string()).size(13),
            ]
            .align_y(iced::Alignment::Center)
            .into()
        });

        // Row 4: komi
        let komi_row = row![
            Space::new().width(Length::Fill),
            text(info.komi.clone()).size(13),
            text("  Komi").size(11).color(theme::INFO_LABEL),
            Space::new().width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center);

        // Move info
        let move_info = row![
            text(format!("Move {}", board.move_number))
                .size(12)
                .color(theme::INFO_LABEL),
        ];

        let mut col = column![player_row, capture_row].spacing(4);
        if let Some(h) = handicap_row {
            col = col.push(h);
        }
        col = col.push(komi_row);
        col = col.push(move_info);

        container(col)
            .padding(theme::PANEL_PADDING)
            .width(Length::Fill)
            .into()
    }

    /// Build the game controls panel.
    fn view_game_controls(&self) -> Element<'_, Message> {
        let top_row = row![
            button("Pass").on_press(Message::PassRequested),
            button("◀").on_press(Message::NavigatePrev),
            button("▶").on_press(Message::NavigateNext),
        ]
        .spacing(4);

        let bottom_row = row![
            button("Open").on_press(Message::OpenFileRequested),
            button("Save").on_press(Message::SaveFileRequested),
            button("Save As").on_press(Message::SaveAsRequested),
            button("New").on_press(Message::NewGameRequested),
        ]
        .spacing(4);

        container(column![top_row, bottom_row].spacing(4))
            .padding(theme::PANEL_PADDING)
            .width(Length::Fill)
            .into()
    }
}

/// Game metadata extracted from the root node's SGF properties.
struct GameInfo {
    white_name: String,
    black_name: String,
    white_rank: String,
    black_rank: String,
    komi: String,
    handicap: Option<u8>,
    result: String,
}

/// Extract game info from the root node of the active game.
fn extract_game_info(tree: &GameTree, root: NodeId) -> GameInfo {
    let mut info = GameInfo {
        white_name: String::new(),
        black_name: String::new(),
        white_rank: String::new(),
        black_rank: String::new(),
        komi: String::new(),
        handicap: None,
        result: String::new(),
    };
    for prop in &tree.node(root).properties {
        match prop {
            SGFProperty::PW(s) => info.white_name = s.clone(),
            SGFProperty::PB(s) => info.black_name = s.clone(),
            SGFProperty::WR(s) => info.white_rank = s.clone(),
            SGFProperty::BR(s) => info.black_rank = s.clone(),
            SGFProperty::KM(k) => info.komi = k.to_string(),
            SGFProperty::HA(n) => info.handicap = Some(*n),
            SGFProperty::RE(s) => info.result = s.clone(),
            _ => {}
        }
    }
    info
}

fn new_game_tree() -> GameTree {
    use tesuji::sgf::node::{FileFormat, GameType, Komi};
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
