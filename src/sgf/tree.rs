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
    /// Create an empty game tree with a single empty root node.
    pub fn new() -> Self {
        let root = TreeNode { properties: Vec::new(), parent: None, children: Vec::new() };
        GameTree { nodes: vec![root], roots: vec![0] }
    }

    pub fn node(&self, id: NodeId) -> &TreeNode {
        &self.nodes[id]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut TreeNode {
        &mut self.nodes[id]
    }

    /// Append a new child node under `parent` and return its `NodeId`.
    pub fn add_node(&mut self, parent: NodeId, props: Vec<SGFProperty>) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(TreeNode { properties: props, parent: Some(parent), children: Vec::new() });
        self.nodes[parent].children.push(id);
        id
    }

    /// Unlink `id` from its parent's children list.
    ///
    /// The orphaned nodes remain in the arena (`nodes` Vec) and are never
    /// reclaimed, so the Vec only ever grows. For typical SGF editing sessions
    /// this is negligible (a full 300-move game is ~300 nodes). If heavy
    /// editing (repeated delete/add cycles) becomes a real use-case, a
    /// compaction pass or a free-list could be introduced: keep a
    /// `Vec<NodeId>` of recycled slots and hand them out in `add_node` before
    /// pushing to the end of the Vec.
    pub fn remove_subtree(&mut self, id: NodeId) {
        if let Some(parent) = self.nodes[id].parent {
            self.nodes[parent].children.retain(|&c| c != id);
        }
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
