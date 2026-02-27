use crate::sgf::node::SGFProperty;

pub type NodeId = usize;

pub struct TreeNode {
    pub properties: Vec<SGFProperty>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}

pub struct GameTree {
    /// Arena storage — private to external users; pub(crate) for the parser.
    pub(crate) nodes: Vec<TreeNode>,
    /// One root NodeId per top-level game record in the file.
    pub roots: Vec<NodeId>,
}

impl GameTree {
    pub fn node(&self, id: NodeId) -> &TreeNode {
        &self.nodes[id]
    }

    pub fn iter_mainline(&self, start: NodeId) -> MainlineIter<'_> {
        MainlineIter { tree: self, current: Some(start) }
    }

    pub fn iter_subtree(&self, start: NodeId) -> SubtreeIter<'_> {
        SubtreeIter { tree: self, stack: vec![start] }
    }
}

/// Follows the first child of each node (the main line of play).
pub struct MainlineIter<'a> {
    tree: &'a GameTree,
    current: Option<NodeId>,
}

impl<'a> Iterator for MainlineIter<'a> {
    type Item = (NodeId, &'a TreeNode);

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        let node = &self.tree.nodes[id];
        self.current = node.children.first().copied();
        Some((id, node))
    }
}

/// DFS pre-order traversal over every node reachable from a given root.
pub struct SubtreeIter<'a> {
    tree: &'a GameTree,
    stack: Vec<NodeId>,
}

impl<'a> Iterator for SubtreeIter<'a> {
    type Item = (NodeId, &'a TreeNode);

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let node = &self.tree.nodes[id];
        // Push children in reverse so the leftmost child is visited first.
        self.stack.extend(node.children.iter().rev().copied());
        Some((id, node))
    }
}
