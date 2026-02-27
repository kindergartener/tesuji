mod cursor;
pub use cursor::TreeCursor;

use crate::sgf::{GameTree, NodeId, SGFProperty};

pub struct Editor {
    pub tree: GameTree,
    pub cursor: NodeId,
}

pub enum EditCommand {
    AddMove(SGFProperty),
    SetProperty(SGFProperty),
    RemoveProperty(String),
    DeleteCurrentNode,
    AppendVariation,
    NavigateNext,
    NavigatePrev,
    NavigateBranch(usize),
}
