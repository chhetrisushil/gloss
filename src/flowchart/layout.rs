//! Placing the flowchart's nodes: a row each, in branch columns with loop lanes (D62, D63).

use super::graph::{Graph, Node, Region, Shape};
use super::route;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const MARGIN: i32 = 16;

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
    /// The x of the centre of each column, empty `no` columns included. Vertical arrows run
    /// here; each box is centred on it to within half a pixel.
    pub centres: Vec<i32>,
    /// Loop decision node -> x of its back and done lanes.
    pub loops: BTreeMap<usize, (i32, i32)>,
    pub give_lane: i32,
    /// The y of the top of the gap above each row; `gaps[0]` is 0 and unused (row 0 has no gap).
    pub gaps: Vec<i32>,
    /// Decision of an `if` condition -> the column its `no` arrow runs down.
    pub no_cols: BTreeMap<usize, usize>,
    /// One polyline per arrow, start first: every segment is horizontal or vertical.
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

/// Gives the nodes `lo..hi` columns from `col` on (D62), noting each loop's decision and first
/// and last column in `spans`. Returns the last column used.
fn place(g: &Graph, (lo, hi): (usize, usize), col: usize, out: &mut Columns) -> usize {
    let (mut last, mut i) = (col, lo);
    while i < hi {
        match g.regions.iter().find(|r| r.start() == i) {
            Some(Region::Loop(span)) => {
                out.column[i] = col;
                let inner = place(g, (i + 1, span.end), col, out);
                out.spans.push((i, col, inner));
                last = last.max(inner);
                i = span.end;
            }
            Some(Region::If {
                branches,
                otherwise,
            }) => {
                let mut c = col;
                for b in branches {
                    out.column[b.start] = c;
                    c = place(g, (b.start + 1, b.end), c, out) + 1;
                    out.no_cols.insert(b.start, c);
                }
                last = last.max(place(g, (otherwise.start, otherwise.end), c, out));
                i = otherwise.end;
            }
            None => {
                out.column[i] = col;
                i += 1;
            }
        }
    }
    last
}

struct Columns {
    column: Vec<usize>,
    no_cols: BTreeMap<usize, usize>,
    /// Each loop's decision, first column and last column.
    spans: Vec<(usize, usize, usize)>,
}

pub(crate) fn layout(g: &Graph) -> Layout {
    let sizes: Vec<(i32, i32)> = g.nodes.iter().map(size).collect();
    let heights: Vec<i32> = sizes.iter().map(|s| s.1).collect();
    let mut cols = Columns {
        column: vec![0; sizes.len()],
        no_cols: BTreeMap::new(),
        spans: vec![],
    };
    let count = place(g, (0, sizes.len()), 0, &mut cols) + 1;
    let Columns {
        column,
        no_cols,
        mut spans,
    } = cols;
    // Each gap is as tall as the tracks that cross it need.
    let sets = route::tracks(g, &column, &no_cols);
    let tracks: Vec<usize> = (1..sizes.len())
        .map(|gap| sets.get(&gap).map_or(0, BTreeSet::len))
        .collect();
    let (ys, gaps, height) = stack(&heights, &tracks);
    spans.sort();
    let mut widths = vec![0; count];
    for (&c, s) in column.iter().zip(&sizes) {
        widths[c] = widths[c].max(s.0);
    }
    // The gap before column `k` holds the done lanes of the loops that end in column `k - 1`,
    // inner first, then the back lanes of those that start in `k`, outer first (D63).
    let (mut xs, mut loops, mut right) = (vec![], BTreeMap::<usize, (i32, i32)>::new(), 0);
    for k in 0..=count {
        let mut x = right;
        for s in spans.iter().rev().filter(|s| s.2 + 1 == k) {
            x += MARGIN;
            loops.entry(s.0).or_default().1 = x;
        }
        for s in spans.iter().filter(|s| s.1 == k) {
            x += MARGIN;
            loops.entry(s.0).or_default().0 = x;
        }
        x += MARGIN;
        xs.push(x);
        right = x + widths.get(k).copied().unwrap_or(0);
    }
    let give_lane = xs[count];
    let centres: Vec<i32> = (0..count).map(|c| xs[c] + widths[c] / 2).collect();
    let boxes = (sizes.iter().zip(&ys).zip(&column))
        .map(|((&(w, h), &y), &c)| Rect {
            x: centres[c] - w / 2,
            y,
            w,
            h,
        })
        .collect();
    let mut layout = Layout {
        width: give_lane + MARGIN,
        height,
        boxes,
        column,
        centres,
        loops,
        give_lane,
        gaps,
        no_cols,
        routes: vec![],
    };
    layout.routes = route::route(g, &layout);
    layout
}

#[cfg(test)]
mod tests {
    use super::super::graph::Region;
    use super::*;
    use crate::source::Source;

    fn graph_of(text: &str) -> Graph {
        super::super::check::program(text);
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
        // A gap with no track is as tall as one with a single track.
        assert_eq!(stack(&[36, 36], &[0]), (vec![16, 76], vec![0, 52], 128));
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

    const CHAIN: &str = "feature start\n  set n to 1\n  if n is 1\n    set n to 2\n  otherwise if n is 2\n    set n to 5\n  otherwise\n    set n to 4\n  show n\n";
    const NESTED: &str = "feature start\n  for each i from 1 to 3\n    if i is 2\n      show i\n    otherwise\n      show 0\n    for each j from 1 to 3\n      show j\n  show 9\n";

    #[test]
    fn an_if_chain_steps_right() {
        let l = layout(&graph_of(CHAIN));
        assert_eq!(l.column, [0, 0, 0, 0, 1, 1, 2, 0, 0]);
        // An if with no otherwise keeps an empty column for its no path.
        let l = layout(&graph_of(
            "feature start\n  if 1 is 1\n    show 1\n  show 2\n",
        ));
        assert_eq!(l.column, [0; 5]);
        assert_eq!(l.centres, [86, 173]);
        assert_eq!(l.give_lane, l.centres[1] + MARGIN);
    }

    #[test]
    fn boxes_never_overlap_in_x_across_columns() {
        for text in [CHAIN, NESTED] {
            let l = layout(&graph_of(text));
            for (a, ba) in l.boxes.iter().enumerate() {
                for (b, bb) in l.boxes.iter().enumerate() {
                    if l.column[a] < l.column[b] {
                        assert!(ba.x + ba.w < bb.x, "{a} {b}");
                    }
                }
            }
        }
    }

    #[test]
    fn loop_lanes_bracket_the_loop() {
        let g = graph_of(NESTED);
        let l = layout(&g);
        for r in &g.regions {
            if let Region::Loop(span) = r {
                let (back, done) = l.loops[&span.start];
                for b in &l.boxes[span.clone()] {
                    assert!(back < b.x && b.x + b.w < done, "{span:?}");
                }
                assert!(MARGIN <= back && done + MARGIN <= l.give_lane);
            }
        }
        let (outer, inner) = (l.loops[&1], l.loops[&5]);
        assert!(outer.0 < inner.0 && inner.1 < outer.1);
        // Loops that end in the same column keep inner done lanes inside outer ones, and a
        // loop's body is in the loop's column.
        let two =
            "feature start\n  for each i from 1 to 3\n    for each j from 1 to 3\n      show j\n";
        let l = layout(&graph_of(two));
        assert_eq!(l.column, [0; 5]);
        assert!(l.loops[&1].0 < l.loops[&2].0 && l.loops[&2].1 < l.loops[&1].1);
    }

    #[test]
    fn layouts_keep_to_the_geometry() {
        let wide = "a".repeat(41);
        let corpus = [
            "feature start\n  set n to 0\n  if n is 1\n    for each i from 1 to 3\n      if i is 1\n        show i\n      otherwise if i is 2\n        show 2\n    show 5\n  otherwise if n is 2\n    show 3\n  otherwise\n    for each j from 1 to 2\n      show j\n  show n\n",
            "feature start\n  if 1 is 1\n    show 1\n  otherwise if 2 is 2\n    for each i from 1 to 3\n      show i\n  show 2\n",
            "feature start\n  for each i from 1 to 3\n    show i\n  for each j from 1 to 3\n    show j\n",
            "feature start\n  if 1 is 1\n    note nothing\n  otherwise\n    note nothing\n  show 1\n",
            "feature start\n  if 1 is 1\n    show 1\n  otherwise\n    for each i from 1 to 3\n      note nothing\n  show 2\n",
            "feature start\n  for each i from 1 to 3\n    if i is 1\n      for each j from 1 to 3\n        if j is 1\n          for each k from 1 to 3\n            if k is 1\n              skip\n            otherwise\n              stop\n            show k\n        show j\n    show i\n",
            "feature start\n  show \"WIDE\"\n  show 22\n  if 1 is 1\n    show 1\n",
            "feature start\n  if 1 is 1\n    if 2 is 2\n      show 1\n    otherwise if 3 is 3\n      show 2\n  otherwise if 4 is 4\n    show 3\n  show 4\n",
            "feature start\n  if 1 is 1\n    show 12\n  show 1\n",
            "feature start\n  if 1 is 1\n    if 2 is 2\n      show 1\n    otherwise\n      show 2\n  otherwise\n    show 4\n  show 5\n",
            "feature start\n  if 1 is 1\n    if 2 is 2\n      show 1\n    otherwise if 3 is 3\n      show 2\n    otherwise if 4 is 4\n      show 3\n  otherwise if 5 is 5\n    show 4\n  show 5\n",
            // Two loops that end in the same column: their done lanes nest.
            "feature start\n  for each i from 1 to 3\n    for each j from 1 to 3\n      show j\n  show 1\n",
            CHAIN,
            NESTED,
        ];
        for text in corpus {
            let g = graph_of(&text.replace("WIDE", &wide));
            super::super::check::geometry(&g, &layout(&g))
                .unwrap_or_else(|e| panic!("{e}\n{text}"));
        }
    }

    #[test]
    fn a_nested_if_then_a_loop_in_the_next_branch_has_exact_numbers() {
        let text = "feature start\n  if 1 is 1\n    if 2 is 2\n      show 1\n  otherwise if 3 is 3\n    for each i from 1 to 3\n      show i\n";
        let l = layout(&graph_of(text));
        let xs: Vec<i32> = l.boxes.iter().map(|b| b.x).collect();
        assert_eq!(l.column, [0, 0, 0, 0, 2, 2, 2, 0]);
        assert_eq!(xs, [16, 16, 16, 47, 219, 205, 295, 61]);
        assert_eq!(l.centres, [86, 173, 334, 495]);
        assert_eq!(l.loops[&5], (189, 479));
        assert_eq!((l.give_lane, l.width), (511, 527));
    }
}
