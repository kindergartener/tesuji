# Tesuji

A Rust library and desktop GUI for reading, navigating, and editing
[SGF (Smart Game Format)][sgf] files — the standard format for recording Go
(baduk/weiqi) games.

[sgf]: https://www.red-bean.com/sgf/

## Repository layout

| Crate | Description |
|-------|-------------|
| `tesuji` (root) | Library: SGF parser, serializer, game-tree editor |
| `gui/` | GUI application built on [iced] |

[iced]: https://iced.rs

## `tesuji` — the library

Use the `tesuji` library when you want to:

- **Parse SGF files** into a structured game tree you can traverse
  programmatically.
- **Serialize** a modified game tree back to valid SGF text.
- **Simulate board positions** — replay any path through the tree to get
  captures, ko points, and stone placements.
- **Build your own SGF editor** — the command-pattern `Editor` and `Adapter`
  trait make it straightforward to plug in a new UI backend.

### Adding to your project

```toml
[dependencies]
tesuji = "0.1"
```

### Example

```rust
use tesuji::{parse_sgf, Editor, EditCommand};
use tesuji::sgf::Board;

fn main() -> anyhow::Result<()> {
    let sgf = std::fs::read_to_string("game.sgf")?;
    let tree = parse_sgf(&sgf)?;
    let mut editor = Editor::new(tree);

    // Walk the main line and print each board's move number.
    loop {
        let board = Board::from_tree(&editor.tree, editor.cursor);
        println!("move {}", board.move_number);
        editor.apply(EditCommand::NavigateNext);

        if editor.tree.node(editor.cursor).children.is_empty() {
            break;
        }
    }
    Ok(())
}
```

### Feature flags

| Flag | Default | Description |
|------|---------|-------------|
| `cli` | off | Enables the `clap`-based CLI adapter (`tesuji::cli`) |

## `tesuji-gui` — the desktop application

`tesuji-gui` is an iced-based GUI for browsing and editing SGF files.

### Building

```sh
# From the repository root:
cargo build --release --manifest-path gui/Cargo.toml
```

### Running

```sh
cargo run --release --manifest-path gui/Cargo.toml -- [file.sgf]
```

## Development

```sh
# Build everything
cargo build

# Run all tests (library + GUI)
cargo test --workspace
```

## Roadmap

### `tesuji` library

- [ ] Variable board sizes — `Board::cells` only supports 19×19
- [ ] Turn inference for handicap games (infer from `AB` stone count)
- [ ] Expand recognized SGF properties (`LB`, `TR`, `SQ`, `CR`, …)
- [ ] Add examples

### `tesuji-gui`

- [ ] Game info panel (player names, ranks, komi, capture counts, turn indicator, etc.)
- [ ] Comment / annotation editor (`SGFProperty::C`)
- [ ] Property editor panel (edit arbitrary SGF properties)
- [ ] Game controls panel (Undo/Redo, File/Edit/View dropdowns)
- [ ] GTP Engine support (GnuGo / KataGo)
- [ ] Score estimation and resign
- [ ] Dark mode and themes
- [ ] Configurable display options (coordinate labels, ko markers, tree node labels)

## License

MIT OR Apache-2.0
