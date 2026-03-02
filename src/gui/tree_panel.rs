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
    pub scroll_offset: f32,
}

fn subtree_width(tree: &GameTree, id: NodeId) -> usize {
    let children = &tree.node(id).children;
    if children.is_empty() {
        1
    } else {
        children.iter().map(|&c| subtree_width(tree, c)).sum()
    }
}

fn assign_layout(
    tree: &GameTree,
    id: NodeId,
    depth: usize,
    col_start: usize,
    layout: &mut HashMap<NodeId, (usize, usize)>,
) {
    layout.insert(id, (col_start, depth));
    let mut next_col = col_start;
    for &child in &tree.node(id).children {
        assign_layout(tree, child, depth + 1, next_col, layout);
        next_col += subtree_width(tree, child);
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
                    let scroll_delta = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => y * NODE_PITCH,
                        mouse::ScrollDelta::Pixels { y, .. } => *y,
                    };
                    let mut layout = HashMap::new();
                    assign_layout(self.tree, self.root, 0, 0, &mut layout);
                    let max_row = layout.values().map(|&(_, r)| r).max().unwrap_or(0);
                    let total_height = PANEL_MARGIN * 2.0 + max_row as f32 * NODE_PITCH;
                    let max_scroll = (total_height - bounds.height).max(0.0);
                    state.scroll_offset =
                        (state.scroll_offset - scroll_delta).clamp(0.0, max_scroll);
                    Some(Action::capture())
                } else {
                    None
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let adjusted_y = pos.y + state.scroll_offset;
                    let mut layout = HashMap::new();
                    assign_layout(self.tree, self.root, 0, 0, &mut layout);
                    for (&id, &(col, row)) in &layout {
                        let cx = PANEL_MARGIN + col as f32 * NODE_PITCH;
                        let cy = PANEL_MARGIN + row as f32 * NODE_PITCH;
                        let dx = pos.x - cx;
                        let dy = adjusted_y - cy;
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

        let mut layout = HashMap::new();
        assign_layout(self.tree, self.root, 0, 0, &mut layout);

        // Apply scroll offset via translation
        frame.translate(Vector::new(0.0, -state.scroll_offset));

        // Draw edges first (parent → child)
        let edge_stroke = Stroke::default()
            .with_color(Color::from_rgb(0.55, 0.55, 0.55))
            .with_width(1.5);
        for (&id, &(col, row)) in &layout {
            let px = PANEL_MARGIN + col as f32 * NODE_PITCH;
            let py = PANEL_MARGIN + row as f32 * NODE_PITCH;
            for &child_id in &self.tree.node(id).children {
                if let Some(&(child_col, child_row)) = layout.get(&child_id) {
                    let cx = PANEL_MARGIN + child_col as f32 * NODE_PITCH;
                    let cy = PANEL_MARGIN + child_row as f32 * NODE_PITCH;
                    frame.stroke(
                        &Path::line(Point::new(px, py), Point::new(cx, cy)),
                        edge_stroke,
                    );
                }
            }
        }

        // Draw nodes
        for (&id, &(col, row)) in &layout {
            let cx = PANEL_MARGIN + col as f32 * NODE_PITCH;
            let cy = PANEL_MARGIN + row as f32 * NODE_PITCH;
            let center = Point::new(cx, cy);

            // Determine stone color from node properties
            let stone_color = self.tree.node(id).properties.iter().find_map(|p| match p {
                SGFProperty::B(_) => Some(theme::STONE_BLACK),
                SGFProperty::W(_) => Some(theme::STONE_WHITE),
                _ => None,
            });

            match stone_color {
                Some(color) => {
                    frame.fill(&Path::circle(center, NODE_RADIUS), color);
                    if color == theme::STONE_WHITE {
                        frame.stroke(
                            &Path::circle(center, NODE_RADIUS),
                            Stroke::default()
                                .with_color(Color::from_rgb(0.4, 0.4, 0.4))
                                .with_width(1.0),
                        );
                    }
                }
                None => {
                    // Root or setup node: gray circle
                    frame.fill(
                        &Path::circle(center, NODE_RADIUS),
                        Color::from_rgb(0.55, 0.55, 0.55),
                    );
                }
            }

            // Cursor highlight ring
            if id == self.cursor {
                frame.stroke(
                    &Path::circle(center, NODE_RADIUS + 3.0),
                    Stroke::default()
                        .with_color(theme::TREE_CURSOR_RING)
                        .with_width(2.5),
                );
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
            let adjusted_y = pos.y + state.scroll_offset;
            let mut layout = HashMap::new();
            assign_layout(self.tree, self.root, 0, 0, &mut layout);
            for &(col, row) in layout.values() {
                let cx = PANEL_MARGIN + col as f32 * NODE_PITCH;
                let cy = PANEL_MARGIN + row as f32 * NODE_PITCH;
                let dx = pos.x - cx;
                let dy = adjusted_y - cy;
                if dx * dx + dy * dy <= NODE_RADIUS * NODE_RADIUS {
                    return mouse::Interaction::Pointer;
                }
            }
        }
        mouse::Interaction::default()
    }
}
