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
    /// Replace the entire tree (e.g. after loading a new file).
    Load(GameTree),
}

fn property_key(p: &SGFProperty) -> &str {
    match p {
        SGFProperty::AP(_) => "AP",
        SGFProperty::B(_) => "B",
        SGFProperty::W(_) => "W",
        SGFProperty::AB(_) => "AB",
        SGFProperty::AW(_) => "AW",
        SGFProperty::CA(_) => "CA",
        SGFProperty::DT(_) => "DT",
        SGFProperty::FF(_) => "FF",
        SGFProperty::GM(_) => "GM",
        SGFProperty::KM(_) => "KM",
        SGFProperty::SZ(_) => "SZ",
        SGFProperty::PB(_) => "PB",
        SGFProperty::PW(_) => "PW",
        SGFProperty::RE(_) => "RE",
        SGFProperty::C(_) => "C",
        SGFProperty::Unknown(k, _) => k.as_str(),
    }
}

impl Editor {
    pub fn new(tree: GameTree) -> Self {
        let cursor = tree.roots.first().copied().unwrap_or(0);
        Self { tree, cursor }
    }

    pub fn apply(&mut self, cmd: EditCommand) {
        match cmd {
            EditCommand::AddMove(prop) => {
                let id = self.tree.add_node(self.cursor, vec![prop]);
                self.cursor = id;
            }
            EditCommand::SetProperty(prop) => {
                let key = property_key(&prop).to_string();
                let node = self.tree.node_mut(self.cursor);
                if let Some(existing) = node.properties.iter_mut().find(|p| property_key(p) == key) {
                    *existing = prop;
                } else {
                    node.properties.push(prop);
                }
            }
            EditCommand::RemoveProperty(key) => {
                self.tree.node_mut(self.cursor).properties.retain(|p| property_key(p) != key);
            }
            EditCommand::DeleteCurrentNode => {
                let old_cursor = self.cursor;
                if let Some(parent) = self.tree.node(self.cursor).parent {
                    self.cursor = parent;
                    self.tree.remove_subtree(old_cursor);
                }
            }
            EditCommand::AppendVariation => {
                self.tree.add_node(self.cursor, vec![]);
            }
            EditCommand::NavigateNext => {
                if let Some(&c) = self.tree.node(self.cursor).children.first() {
                    self.cursor = c;
                }
            }
            EditCommand::NavigatePrev => {
                if let Some(p) = self.tree.node(self.cursor).parent {
                    self.cursor = p;
                }
            }
            EditCommand::NavigateBranch(n) => {
                if let Some(&c) = self.tree.node(self.cursor).children.get(n) {
                    self.cursor = c;
                }
            }
            EditCommand::Load(new_tree) => {
                let cursor = new_tree.roots.first().copied().unwrap_or(0);
                self.tree = new_tree;
                self.cursor = cursor;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sgf::{GameTree, parse_sgf};
    use crate::sgf::node::GoCoord;

    fn simple_tree() -> Editor {
        let tree = parse_sgf("(;GM[1]FF[4]SZ[19];B[dd];W[pd])").unwrap();
        Editor::new(tree)
    }

    #[test]
    fn navigate_next_and_prev() {
        let mut ed = simple_tree();
        let root = ed.cursor;
        ed.apply(EditCommand::NavigateNext);
        assert_ne!(ed.cursor, root);
        ed.apply(EditCommand::NavigatePrev);
        assert_eq!(ed.cursor, root);
    }

    #[test]
    fn navigate_prev_at_root_is_noop() {
        let mut ed = simple_tree();
        let root = ed.cursor;
        ed.apply(EditCommand::NavigatePrev);
        assert_eq!(ed.cursor, root);
    }

    #[test]
    fn navigate_next_at_leaf_is_noop() {
        let mut ed = simple_tree();
        // Advance to the last node.
        ed.apply(EditCommand::NavigateNext);
        ed.apply(EditCommand::NavigateNext);
        let leaf = ed.cursor;
        ed.apply(EditCommand::NavigateNext);
        assert_eq!(ed.cursor, leaf);
    }

    #[test]
    fn add_move_advances_cursor() {
        let mut ed = Editor::new(GameTree::new());
        let start = ed.cursor;
        let coord = GoCoord::new('d', 'd').unwrap();
        ed.apply(EditCommand::AddMove(SGFProperty::B(coord)));
        assert_ne!(ed.cursor, start);
        // New node should have exactly one property.
        assert_eq!(ed.tree.node(ed.cursor).properties.len(), 1);
    }

    #[test]
    fn set_property_upserts() {
        let mut ed = Editor::new(GameTree::new());
        ed.apply(EditCommand::SetProperty(SGFProperty::SZ(19)));
        assert_eq!(ed.tree.node(ed.cursor).properties.len(), 1);
        // Setting same key again replaces it.
        ed.apply(EditCommand::SetProperty(SGFProperty::SZ(9)));
        assert_eq!(ed.tree.node(ed.cursor).properties.len(), 1);
        match &ed.tree.node(ed.cursor).properties[0] {
            SGFProperty::SZ(n) => assert_eq!(*n, 9),
            _ => panic!("expected SZ"),
        }
    }

    #[test]
    fn remove_property() {
        let mut ed = Editor::new(GameTree::new());
        ed.apply(EditCommand::SetProperty(SGFProperty::SZ(19)));
        ed.apply(EditCommand::RemoveProperty("SZ".to_string()));
        assert_eq!(ed.tree.node(ed.cursor).properties.len(), 0);
    }

    #[test]
    fn append_variation_adds_child() {
        let mut ed = simple_tree();
        let root_children_before = ed.tree.node(ed.cursor).children.len();
        ed.apply(EditCommand::AppendVariation);
        assert_eq!(ed.tree.node(ed.cursor).children.len(), root_children_before + 1);
    }

    #[test]
    fn navigate_branch() {
        let mut ed = simple_tree();
        // Add a second variation at root.
        ed.apply(EditCommand::AppendVariation);
        // Branch 0 is the original child, branch 1 is the new empty node.
        let child0 = ed.tree.node(ed.cursor).children[0];
        ed.apply(EditCommand::NavigateBranch(0));
        assert_eq!(ed.cursor, child0);
    }

    #[test]
    fn delete_current_node_retreats_to_parent() {
        let mut ed = simple_tree();
        let root = ed.cursor;
        ed.apply(EditCommand::NavigateNext);
        let child = ed.cursor;
        ed.apply(EditCommand::DeleteCurrentNode);
        assert_eq!(ed.cursor, root);
        // Child should no longer be in root's children list.
        assert!(!ed.tree.node(root).children.contains(&child));
    }

    #[test]
    fn delete_current_node_at_root_is_noop() {
        let mut ed = simple_tree();
        let root = ed.cursor;
        ed.apply(EditCommand::DeleteCurrentNode);
        assert_eq!(ed.cursor, root);
    }

    #[test]
    fn load_replaces_tree() {
        let mut ed = simple_tree();
        let new_tree = parse_sgf("(;GM[1]SZ[9])").unwrap();
        ed.apply(EditCommand::Load(new_tree));
        // Cursor should be on the new root.
        assert_eq!(ed.tree.node(ed.cursor).properties.len(), 2);
    }
}

pub trait Adapter {
    /// Render the current editor state to the adapter's medium.
    fn render(&mut self, editor: &Editor) -> anyhow::Result<()>;
    /// Produce the next command from user input. `None` signals quit.
    fn next_command(&mut self) -> anyhow::Result<Option<EditCommand>>;
}

/// TEA event loop: render → input → update → repeat.
pub fn run_editor(mut editor: Editor, adapter: &mut impl Adapter) -> anyhow::Result<()> {
    loop {
        adapter.render(&editor)?;
        match adapter.next_command()? {
            Some(cmd) => editor.apply(cmd),
            None => break,
        }
    }
    Ok(())
}
