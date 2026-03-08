//! SGF data model, parser, serializer, and board simulator.
//!
//! The primary entry points are [`parse_sgf`] (text → [`GameTree`]) and
//! [`write_sgf`] ([`GameTree`] → text).  [`Board::from_tree`] replays the
//! move sequence from the root down to any cursor node to produce a concrete
//! board position.
//!
//! ## Key types
//!
//! - [`SGFProperty`] — a single SGF property (e.g. `B[dd]`, `KM[6.5]`).
//! - [`GameTree`] — arena-allocated tree of [`TreeNode`]s indexed by [`NodeId`].
//! - [`Board`] — a Go board position derived from a tree path via [`Board::from_tree`].
//! - [`node::GoCoord`] — a pair of SGF board coordinates (e.g. `dd`).

pub mod board;
pub mod node;
mod parser;
mod serializer;
pub mod tree;

pub use board::{Board, Cell, Neighbors, count_liberties, find_group, orthogonal_neighbors};
pub use node::SGFProperty;
pub use parser::parse_sgf;
pub use serializer::write_sgf;
pub use tree::{GameTree, MainlineIter, NodeId, SubtreeIter, TreeNode};
