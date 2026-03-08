use criterion::{Criterion, black_box, criterion_group, criterion_main};
use iced::Size;

use tesuji::gui::assets::BoardAssets;
use tesuji::gui::board::{BoardMetrics, build_board_primitives, build_board_primitives_textured};
use tesuji::sgf::{Board, Cell};

/// Create an empty 19×19 board.
fn empty_board() -> Board {
    let tree = tesuji::sgf::GameTree::new();
    Board::from_tree(&tree, tree.roots[0])
}

/// Create a mid-game 19×19 board with a realistic number of stones.
fn midgame_board() -> Board {
    let sgf = "(;GM[1]FF[4]SZ[19];\
        B[pd];W[dp];B[pp];W[dd];B[fc];W[cf];B[jd];W[qf];B[nc];W[rd];\
        B[qc];W[qi];B[qk];W[oi];B[ok];W[mh];B[cn];W[fq];B[bp];W[cq];\
        B[ck];W[dj];B[dk];W[ej];B[bj];W[ci];B[bi];W[ch];B[fg];W[eh];\
        B[fh];W[fi];B[gi];W[fj];B[gj];W[gk];B[hk];W[gl];B[hl];W[gm];\
        B[hm];W[hn];B[in];W[ho];B[io];W[hp];B[ip];W[iq];B[jq];W[jr];\
        B[kq];W[kr];B[lq];W[lr];B[mr];W[mq];B[nq];W[mp];B[np];W[mo])";
    let tree = tesuji::parse_sgf(sgf).unwrap();
    // Navigate to last move
    let mut cursor = tree.roots[0];
    loop {
        let children = &tree.node(cursor).children;
        if children.is_empty() {
            break;
        }
        cursor = children[0];
    }
    Board::from_tree(&tree, cursor)
}

/// Create a full board (361 stones) for worst-case benchmarking.
fn full_board() -> Board {
    let mut board = empty_board();
    // Fill with alternating stones
    for row in 0..19 {
        for col in 0..19 {
            board.cells[row][col] = if (row + col) % 2 == 0 {
                Cell::Black
            } else {
                Cell::White
            };
        }
    }
    board.move_number = 361;
    board
}

// ── Vector rendering benchmarks ──

fn bench_vector_empty(c: &mut Criterion) {
    let board = empty_board();
    let metrics = BoardMetrics::new(Size::new(800.0, 800.0), board.size);

    c.bench_function("vector/empty_19x19", |b| {
        b.iter(|| {
            black_box(build_board_primitives(
                black_box(&board),
                black_box(&metrics),
                black_box(None),
                black_box(None),
            ))
        })
    });
}

fn bench_vector_midgame(c: &mut Criterion) {
    let board = midgame_board();
    let metrics = BoardMetrics::new(Size::new(800.0, 800.0), board.size);
    let last_move = Some((12, 14)); // W[mo]

    c.bench_function("vector/midgame_19x19", |b| {
        b.iter(|| {
            black_box(build_board_primitives(
                black_box(&board),
                black_box(&metrics),
                black_box(Some((9, 9))),
                black_box(last_move),
            ))
        })
    });
}

fn bench_vector_full(c: &mut Criterion) {
    let board = full_board();
    let metrics = BoardMetrics::new(Size::new(800.0, 800.0), board.size);

    c.bench_function("vector/full_19x19", |b| {
        b.iter(|| {
            black_box(build_board_primitives(
                black_box(&board),
                black_box(&metrics),
                black_box(None),
                black_box(Some((0, 0))),
            ))
        })
    });
}

// ── Textured rendering benchmarks ──

fn bench_textured_empty(c: &mut Criterion) {
    let board = empty_board();
    let metrics = BoardMetrics::new(Size::new(800.0, 800.0), board.size);
    let assets = BoardAssets::load();

    c.bench_function("textured/empty_19x19", |b| {
        b.iter(|| {
            black_box(build_board_primitives_textured(
                black_box(&board),
                black_box(&metrics),
                black_box(None),
                black_box(None),
                black_box(&assets),
            ))
        })
    });
}

fn bench_textured_midgame(c: &mut Criterion) {
    let board = midgame_board();
    let metrics = BoardMetrics::new(Size::new(800.0, 800.0), board.size);
    let assets = BoardAssets::load();
    let last_move = Some((12, 14));

    c.bench_function("textured/midgame_19x19", |b| {
        b.iter(|| {
            black_box(build_board_primitives_textured(
                black_box(&board),
                black_box(&metrics),
                black_box(Some((9, 9))),
                black_box(last_move),
                black_box(&assets),
            ))
        })
    });
}

fn bench_textured_full(c: &mut Criterion) {
    let board = full_board();
    let metrics = BoardMetrics::new(Size::new(800.0, 800.0), board.size);
    let assets = BoardAssets::load();

    c.bench_function("textured/full_19x19", |b| {
        b.iter(|| {
            black_box(build_board_primitives_textured(
                black_box(&board),
                black_box(&metrics),
                black_box(None),
                black_box(Some((0, 0))),
                black_box(&assets),
            ))
        })
    });
}

criterion_group!(
    benches,
    bench_vector_empty,
    bench_vector_midgame,
    bench_vector_full,
    bench_textured_empty,
    bench_textured_midgame,
    bench_textured_full,
);
criterion_main!(benches);
