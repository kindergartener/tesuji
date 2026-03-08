use std::collections::HashMap;

use iced::widget::canvas::{self, Action, Frame, Path, Stroke};
use iced::{Color, Event, Point, Rectangle, Vector, mouse};

use crate::gui::{Message, theme};
use tesuji::sgf::{GameTree, NodeId, SGFProperty};

const NODE_RADIUS: f32 = 8.0;
const NODE_PITCH: f32 = 28.0;
const PANEL_MARGIN: f32 = 16.0;

pub struct TreePanelProgram<'a> {
    pub tree: &'a GameTree,
    pub root: NodeId,
    pub cursor: NodeId,
}

#[derive(Default)]
pub struct TreePanelState {
    /// Cached layout, reused in update/mouse_interaction.
    pub cached: Option<TreeLayout>,
}

pub struct TreeLayout {
    pub positions: HashMap<NodeId, (usize, usize)>,
    pub max_col: usize,
    pub max_row: usize,
    pub move_numbers: HashMap<NodeId, usize>,
}

/// Compute positions and move numbers using greedy interval packing.
/// Variations reuse columns when their vertical extents don't overlap.
fn compute_layout(tree: &GameTree, root: NodeId) -> TreeLayout {
    // --- Pass 1: compute mainline lengths bottom-up (iterative post-order) ---
    let mut mainline_len: HashMap<NodeId, usize> = HashMap::new();
    {
        let mut stack: Vec<(NodeId, bool)> = vec![(root, false)];
        while let Some((id, processed)) = stack.pop() {
            if processed {
                let first_child = tree.node(id).children.first().copied();
                let len = match first_child {
                    Some(c) => 1 + mainline_len[&c],
                    None => 1,
                };
                mainline_len.insert(id, len);
            } else {
                stack.push((id, true));
                for &child in tree.node(id).children.iter().rev() {
                    stack.push((child, false));
                }
            }
        }
    }

    // --- Pass 2: assign (col, row) via DFS with interval packing ---
    // col_intervals[c] = list of (start_row, end_row) occupied intervals
    let mut col_intervals: Vec<Vec<(usize, usize)>> = vec![Vec::new()];
    let mut positions: HashMap<NodeId, (usize, usize)> = HashMap::new();
    let mut max_col: usize = 0;
    let mut max_row: usize = 0;

    // Find the leftmost column > min_col where [start, end] doesn't overlap.
    let find_free_col = |col_intervals: &mut Vec<Vec<(usize, usize)>>,
                         min_col: usize,
                         start: usize,
                         end: usize|
     -> usize {
        for c in (min_col + 1).. {
            // Ensure the column exists
            while c >= col_intervals.len() {
                col_intervals.push(Vec::new());
            }
            let overlaps = col_intervals[c]
                .iter()
                .any(|&(s, e)| start <= e && end >= s);
            if !overlaps {
                return c;
            }
        }
        unreachable!()
    };

    enum Action {
        Place(NodeId, usize, usize, bool), // id, col, depth, is_variation_root
        DeferChild(NodeId, usize),         // parent_id, child_index
    }

    let mut stack: Vec<Action> = vec![Action::Place(root, 0, 0, true)];

    while let Some(action) = stack.pop() {
        match action {
            Action::Place(id, col, depth, is_variation_root) => {
                positions.insert(id, (col, depth));
                if col > max_col {
                    max_col = col;
                }
                if depth > max_row {
                    max_row = depth;
                }

                // Record interval for variation roots
                if is_variation_root {
                    let end = depth + mainline_len[&id] - 1;
                    while col >= col_intervals.len() {
                        col_intervals.push(Vec::new());
                    }
                    col_intervals[col].push((depth, end));
                }

                // Schedule children in reverse so first child is processed first
                let children = &tree.node(id).children;
                if !children.is_empty() {
                    // Defer non-first children (variations) in reverse order
                    for i in (1..children.len()).rev() {
                        stack.push(Action::DeferChild(id, i));
                    }
                    // First child inherits parent's column
                    stack.push(Action::Place(children[0], col, depth + 1, false));
                }
            }
            Action::DeferChild(parent_id, child_index) => {
                let child_id = tree.node(parent_id).children[child_index];
                let (parent_col, _) = positions[&parent_id];
                let child_depth = positions[&parent_id].1 + 1;
                let ml = mainline_len[&child_id];
                let col = find_free_col(
                    &mut col_intervals,
                    parent_col,
                    child_depth,
                    child_depth + ml - 1,
                );
                stack.push(Action::Place(child_id, col, child_depth, true));
            }
        }
    }

    // --- Pass 3: compute move numbers (iterative pre-order) ---
    let mut move_numbers: HashMap<NodeId, usize> = HashMap::new();
    // (id, parent_move_count)
    let mut mn_stack: Vec<(NodeId, usize)> = vec![(root, 0)];

    while let Some((id, parent_moves)) = mn_stack.pop() {
        let is_move = tree
            .node(id)
            .properties
            .iter()
            .any(|p| matches!(p, SGFProperty::B(_) | SGFProperty::W(_)));
        let this_moves = if is_move { parent_moves + 1 } else { 0 };
        move_numbers.insert(id, this_moves);
        let next = if is_move {
            parent_moves + 1
        } else {
            parent_moves
        };
        for &child in tree.node(id).children.iter().rev() {
            mn_stack.push((child, next));
        }
    }

    TreeLayout {
        positions,
        max_col,
        max_row,
        move_numbers,
    }
}

/// Compute translation to center the cursor node in the viewport.
fn cursor_translation(layout: &TreeLayout, cursor: NodeId, bounds: Rectangle) -> (f32, f32) {
    let (col, row) = layout.positions.get(&cursor).copied().unwrap_or((0, 0));
    let cursor_x = PANEL_MARGIN + col as f32 * NODE_PITCH;
    let cursor_y = PANEL_MARGIN + row as f32 * NODE_PITCH;
    let tx = bounds.width / 2.0 - cursor_x;
    let ty = bounds.height / 2.0 - cursor_y;
    (tx, ty)
}

impl<'a> canvas::Program<Message> for TreePanelProgram<'a> {
    type State = TreePanelState;

    fn update(
        &self,
        state: &mut TreePanelState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.cached = Some(compute_layout(self.tree, self.root));
                    let layout = state.cached.as_ref().unwrap();
                    let (tx, ty) = cursor_translation(layout, self.cursor, bounds);
                    let content_x = pos.x - tx;
                    let content_y = pos.y - ty;
                    for (&id, &(col, row)) in &layout.positions {
                        let cx = PANEL_MARGIN + col as f32 * NODE_PITCH;
                        let cy = PANEL_MARGIN + row as f32 * NODE_PITCH;
                        let dx = content_x - cx;
                        let dy = content_y - cy;
                        if dx * dx + dy * dy <= NODE_RADIUS * NODE_RADIUS {
                            return Some(
                                Action::publish(Message::NavigateToNode(id)).and_capture(),
                            );
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &TreePanelState,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Panel background
        frame.fill_rectangle(
            Point::ORIGIN,
            bounds.size(),
            Color::from_rgb(0.93, 0.93, 0.93),
        );

        let layout = compute_layout(self.tree, self.root);

        let (tx, ty) = cursor_translation(&layout, self.cursor, bounds);
        frame.translate(Vector::new(tx, ty));

        let edge_stroke = Stroke::default()
            .with_color(Color::from_rgb(0.55, 0.55, 0.55))
            .with_width(1.5);

        // Draw edges (RELU-style)
        for (&id, &(col, row)) in &layout.positions {
            let px = PANEL_MARGIN + col as f32 * NODE_PITCH;
            let py = PANEL_MARGIN + row as f32 * NODE_PITCH;
            for &child_id in &self.tree.node(id).children {
                if let Some(&(child_col, _child_row)) = layout.positions.get(&child_id) {
                    let cx = PANEL_MARGIN + child_col as f32 * NODE_PITCH;
                    let cy = py + NODE_PITCH; // child is always one row below
                    if child_col == col {
                        // mainline: straight vertical
                        frame.stroke(
                            &Path::line(Point::new(px, py), Point::new(cx, cy)),
                            edge_stroke,
                        );
                    } else {
                        // variation: horizontal then 45° diagonal
                        let elbow_x = cx - NODE_PITCH;
                        let path = Path::new(|p| {
                            p.move_to(Point::new(px, py));
                            if elbow_x > px {
                                p.line_to(Point::new(elbow_x, py));
                            }
                            p.line_to(Point::new(cx, cy));
                        });
                        frame.stroke(&path, edge_stroke);
                    }
                }
            }
        }

        let cursor_stroke = Stroke::default()
            .with_color(theme::TREE_CURSOR_RING)
            .with_width(2.5);
        let white_outline_stroke = Stroke::default()
            .with_color(Color::from_rgb(0.4, 0.4, 0.4))
            .with_width(1.0);

        // Draw nodes
        for (&id, &(col, row)) in &layout.positions {
            let cx = PANEL_MARGIN + col as f32 * NODE_PITCH;
            let cy = PANEL_MARGIN + row as f32 * NODE_PITCH;
            let center = Point::new(cx, cy);

            let move_prop = self.tree.node(id).properties.iter().find_map(|p| match p {
                SGFProperty::B(c) => Some((theme::STONE_BLACK, *c, true)),
                SGFProperty::W(c) => Some((theme::STONE_WHITE, *c, false)),
                _ => None,
            });

            if id == self.root {
                // Gray diamond
                let s = NODE_RADIUS * 1.2;
                let diamond = Path::new(|p| {
                    p.move_to(Point::new(cx, cy - s));
                    p.line_to(Point::new(cx + s, cy));
                    p.line_to(Point::new(cx, cy + s));
                    p.line_to(Point::new(cx - s, cy));
                    p.close();
                });
                frame.fill(&diamond, Color::from_rgb(0.55, 0.55, 0.55));

                if id == self.cursor {
                    let rs = s + 3.0;
                    let ring = Path::new(|p| {
                        p.move_to(Point::new(cx, cy - rs));
                        p.line_to(Point::new(cx + rs, cy));
                        p.line_to(Point::new(cx, cy + rs));
                        p.line_to(Point::new(cx - rs, cy));
                        p.close();
                    });
                    frame.stroke(&ring, cursor_stroke);
                }
            } else {
                match move_prop {
                    Some((color, coord, is_black)) => {
                        frame.fill(&Path::circle(center, NODE_RADIUS), color);
                        if !is_black {
                            frame.stroke(&Path::circle(center, NODE_RADIUS), white_outline_stroke);
                        }
                        let label = if coord.is_pass() {
                            "-".to_string()
                        } else {
                            layout.move_numbers[&id].to_string()
                        };
                        let label_color = if is_black {
                            Color::WHITE
                        } else {
                            Color::from_rgb(0.1, 0.1, 0.1)
                        };
                        frame.fill_text(canvas::Text {
                            content: label,
                            position: center,
                            size: iced::Pixels(8.0),
                            color: label_color,
                            align_x: iced::alignment::Horizontal::Center.into(),
                            align_y: iced::alignment::Vertical::Center.into(),
                            ..canvas::Text::default()
                        });
                    }
                    None => {
                        // Non-move setup node: gray circle
                        frame.fill(
                            &Path::circle(center, NODE_RADIUS),
                            Color::from_rgb(0.55, 0.55, 0.55),
                        );
                    }
                }

                if id == self.cursor {
                    frame.stroke(&Path::circle(center, NODE_RADIUS + 3.0), cursor_stroke);
                }
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &TreePanelState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if let Some(pos) = cursor.position_in(bounds) {
            let layout;
            let layout_ref = match &state.cached {
                Some(cached) => cached,
                None => {
                    layout = compute_layout(self.tree, self.root);
                    &layout
                }
            };
            let (tx, ty) = cursor_translation(layout_ref, self.cursor, bounds);
            let content_x = pos.x - tx;
            let content_y = pos.y - ty;
            for &(col, row) in layout_ref.positions.values() {
                let cx = PANEL_MARGIN + col as f32 * NODE_PITCH;
                let cy = PANEL_MARGIN + row as f32 * NODE_PITCH;
                let dx = content_x - cx;
                let dy = content_y - cy;
                if dx * dx + dy * dy <= NODE_RADIUS * NODE_RADIUS {
                    return mouse::Interaction::Pointer;
                }
            }
        }
        mouse::Interaction::default()
    }
}
