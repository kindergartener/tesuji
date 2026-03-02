use std::collections::HashSet;

use crate::sgf::{
    node::SGFProperty,
    tree::{GameTree, NodeId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Black,
    White,
}

impl Cell {
    fn opposite(self) -> Cell {
        match self {
            Cell::Empty => Cell::Empty,
            Cell::Black => Cell::White,
            Cell::White => Cell::Black,
        }
    }
}

/// `Board` is computed on demand by calling `Board::from_tree` with a
/// reference to the tree and the current cursor `NodeId`.
///
/// Typical usage is something like:
///
/// ```text
/// // After every EditCommand that moves the cursor or modifies the tree, recompute the board and redraw:
/// let board = Board::from_tree(&editor.tree, editor.cursor);
/// render_board(&board);
/// ```
pub struct Board {
    /// `cells[row][col]`, 0-indexed from top-left.
    pub cells: [[Cell; 19]; 19],

    /// Number of moves played to reach this position (not counting setup stones)
    pub move_number: usize,

    /// Board size
    pub size: usize,

    /// Black stones captured by white
    pub captured_white: u16,

    /// White stones captured by black
    pub captured_black: u16,

    /// Forbidden point for simple ko rule. `None` when no active ko.
    pub ko_point: Option<(usize, usize)>,
}

impl Board {
    /// Build a board snapshot for the position at `cursor` in `tree`.
    ///
    /// If `tree` is empty or `cursor` is out of range this returns an
    /// empty board with `move_number = 0`.
    pub fn from_tree(tree: &GameTree, cursor: NodeId) -> Self {
        let mut board = Self {
            cells: [[Cell::Empty; 19]; 19],
            move_number: 0,
            size: 19,
            captured_white: 0,
            captured_black: 0,
            ko_point: None,
        };

        // Get path from root -> cursor:
        // Follow parent ptr upwards -> push each node id onto a stack -> reverse stack
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

        // Apply each node move
        for id in path {
            let node = tree.node(id);
            for prop in &node.properties {
                match prop {
                    SGFProperty::B(coord) => {
                        let row = coord.second() as usize - b'a' as usize;
                        let col = coord.first() as usize - b'a' as usize;
                        board.cells[row][col] = Cell::Black;
                        board.move_number += 1;
                        board.ko_point = board.apply_captures(row, col, Cell::Black);
                    }
                    SGFProperty::W(coord) => {
                        let row = coord.second() as usize - b'a' as usize;
                        let col = coord.first() as usize - b'a' as usize;
                        board.cells[row][col] = Cell::White;
                        board.move_number += 1;
                        board.ko_point = board.apply_captures(row, col, Cell::White);
                    }
                    // Do not increment move counter for setup stones
                    // and clear ko point
                    SGFProperty::AB(coords) => {
                        board.ko_point = None;
                        for coord in coords {
                            let row = coord.second() as usize - b'a' as usize;
                            let col = coord.first() as usize - b'a' as usize;
                            board.cells[row][col] = Cell::Black;
                        }
                    }
                    SGFProperty::AW(coords) => {
                        board.ko_point = None;
                        for coord in coords {
                            let row = coord.second() as usize - b'a' as usize;
                            let col = coord.first() as usize - b'a' as usize;
                            board.cells[row][col] = Cell::White;
                        }
                    }
                    _ => {}
                }
            }
        }

        board
    }

    /// Remove any opponent stones with zero liberties after placing a stone of
    /// `color` at `(placed_row, placed_col)`.
    ///
    /// Returns the ko point if a simple ko arises or `None` otherwise.
    fn apply_captures(
        &mut self,
        placed_row: usize,
        placed_col: usize,
        color: Cell,
    ) -> Option<(usize, usize)> {
        let opponent = color.opposite();
        let mut total_captured: usize = 0;
        let mut last_captured_at: (usize, usize) = (0, 0);

        for (nr, nc) in orthogonal_neighbors(placed_row, placed_col, self.size) {
            if self.cells[nr][nc] != opponent {
                continue;
            }

            let group = find_group(&self.cells, nr, nc, self.size);
            if count_liberties(&self.cells, &group, self.size) > 0 {
                continue;
            }

            // Remove every stone in the group
            let n = group.len();
            if n == 1 {
                last_captured_at = group[0];
            }
            total_captured += n;
            for (gr, gc) in group {
                self.cells[gr][gc] = Cell::Empty;
            }
            match opponent {
                Cell::Black => self.captured_white += n as u16,
                Cell::White => self.captured_black += n as u16,
                Cell::Empty => {}
            }
        }

        // Simple ko
        if total_captured == 1 {
            let placed_group = find_group(&self.cells, placed_row, placed_col, self.size);
            if count_liberties(&self.cells, &placed_group, self.size) == 1 {
                return Some(last_captured_at);
            }
        }

        None
    }
}

/// Returns valid orthogonally adjacent board positions to `(row, col)`
/// respecting edge conditions
fn orthogonal_neighbors(row: usize, col: usize, size: usize) -> Vec<(usize, usize)> {
    let mut neighbors = Vec::with_capacity(4);
    if row > 0 {
        neighbors.push((row - 1, col));
    }
    if row + 1 < size {
        neighbors.push((row + 1, col));
    }
    if col > 0 {
        neighbors.push((row, col - 1));
    }
    if col + 1 < size {
        neighbors.push((row, col + 1));
    }
    neighbors
}

/// Collects all stones of the same color into a connected group by doing
/// a DFS flood-fill from `(row, col)`.
fn find_group(
    cells: &[[Cell; 19]; 19],
    row: usize,
    col: usize,
    size: usize,
) -> Vec<(usize, usize)> {
    let color = cells[row][col];
    let mut visited = [[false; 19]; 19];
    let mut stack = vec![(row, col)];
    let mut group = Vec::new();

    while let Some((r, c)) = stack.pop() {
        if visited[r][c] {
            continue;
        }
        visited[r][c] = true;

        if cells[r][c] != color {
            continue;
        }
        group.push((r, c));

        for (nr, nc) in orthogonal_neighbors(r, c, size) {
            if !visited[nr][nc] && cells[nr][nc] == color {
                stack.push((nr, nc));
            }
        }
    }

    group
}

/// Count a group's liberties
fn count_liberties(cells: &[[Cell; 19]; 19], group: &[(usize, usize)], size: usize) -> usize {
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
        assert_eq!(board.ko_point, None);
    }

    #[test]
    fn single_black_move() {
        let tree = parse_sgf("(;B[dd])").unwrap();
        let cursor = tree
            .node(tree.roots[0])
            .children
            .first()
            .copied()
            .unwrap_or(tree.roots[0]);
        let board = Board::from_tree(&tree, cursor);
        assert_eq!(board.cells[3][3], Cell::Black);
        assert_eq!(board.move_number, 1);
        assert_eq!(board.ko_point, None);
    }

    #[test]
    fn moves_on_mainline_accumulate() {
        let tree = parse_sgf("(;GM[1]FF[4]SZ[19];B[dd];W[pp];B[dp])").unwrap();
        let root = tree.roots[0];
        let n1 = tree.node(root).children[0];
        let n2 = tree.node(n1).children[0];
        let n3 = tree.node(n2).children[0];
        let board = Board::from_tree(&tree, n3);
        assert_eq!(board.cells[3][3], Cell::Black);
        assert_eq!(board.cells[15][15], Cell::White);
        assert_eq!(board.cells[15][3], Cell::Black);
        assert_eq!(board.move_number, 3);
    }

    #[test]
    fn variation_branch_only_shows_branch_moves() {
        let tree = parse_sgf("(;GM[1]FF[4]SZ[19];B[dd](;W[pp])(;W[dp]))").unwrap();
        let root = tree.roots[0];
        let b_node = tree.node(root).children[0];
        let var_node = tree.node(b_node).children[1];
        let board = Board::from_tree(&tree, var_node);
        assert_eq!(board.cells[15][15], Cell::Empty);
        assert_eq!(board.cells[15][3], Cell::White);
    }

    #[test]
    fn setup_stones_do_not_increment_move_number() {
        let tree = parse_sgf("(;AB[dd][pp]AW[dp])").unwrap();
        let board = Board::from_tree(&tree, tree.roots[0]);
        assert_eq!(board.cells[3][3], Cell::Black);
        assert_eq!(board.cells[15][15], Cell::Black);
        assert_eq!(board.cells[15][3], Cell::White);
        assert_eq!(board.move_number, 0);
        assert_eq!(board.ko_point, None);
    }

    #[test]
    fn capture_removes_surrounded_group() {
        let tree = parse_sgf("(;AW[bb]AB[ab][ba][bc];B[cb])").unwrap();
        let root = tree.roots[0];
        let move_node = tree.node(root).children[0];
        let board = Board::from_tree(&tree, move_node);
        assert_eq!(board.cells[1][1], Cell::Empty);
        assert_eq!(board.cells[1][0], Cell::Black);
        assert_eq!(board.cells[0][1], Cell::Black);
        assert_eq!(board.cells[2][1], Cell::Black);
        assert_eq!(board.cells[1][2], Cell::Black);
        assert_eq!(board.captured_black, 1);
        assert_eq!(board.captured_white, 0);
        assert_eq!(board.ko_point, None);
    }

    #[test]
    fn no_ko_when_multiple_stones_captured() {
        let tree = parse_sgf("(;AW[bb][dd]AB[ab][ba][bc][dc][cd][ed];B[cb];B[de])").unwrap();
        let root = tree.roots[0];
        let n1 = tree.node(root).children[0];
        let n2 = tree.node(n1).children[0];
        let board = Board::from_tree(&tree, n2);
        assert_eq!(board.captured_black, 2);
        assert_eq!(board.ko_point, None);
    }

    #[test]
    fn simple_ko_detected() {
        let tree = parse_sgf("(;AW[ff][ee][eg][df]AB[fe][fg][gf];B[ef])").unwrap();
        let root = tree.roots[0];
        let move_node = tree.node(root).children[0];
        let board = Board::from_tree(&tree, move_node);
        assert_eq!(board.cells[5][5], Cell::Empty);
        assert_eq!(board.cells[5][4], Cell::Black);
        assert_eq!(board.captured_black, 1);
        assert_eq!(board.ko_point, Some((5, 5)));
    }
}
