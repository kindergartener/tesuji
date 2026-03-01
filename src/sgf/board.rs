use crate::sgf::{
    node::SGFProperty,
    tree::{GameTree, NodeId},
};

// ---------------------------------------------------------------------------
// Cell
// ---------------------------------------------------------------------------

/// The occupancy state of a single intersection on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Black,
    White,
}

// ---------------------------------------------------------------------------
// Board
// ---------------------------------------------------------------------------

/// A snapshot of the board position at a specific node in the game tree.
///
/// # Relationship to `GameTree` and `Editor`
///
/// `Board` is **derived state** — it does not live inside the `GameTree` or
/// `Editor`. It is computed on demand by calling [`Board::from_tree`] with a
/// reference to the tree and the current cursor `NodeId`.
///
/// In a GUI or TUI setting the typical usage pattern is:
///
/// ```text
/// // After every EditCommand that moves the cursor or modifies the tree,
/// // recompute the board and redraw:
/// let board = Board::from_tree(&editor.tree, editor.cursor);
/// render_board(&board);
/// ```
///
/// Recomputing on every cursor change is cheap for 19×19 because it is a
/// single linear walk up the parent chain (depth ≤ ~300 for a full game)
/// followed by a forward replay over the same slice. If profiling shows this
/// to be a bottleneck a cached `Option<Board>` can be stored alongside the
/// `Editor` and invalidated whenever `editor.cursor` changes.
///
/// # What this board does NOT model
///
/// Stone **captures** are not yet applied. The cells reflect only placement
/// moves (`B`, `W`) and setup stones (`AB`, `AW`); captured groups are left
/// on the board. See `BOARD-TODO.md` for the planned capture and ko
/// implementation.
///
/// # Coordinate system
///
/// `cells[row][col]` where both `row` and `col` are 0-indexed from the
/// top-left corner of the board, matching the SGF convention that `'a'` on
/// each axis maps to 0:
///
/// ```text
///          col
///      0  1  2  …  18
/// row 0  .  .  .  …  .
///     1  .  .  .  …  .
///     …
///    18  .  .  .  …  .
/// ```
///
/// A `GoCoord` maps to `cells[coord.second() as usize - 'a' as usize]
///                             [coord.first()  as usize - 'a' as usize]`.
/// The helpers on `GoCoord` already return `char` values; subtract `'a'` to
/// get the 0-based index.
pub struct Board {
    /// `cells[row][col]`, 0-indexed from top-left.
    pub cells: [[Cell; 19]; 19],

    /// Number of moves played to reach this position (not counting setup
    /// stones placed via `AB`/`AW`). Useful for displaying the move number
    /// in the UI and for identifying whose turn it is (`Black` plays on even
    /// move numbers if `move_number` is 0-based, assuming no handicap).
    pub move_number: usize,
}

impl Board {
    /// Build a board snapshot for the position at `cursor` in `tree`.
    ///
    /// The algorithm:
    /// 1. Walk parent pointers from `cursor` up to the root, collecting the
    ///    path of `NodeId`s in reverse order.
    /// 2. Replay each node's properties forward, placing stones as they
    ///    appear.
    ///
    /// Properties applied:
    /// - `AB` / `AW` — setup stones (handicap, diagram positions). These do
    ///   not increment `move_number`.
    /// - `B` / `W`   — move stones. Each increments `move_number` by 1.
    ///
    /// Properties **not** applied (yet): captures. See `BOARD-TODO.md`.
    ///
    /// If `tree` is empty or `cursor` is out of range this returns an
    /// all-empty board with `move_number = 0`.
    pub fn from_tree(tree: &GameTree, cursor: NodeId) -> Self {
        let mut board = Self {
            cells: [[Cell::Empty; 19]; 19],
            move_number: 0,
        };

        // --- Step 1: collect the path from root down to cursor -------------
        //
        // We follow parent pointers from `cursor` upward, pushing each id
        // onto a stack, then reverse so we replay root → cursor.
        let mut path: Vec<NodeId> = Vec::new();
        let mut current = cursor;
        loop {
            path.push(current);
            match tree.node(current).parent {
                Some(parent) => current = parent,
                None => break,
            }
        }
        path.reverse(); // now ordered root → cursor

        // --- Step 2: replay each node forward -------------------------------
        for id in path {
            let node = tree.node(id);
            for prop in &node.properties {
                match prop {
                    // Single placement moves — advance move counter.
                    SGFProperty::B(coord) => {
                        let row = coord.second() as usize - b'a' as usize;
                        let col = coord.first()  as usize - b'a' as usize;
                        board.cells[row][col] = Cell::Black;
                        board.move_number += 1;
                    }
                    SGFProperty::W(coord) => {
                        let row = coord.second() as usize - b'a' as usize;
                        let col = coord.first()  as usize - b'a' as usize;
                        board.cells[row][col] = Cell::White;
                        board.move_number += 1;
                    }
                    // Setup stones — do not count as moves.
                    SGFProperty::AB(coords) => {
                        for coord in coords {
                            let row = coord.second() as usize - b'a' as usize;
                            let col = coord.first()  as usize - b'a' as usize;
                            board.cells[row][col] = Cell::Black;
                        }
                    }
                    SGFProperty::AW(coords) => {
                        for coord in coords {
                            let row = coord.second() as usize - b'a' as usize;
                            let col = coord.first()  as usize - b'a' as usize;
                            board.cells[row][col] = Cell::White;
                        }
                    }
                    _ => {}
                }
            }
        }

        board
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sgf::parse_sgf;

    #[test]
    fn empty_tree_gives_empty_board() {
        let tree = GameTree::new();
        let board = Board::from_tree(&tree, tree.roots[0]);
        assert!(board.cells.iter().flatten().all(|&c| c == Cell::Empty));
        assert_eq!(board.move_number, 0);
    }

    #[test]
    fn single_black_move() {
        // B[dd] — column 'd' (3), row 'd' (3)
        let tree = parse_sgf("(;B[dd])").unwrap();
        let cursor = tree.node(tree.roots[0]).children.first().copied().unwrap_or(tree.roots[0]);
        let board = Board::from_tree(&tree, cursor);
        assert_eq!(board.cells[3][3], Cell::Black);
        assert_eq!(board.move_number, 1);
    }

    #[test]
    fn moves_on_mainline_accumulate() {
        // Three-move sequence: B[dd], W[pp], B[dp]
        let tree = parse_sgf("(;GM[1]FF[4]SZ[19];B[dd];W[pp];B[dp])").unwrap();
        // Advance cursor to the last node.
        let root = tree.roots[0];
        let n1 = tree.node(root).children[0];
        let n2 = tree.node(n1).children[0];
        let n3 = tree.node(n2).children[0];
        let board = Board::from_tree(&tree, n3);
        assert_eq!(board.cells[3][3], Cell::Black);    // dd — col d=3, row d=3
        assert_eq!(board.cells[15][15], Cell::White); // pp — col p=15, row p=15
        assert_eq!(board.cells[15][3], Cell::Black);  // dp — col d=3, row p=15
        assert_eq!(board.move_number, 3);
    }

    #[test]
    fn variation_branch_only_shows_branch_moves() {
        // Root → B[dd] → (W[pp] | W[dp])
        // Board at the W[dp] branch should NOT contain W[pp].
        let tree = parse_sgf("(;GM[1]FF[4]SZ[19];B[dd](;W[pp])(;W[dp]))").unwrap();
        let root = tree.roots[0];
        let b_node = tree.node(root).children[0];
        // children[1] is the W[dp] variation
        let var_node = tree.node(b_node).children[1];
        let board = Board::from_tree(&tree, var_node);
        assert_eq!(board.cells[15][15], Cell::Empty); // W[pp] NOT played
        assert_eq!(board.cells[15][3], Cell::White);  // W[dp] IS played — col d=3, row p=15
    }

    #[test]
    fn setup_stones_do_not_increment_move_number() {
        let tree = parse_sgf("(;AB[dd][pp]AW[dp])").unwrap();
        let board = Board::from_tree(&tree, tree.roots[0]);
        assert_eq!(board.cells[3][3], Cell::Black);   // dd — col d=3, row d=3
        assert_eq!(board.cells[15][15], Cell::Black); // pp — col p=15, row p=15
        assert_eq!(board.cells[15][3], Cell::White);  // dp — col d=3, row p=15
        assert_eq!(board.move_number, 0);
    }
}
