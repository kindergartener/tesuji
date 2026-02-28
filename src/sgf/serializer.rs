use std::fmt::Write as _;

use crate::sgf::{GameTree, NodeId, SGFProperty};

/// Serialise an entire [`GameTree`] back to SGF text.
///
/// Each top-level game record (root) is written as its own `(…)` collection.
/// Multiple records are concatenated with no separator.
pub fn write_sgf(tree: &GameTree) -> String {
    let mut out = String::new();
    for &root in &tree.roots {
        write_game(tree, root, &mut out);
    }
    out
}

/// Write one top-level game record rooted at `root` into `out`.
fn write_game(tree: &GameTree, root: NodeId, out: &mut String) {
    out.push('(');
    write_node(tree, root, out);
    out.push(')');
}

/// Write a single node (`;` followed by its properties) and then recurse.
///
/// Branching rules:
/// - **0 children** — nothing more to emit.
/// - **1 child** — continue inline (no extra parentheses).
/// - **2+ children** — each child is a separate variation, wrapped in `(…)`.
fn write_node(tree: &GameTree, id: NodeId, out: &mut String) {
    out.push(';');
    for prop in &tree.node(id).properties {
        write_property(prop, out);
    }
    let children: Vec<NodeId> = tree.node(id).children.clone();
    match children.len() {
        0 => {}
        1 => write_node(tree, children[0], out),
        _ => {
            for child in children {
                out.push('(');
                write_node(tree, child, out);
                out.push(')');
            }
        }
    }
}

/// Append the SGF text for one property to `out`.
fn write_property(prop: &SGFProperty, out: &mut String) {
    write!(out, "{}", prop).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::sgf::{parse_sgf, write_sgf};

    /// Parse → serialize → re-parse and check that node count and root count match.
    #[test]
    fn round_trip_node_count() {
        let sgf = "(;GM[1]FF[4]SZ[19];B[dd];W[pd];B[dp])";
        let tree1 = parse_sgf(sgf).unwrap();
        let serialized = write_sgf(&tree1);
        let tree2 = parse_sgf(&serialized).unwrap();
        assert_eq!(tree1.roots.len(), tree2.roots.len());
        // Both trees should have the same number of nodes (4 here).
        assert_eq!(
            tree1.iter_subtree(tree1.roots[0]).count(),
            tree2.iter_subtree(tree2.roots[0]).count(),
        );
    }

    /// Verify that a game with variations round-trips correctly.
    #[test]
    fn round_trip_with_variations() {
        let sgf = "(;GM[1]FF[4]SZ[19];B[dd](;W[pd])(;W[dp]))";
        let tree1 = parse_sgf(sgf).unwrap();
        let serialized = write_sgf(&tree1);
        let tree2 = parse_sgf(&serialized).unwrap();
        // Root node should have one child (the B[dd] node) which itself has two children.
        let root2 = tree2.roots[0];
        let b_node = tree2.node(root2).children[0];
        assert_eq!(tree2.node(b_node).children.len(), 2);
    }
}
