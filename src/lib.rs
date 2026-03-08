//! SGF parser, serializer, and game-tree editor for Go.
//!
//! `tesuji` reads and writes [Smart Game Format (SGF)][sgf] files, models them
//! as an arena-based game tree, and provides a command-pattern [`Editor`] for
//! navigating and mutating that tree.  The same core is used by the companion
//! `tesuji-gui` desktop application, but the library itself has no GUI dependency.
//!
//! [sgf]: https://www.red-bean.com/sgf/
//!
//! # Quick start
//!
//! ## Parse and replay a game
//!
//! ```
//! use tesuji::{parse_sgf, Editor, EditCommand};
//! use tesuji::sgf::Board;
//!
//! let tree = parse_sgf("(;GM[1]FF[4]SZ[19];B[dd];W[pd])").unwrap();
//! let mut editor = Editor::new(tree);
//!
//! editor.apply(EditCommand::NavigateNext); // cursor → B[dd]
//! editor.apply(EditCommand::NavigateNext); // cursor → W[pd]
//!
//! let board = Board::from_tree(&editor.tree, editor.cursor);
//! assert_eq!(board.move_number, 2);
//! ```
//!
//! ## Add a move and undo it
//!
//! ```
//! use tesuji::{parse_sgf, Editor, EditCommand};
//! use tesuji::sgf::SGFProperty;
//! use tesuji::sgf::node::GoCoord;
//!
//! let tree = parse_sgf("(;GM[1]SZ[19])").unwrap();
//! let mut editor = Editor::new(tree);
//!
//! let coord = GoCoord::new('d', 'd').unwrap();
//! editor.apply(EditCommand::AddMove(SGFProperty::B(coord)));
//! let after_add = editor.cursor;
//!
//! editor.apply(EditCommand::Undo);
//! assert_ne!(editor.cursor, after_add); // cursor rolled back to root
//! ```
//!
//! # Crate layout
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`sgf`] | [`sgf::SGFProperty`], [`GameTree`], [`parse_sgf`], [`write_sgf`], [`sgf::Board`] |
//! | [`editor`] | [`Editor`], [`EditCommand`], [`Adapter`], [`run_editor`] |

#[cfg(feature = "cli")]
pub mod cli;
pub mod editor;
pub mod sgf;

pub use editor::{Adapter, EditCommand, Editor, run_editor};
pub use sgf::{GameTree, parse_sgf, write_sgf};
