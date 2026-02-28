use std::{
    io::{self, BufRead, Write as _},
    path::PathBuf,
};

use anyhow::Result;
use clap::{Arg, Command};
use clap_complete::{Shell, generate};

use crate::{
    editor::{Adapter, EditCommand, Editor, run_editor},
    sgf::{GameTree, write_sgf},
};

// ---------------------------------------------------------------------------
// Binary-level command (used by main.rs for arg parsing and completion generation)
// ---------------------------------------------------------------------------

/// Build the top-level `tesuji` clap command used when the binary is invoked.
pub fn binary_command() -> Command {
    Command::new("tesuji")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A terminal SGF editor for Go game records")
        .long_about(
            "Tesuji is a terminal-based SGF (Smart Game Format) editor for \
             reviewing and annotating Go (baduk/weiqi) game records.\n\n\
             An interactive REPL lets you navigate the game tree, edit \
             properties, and save your changes. Type `help` at the prompt \
             to see all available commands.",
        )
        .arg(
            Arg::new("file")
                .value_name("FILE")
                .help("SGF file to open")
                .long_help(
                    "Path to an SGF file to open on startup. If omitted, \
                     the editor starts with an empty game tree.",
                )
                .required(false),
        )
        .arg(
            Arg::new("completions")
                .long("generate-completion")
                .value_name("SHELL")
                .help("Print a shell completion script and exit")
                .long_help(
                    "Generate a shell completion script for SHELL and print \
                     it to stdout. Source the script to enable tab-completion.\n\n\
                     Supported shells: bash, zsh, fish, powershell, elvish",
                )
                .value_parser(clap::value_parser!(Shell)),
        )
}

// ---------------------------------------------------------------------------
// Interactive REPL command parser (multicall style)
// ---------------------------------------------------------------------------

/// Build the clap `Command` used to parse each line typed at the `>` prompt.
///
/// Using `multicall(true)` means the first whitespace-delimited token is
/// treated as the subcommand name, mirroring how a user naturally types
/// short commands. Every subcommand gets `--help` for free.
fn repl_command() -> Command {
    Command::new("tesuji")
        .multicall(true)
        .about("Tesuji interactive SGF editor — type a command or `help`")
        .subcommand_required(false)
        // ── Navigation ──────────────────────────────────────────────────────
        .subcommand(
            Command::new("next")
                .visible_alias("n")
                .about("Advance to the next node (first child)")
                .long_about(
                    "Move the cursor forward to the first child of the current \
                     node. If the node has no children this is a no-op.",
                ),
        )
        .subcommand(
            Command::new("prev")
                .visible_alias("p")
                .about("Move to the parent node")
                .long_about(
                    "Retreat the cursor to the parent of the current node. \
                     At the root node this is a no-op.",
                ),
        )
        .subcommand(
            Command::new("branch")
                .visible_alias("b")
                .about("Select variation branch N at the current node")
                .long_about(
                    "Advance to the Nth child of the current node (0-indexed). \
                     Use `tree` to see available branches and their indices.",
                )
                .arg(
                    Arg::new("n")
                        .value_name("N")
                        .help("Branch index (0-based)")
                        .required(true)
                        .value_parser(clap::value_parser!(usize)),
                ),
        )
        // ── Display ─────────────────────────────────────────────────────────
        .subcommand(
            Command::new("show")
                .visible_alias("s")
                .about("Redisplay the current node's properties"),
        )
        .subcommand(
            Command::new("tree")
                .visible_alias("t")
                .about("Display the full game tree")
                .long_about(
                    "Print the entire game tree. The current node is marked \
                     with `*`. Each line shows the node ID and its SGF properties.",
                ),
        )
        // ── File I/O ────────────────────────────────────────────────────────
        .subcommand(
            Command::new("load")
                .visible_alias("l")
                .about("Load an SGF file from disk")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to the SGF file to open")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("save")
                .visible_alias("w")
                .about("Save the game to disk")
                .long_about(
                    "Serialize the current game tree to SGF and write it to \
                     disk. If PATH is omitted the file opened on startup (or \
                     via `load`) is used.",
                )
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Destination path (defaults to the currently loaded file)")
                        .required(false),
                ),
        )
        // ── Editing ─────────────────────────────────────────────────────────
        .subcommand(
            Command::new("delete")
                .visible_alias("d")
                .about("Delete the current node and its subtree")
                .long_about(
                    "Remove the current node and every node rooted beneath it, \
                     then retreat the cursor to the parent. At the root node \
                     this is a no-op.",
                ),
        )
        .subcommand(
            Command::new("variation")
                .visible_alias("v")
                .about("Append an empty variation at the current position")
                .long_about(
                    "Add a new empty child node to the current node without \
                     moving the cursor. Use `branch N` to enter the variation.",
                ),
        )
        // ── Meta ────────────────────────────────────────────────────────────
        .subcommand(
            Command::new("completions")
                .about("Print a shell completion script to stdout")
                .arg(
                    Arg::new("shell")
                        .value_name("SHELL")
                        .help("Target shell (bash, zsh, fish, powershell, elvish)")
                        .required(true)
                        .value_parser(clap::value_parser!(Shell)),
                ),
        )
        .subcommand(
            Command::new("quit")
                .visible_alias("q")
                .about("Exit the editor"),
        )
}

// ---------------------------------------------------------------------------
// Tree rendering helpers
// ---------------------------------------------------------------------------

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
    let label: String =
        node.properties.iter().map(|p| format!("{p}")).collect::<Vec<_>>().join(" ");
    out.push_str(&format!("{indent}{marker}[{id}] {label}\n"));
    for &child in &node.children {
        render_node(tree, child, depth + 1, cursor, out);
    }
}

// ---------------------------------------------------------------------------
// CliAdapter
// ---------------------------------------------------------------------------

pub struct CliAdapter {
    file_path: Option<PathBuf>,
    last_sgf: String,
    last_tree_display: String,
    /// Cached node display so that `show` can reprint without re-entering render.
    last_node_display: String,
}

impl CliAdapter {
    fn new(file_path: Option<PathBuf>) -> Self {
        Self {
            file_path,
            last_sgf: String::new(),
            last_tree_display: String::new(),
            last_node_display: String::new(),
        }
    }
}

impl Adapter for CliAdapter {
    fn render(&mut self, editor: &Editor) -> Result<()> {
        let node = editor.tree.node(editor.cursor);
        let mut display = format!("--- node {} ---\n", editor.cursor);
        for prop in &node.properties {
            display.push_str(&format!("  {prop}\n"));
        }
        print!("{display}");
        self.last_node_display = display;
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
            if stdin.lock().read_line(&mut line)? == 0 {
                return Ok(None); // EOF
            }

            let args: Vec<&str> = line.split_whitespace().collect();
            if args.is_empty() {
                continue;
            }

            // In multicall mode the first token is argv[0], which clap uses as
            // the subcommand name. Rebuild the parser each iteration so that
            // internal clap state (e.g. `--help` seen flag) is fully reset.
            match repl_command().try_get_matches_from(args) {
                Err(e) => {
                    // `e.print()` sends help/version text to stdout and error
                    // messages to stderr, matching clap's conventional behaviour.
                    let _ = e.print();
                    eprintln!(); // ensure prompt appears on its own line
                    continue;
                }
                Ok(matches) => match matches.subcommand() {
                    // ── Navigation ──────────────────────────────────────────
                    Some(("next", _)) => return Ok(Some(EditCommand::NavigateNext)),
                    Some(("prev", _)) => return Ok(Some(EditCommand::NavigatePrev)),
                    Some(("branch", m)) => {
                        let n = *m.get_one::<usize>("n").unwrap();
                        return Ok(Some(EditCommand::NavigateBranch(n)));
                    }
                    // ── Display ─────────────────────────────────────────────
                    Some(("show", _)) => {
                        print!("{}", self.last_node_display);
                        continue;
                    }
                    Some(("tree", _)) => {
                        print!("{}", self.last_tree_display);
                        continue;
                    }
                    // ── File I/O ────────────────────────────────────────────
                    Some(("load", m)) => {
                        let path = m.get_one::<String>("path").unwrap();
                        let content = std::fs::read_to_string(path)?;
                        let tree = crate::sgf::parse_sgf(&content)?;
                        self.file_path = Some(PathBuf::from(path));
                        return Ok(Some(EditCommand::Load(tree)));
                    }
                    Some(("save", m)) => {
                        let path = match m.get_one::<String>("path") {
                            Some(p) => PathBuf::from(p),
                            None => match &self.file_path {
                                Some(p) => p.clone(),
                                None => {
                                    eprintln!(
                                        "error: no file is currently open; \
                                         usage: save <PATH>"
                                    );
                                    continue;
                                }
                            },
                        };
                        std::fs::write(&path, &self.last_sgf)?;
                        println!("saved to {}", path.display());
                        continue;
                    }
                    // ── Editing ─────────────────────────────────────────────
                    Some(("delete", _)) => return Ok(Some(EditCommand::DeleteCurrentNode)),
                    Some(("variation", _)) => return Ok(Some(EditCommand::AppendVariation)),
                    // ── Meta ────────────────────────────────────────────────
                    Some(("completions", m)) => {
                        let &shell = m.get_one::<Shell>("shell").unwrap();
                        generate(shell, &mut binary_command(), "tesuji", &mut io::stdout());
                        continue;
                    }
                    Some(("quit", _)) => return Ok(None),
                    // clap's built-in `help` subcommand is handled via the
                    // Err(DisplayHelp) path above; this arm is a safety net.
                    Some((name, _)) => {
                        eprintln!("unknown command: {name}  (type `help` for a list)");
                        continue;
                    }
                    None => continue,
                },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(file: Option<&str>) -> Result<()> {
    let (tree, file_path) = if let Some(path) = file {
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
