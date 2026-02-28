use std::{
    io::{self, BufRead, Write as _},
    path::PathBuf,
};

use anyhow::Result;

use crate::{
    editor::{Adapter, EditCommand, Editor, run_editor},
    sgf::{GameTree, write_sgf},
};

pub struct CliAdapter {
    file_path: Option<PathBuf>,
    last_sgf: String,
    last_tree_display: String,
}

impl CliAdapter {
    fn new(file_path: Option<PathBuf>) -> Self {
        Self { file_path, last_sgf: String::new(), last_tree_display: String::new() }
    }
}

fn render_tree(tree: &GameTree, cursor: usize) -> String {
    let mut out = String::new();
    if let Some(&root) = tree.roots.first() {
        render_node(tree, root, 0, cursor, &mut out);
    }
    out
}

fn render_node(tree: &GameTree, id: usize, depth: usize, cursor: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let marker = if id == cursor { "* " } else { "  " };
    let node = tree.node(id);
    let label: String = node.properties.iter().map(|p| format!("{}", p)).collect::<Vec<_>>().join(" ");
    out.push_str(&format!("{}{}[{}] {}\n", indent, marker, id, label));
    for &child in &node.children {
        render_node(tree, child, depth + 1, cursor, out);
    }
}

impl Adapter for CliAdapter {
    fn render(&mut self, editor: &Editor) -> Result<()> {
        let node = editor.tree.node(editor.cursor);
        println!("--- node {} ---", editor.cursor);
        for prop in &node.properties {
            println!("  {}", prop);
        }
        // Cache SGF and tree display for save/tree commands
        self.last_sgf = write_sgf(&editor.tree);
        self.last_tree_display = render_tree(&editor.tree, editor.cursor);
        Ok(())
    }

    fn next_command(&mut self) -> Result<Option<EditCommand>> {
        let stdin = io::stdin();
        loop {
            print!("> ");
            io::stdout().flush()?;

            let mut line = String::new();
            let n = stdin.lock().read_line(&mut line)?;
            if n == 0 {
                return Ok(None); // EOF
            }

            let line = line.trim();
            let (cmd, rest) = match line.split_once(' ') {
                Some((c, r)) => (c, r.trim()),
                None => (line, ""),
            };

            match cmd {
                "quit" => return Ok(None),
                "show" => continue, // render already called
                "next" => return Ok(Some(EditCommand::NavigateNext)),
                "prev" => return Ok(Some(EditCommand::NavigatePrev)),
                "branch" => {
                    let n: usize = rest.parse().unwrap_or(0);
                    return Ok(Some(EditCommand::NavigateBranch(n)));
                }
                "tree" => {
                    print!("{}", self.last_tree_display);
                    continue;
                }
                "load" => {
                    if rest.is_empty() {
                        eprintln!("usage: load <path>");
                        continue;
                    }
                    let content = std::fs::read_to_string(rest)?;
                    let tree = crate::sgf::parse_sgf(&content)?;
                    self.file_path = Some(PathBuf::from(rest));
                    return Ok(Some(EditCommand::Load(tree)));
                }
                "save" => {
                    let path = if rest.is_empty() {
                        match &self.file_path {
                            Some(p) => p.clone(),
                            None => {
                                eprintln!("usage: save <path>");
                                continue;
                            }
                        }
                    } else {
                        PathBuf::from(rest)
                    };
                    std::fs::write(&path, &self.last_sgf)?;
                    println!("saved to {}", path.display());
                    continue;
                }
                "" => continue,
                _ => {
                    eprintln!("unknown command: {}", line);
                    continue;
                }
            }
        }
    }
}

pub fn run(args: &[String]) -> Result<()> {
    let (tree, file_path) = if let Some(path) = args.first() {
        let content = std::fs::read_to_string(path)?;
        let tree = crate::sgf::parse_sgf(&content)?;
        (tree, Some(PathBuf::from(path)))
    } else {
        (GameTree::new(), None)
    };

    let editor = Editor::new(tree);
    let mut adapter = CliAdapter::new(file_path);
    run_editor(editor, &mut adapter)
}
