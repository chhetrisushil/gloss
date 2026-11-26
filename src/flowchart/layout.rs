//! Placing the flowchart's nodes: a row each, in branch columns with loop lanes (D62, D63).

use super::graph::{Graph, Node, Shape};
use std::collections::BTreeMap;

const MARGIN: i32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    pub width: i32,
    pub height: i32,
    /// One per node, same index.
    pub boxes: Vec<Rect>,
    /// One per node.
    pub column: Vec<usize>,
    /// Loop decision node -> x of its back and done lanes.
    pub loops: BTreeMap<usize, (i32, i32)>,
    pub give_lane: i32,
    /// The y of the top of the gap above each row; `gaps[0]` is 0 and unused (row 0 has no gap).
    pub gaps: Vec<i32>,
    /// Filled by Task 4.
    pub routes: Vec<Vec<(i32, i32)>>,
}

/// The width and height of a node's box (§4.11).
fn size(node: &Node) -> (i32, i32) {
    let lines = node.label.len().max(1) as i32;
    let chars = node
        .label
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0) as i32;
    let (w, h) = (9 * chars + 24, 20 * lines + 16);
    // A decision's diamond has half its height at each end.
    if node.shape == Shape::Decision {
        (w + h, h)
    } else {
        (w, h)
    }
}

/// Stacks rows of the given heights from the top margin; `tracks[i]` arrows run in the gap
/// below row `i`. Returns each row's y, each gap's top y (0 for row 0), and the chart height.
pub(crate) fn stack(heights: &[i32], tracks: &[usize]) -> (Vec<i32>, Vec<i32>, i32) {
    let (mut ys, mut gaps, mut y) = (vec![], vec![], MARGIN);
    for (i, &h) in heights.iter().enumerate() {
        gaps.push(if i > 0 { y } else { 0 });
        if i > 0 {
            y += 24 + 10 * (tracks[i - 1].max(1) as i32 - 1);
        }
        ys.push(y);
        y += h;
    }
    (ys, gaps, y + MARGIN)
}

pub(crate) fn layout(g: &Graph) -> Layout {
    let sizes: Vec<(i32, i32)> = g.nodes.iter().map(size).collect();
    let heights: Vec<i32> = sizes.iter().map(|s| s.1).collect();
    let tracks = vec![1; heights.len().saturating_sub(1)];
    let (ys, gaps, height) = stack(&heights, &tracks);
    // One column until Task 3b: every node is centred in it.
    let widest = sizes.iter().map(|s| s.0).max().unwrap_or(0);
    let boxes = sizes
        .iter()
        .zip(&ys)
        .map(|(&(w, h), &y)| Rect {
            x: MARGIN + (widest - w) / 2,
            y,
            w,
            h,
        })
        .collect();
    let give_lane = 2 * MARGIN + widest;
    Layout {
        width: give_lane + MARGIN,
        height,
        boxes,
        column: vec![0; g.nodes.len()],
        loops: BTreeMap::new(),
        give_lane,
        gaps,
        routes: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Source;

    fn graph_of(text: &str) -> Graph {
        let (_, checked) = crate::check_program(text);
        let checked = checked.expect("the program checks");
        let feature = checked.program().features().next().expect("a feature");
        super::super::graph::graph(feature, &Source::new(text))
    }

    fn node(shape: Shape, label: &[&str]) -> Node {
        let label = label.iter().map(|s| s.to_string()).collect();
        Node {
            shape,
            label,
            line: 1,
        }
    }

    #[test]
    fn sizes() {
        let ten = "x".repeat(10);
        assert_eq!(size(&node(Shape::Step, &[&ten])), (114, 36));
        assert_eq!(size(&node(Shape::Call, &[&ten, "ab"])), (114, 56));
        assert_eq!(size(&node(Shape::Decision, &[&ten])), (114 + 36, 36));
    }

    #[test]
    fn rows_are_a_gap_apart() {
        assert_eq!(
            stack(&[36, 20, 36], &[1, 1]),
            (vec![16, 76, 120], vec![0, 52, 96], 172)
        );
        // Each track beyond the first adds 10px to its gap.
        assert_eq!(stack(&[36, 36], &[3]), (vec![16, 96], vec![0, 52], 148));
    }

    #[test]
    fn one_node_per_row() {
        for text in [
            "feature start\n  show 1\n  show 2\n",
            "feature start\n  if 1 is 1\n    show 1\n  otherwise\n    show 2\n  show 3\n",
            "feature start\n  for each i from 1 to 3\n    show i\n  show 0\n",
        ] {
            let l = layout(&graph_of(text));
            assert_eq!(l.boxes.len(), l.column.len());
            for pair in l.boxes.windows(2) {
                assert!(pair[1].y > pair[0].y + pair[0].h, "{pair:?}");
            }
        }
    }
}
