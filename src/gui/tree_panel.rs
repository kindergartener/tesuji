use std::collections::HashMap;

use iced::{Color, Event, Point, Rectangle, Vector, mouse};
use iced::widget::canvas::{self, Action, Frame, Path, Stroke};

use crate::sgf::{GameTree, NodeId, SGFProperty};
use crate::gui::{Message, theme};

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
    pub scroll_y: f32,
    pub scroll_x: f32,
    /// Cached layout computed once per draw, reused in update/mouse_interaction.
    pub cached: Option<TreeLayout>,
}

pub struct TreeLayout {
    pub positions: HashMap<NodeId, (usize, usize)>,
    pub max_col: usize,
    pub max_row: usize,
    pub move_numbers: HashMap<NodeId, usize>,
}

/// Compute subtree widths, positions, and move numbers in a single
/// iterative (non-recursive) pass. O(n) time and space.
fn compute_layout(tree: &GameTree, root: NodeId) -> TreeLayout {
    // --- Pass 1: compute subtree widths bottom-up (iterative post-order) ---
    let mut widths: HashMap<NodeId, usize> = HashMap::new();
    let mut stack: Vec<(NodeId, bool)> = vec![(root, false)];

    while let Some((id, processed)) = stack.pop() {
        if processed {
            let children = &tree.node(id).children;
            if children.is_empty() {
                widths.insert(id, 1);
            } else {
                let w: usize = children.iter().map(|&c| widths[&c]).sum();
                widths.insert(id, w);
            }
        } else {
            stack.push((id, true));
            for &child in tree.node(id).children.iter().rev() {
                stack.push((child, false));
            }
        }
    }

    // --- Pass 2: assign (col, row) positions top-down (iterative pre-order) ---
    let mut positions: HashMap<NodeId, (usize, usize)> = HashMap::new();
    let mut max_col: usize = 0;
    let mut max_row: usize = 0;

    // (id, col_start, depth)
    let mut pos_stack: Vec<(NodeId, usize, usize)> = vec![(root, 0, 0)];

    while let Some((id, col_start, depth)) = pos_stack.pop() {
        positions.insert(id, (col_start, depth));
        if col_start > max_col {
            max_col = col_start;
        }
        if depth > max_row {
            max_row = depth;
        }

        // Collect child assignments in forward order, then push reversed onto stack
        let children = &tree.node(id).children;
        let mut child_assignments: Vec<(NodeId, usize, usize)> = Vec::new();
        let mut next_col = col_start;
        for &child in children {
            child_assignments.push((child, next_col, depth + 1));
            next_col += widths[&child];
        }
        for assignment in child_assignments.into_iter().rev() {
            pos_stack.push(assignment);
        }
    }

    // --- Pass 3: compute move numbers (iterative pre-order) ---
    let mut move_numbers: HashMap<NodeId, usize> = HashMap::new();
    // (id, parent_move_count)
    let mut mn_stack: Vec<(NodeId, usize)> = vec![(root, 0)];

    while let Some((id, parent_moves)) = mn_stack.pop() {
        let is_move = tree.node(id).properties.iter().any(|p| {
            matches!(p, SGFProperty::B(_) | SGFProperty::W(_))
        });
        let this_moves = if is_move { parent_moves + 1 } else { 0 };
        move_numbers.insert(id, this_moves);
        let next = if is_move { parent_moves + 1 } else { parent_moves };
        for &child in tree.node(id).children.iter().rev() {
            mn_stack.push((child, next));
        }
    }

    TreeLayout { positions, max_col, max_row, move_numbers }
}

fn x_translation(max_col: usize, scroll_x: f32, panel_width: f32) -> f32 {
    let total_width = 2.0 * PANEL_MARGIN + max_col as f32 * NODE_PITCH;
    if total_width <= panel_width {
        (panel_width - total_width) / 2.0
    } else {
        -scroll_x
    }
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
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    let (delta_x, delta_y) = match delta {
                        mouse::ScrollDelta::Lines { x, y } => (x * NODE_PITCH, y * NODE_PITCH),
                        mouse::ScrollDelta::Pixels { x, y } => (*x, *y),
                    };

                    // Use cached layout from last draw, or compute fresh
                    let layout = state.cached.get_or_insert_with(|| {
                        compute_layout(self.tree, self.root)
                    });

                    // vertical scroll
                    let max_scroll_y = (PANEL_MARGIN * 2.0
                        + layout.max_row as f32 * NODE_PITCH
                        - bounds.height)
                        .max(0.0);
                    state.scroll_y = (state.scroll_y - delta_y).clamp(0.0, max_scroll_y);

                    // horizontal scroll
                    let total_width = PANEL_MARGIN * 2.0 + layout.max_col as f32 * NODE_PITCH;
                    if total_width > bounds.width {
                        let max_scroll_x = total_width - bounds.width;
                        state.scroll_x = (state.scroll_x - delta_x).clamp(0.0, max_scroll_x);
                    } else {
                        state.scroll_x = 0.0;
                    }

                    Some(Action::capture())
                } else {
                    None
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    // Use cached layout from last draw, or compute fresh
                    let layout = state.cached.get_or_insert_with(|| {
                        compute_layout(self.tree, self.root)
                    });
                    let xt = x_translation(layout.max_col, state.scroll_x, bounds.width);
                    let content_x = pos.x - xt;
                    let content_y = pos.y + state.scroll_y;
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

        let xt = x_translation(layout.max_col, state.scroll_x, bounds.width);
        frame.translate(Vector::new(xt, -state.scroll_y));

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
            // Use cached layout if available
            let layout;
            let layout_ref = match &state.cached {
                Some(cached) => cached,
                None => {
                    layout = compute_layout(self.tree, self.root);
                    &layout
                }
            };
            let xt = x_translation(layout_ref.max_col, state.scroll_x, bounds.width);
            let content_x = pos.x - xt;
            let content_y = pos.y + state.scroll_y;
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
