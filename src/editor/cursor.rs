use crate::sgf::{GameTree, MainlineIter, NodeId, SubtreeIter, TreeNode};

pub struct TreeCursor<'a> {
    tree: &'a GameTree,
    current: NodeId,
}

impl<'a> TreeCursor<'a> {
    pub fn new(tree: &'a GameTree, start: NodeId) -> Self {
        Self { tree, current: start }
    }

    pub fn node_id(&self) -> NodeId {
        self.current
    }

    pub fn current(&self) -> &'a TreeNode {
        self.tree.node(self.current)
    }

    /// Move to the first child. Returns `false` if already at a leaf.
    pub fn advance(&mut self) -> bool {
        if let Some(&child) = self.tree.node(self.current).children.first() {
            self.current = child;
            true
        } else {
            false
        }
    }

    /// Move to the parent. Returns `false` if already at the root.
    pub fn retreat(&mut self) -> bool {
        if let Some(parent) = self.tree.node(self.current).parent {
            self.current = parent;
            true
        } else {
            false
        }
    }

    /// Move to the nth child. Returns `false` if the index is out of range.
    pub fn branch(&mut self, idx: usize) -> bool {
        if let Some(&child) = self.tree.node(self.current).children.get(idx) {
            self.current = child;
            true
        } else {
            false
        }
    }

    pub fn iter_mainline(&self) -> MainlineIter<'a> {
        self.tree.iter_mainline(self.current)
    }

    pub fn iter_subtree(&self) -> SubtreeIter<'a> {
        self.tree.iter_subtree(self.current)
    }
}
