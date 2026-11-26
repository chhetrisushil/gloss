//! Routing the arrows as horizontal and vertical segments through the gaps (D62, D63).

use super::graph::{Arrow, ArrowKind, Graph, Region};
use super::layout::{Layout, Rect};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Point = (i32, i32);

/// The column a forward arrow first runs down, or `None` if it runs down a lane.
fn drop_column(a: &Arrow, column: &[usize], no_cols: &BTreeMap<usize, usize>) -> Option<usize> {
    match a.kind {
        ArrowKind::No => no_cols.get(&a.from).copied(),
        ArrowKind::Done | ArrowKind::Stop => None,
        _ => Some(column[a.from]),
    }
}

/// Whether the arrow only drops through, in the column it ends in: it has no horizontal run
/// and so no track (R17).
fn drops(a: &Arrow, column: &[usize], no_cols: &BTreeMap<usize, usize>) -> bool {
    !a.is_back() && drop_column(a, column, no_cols) == Some(column[a.to])
}

/// The gap (named by the row below it) that holds the arrow's track, if it has one.
fn gap(a: &Arrow, column: &[usize], no_cols: &BTreeMap<usize, usize>) -> Option<usize> {
    (!a.is_back() && !drops(a, column, no_cols)).then_some(a.to)
}

/// For each gap, the target rows of the arrows with a track in it: one track per target.
pub(super) fn tracks(
    g: &Graph,
    column: &[usize],
    no_cols: &BTreeMap<usize, usize>,
) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut sets = BTreeMap::<usize, BTreeSet<usize>>::new();
    for a in &g.arrows {
        if let Some(gap) = gap(a, column, no_cols) {
            sets.entry(gap).or_default().insert(a.to);
        }
    }
    sets
}

/// The decision of the innermost loop around node `n`.
fn innermost(g: &Graph, n: usize) -> usize {
    let spans = g.regions.iter().filter_map(|r| match r {
        Region::Loop(s) if s.start < n && n < s.end => Some(s.start),
        _ => None,
    });
    spans.max().expect("a stop is inside a loop")
}

pub(super) fn route(g: &Graph, l: &Layout) -> Vec<Vec<Point>> {
    let sets = tracks(g, &l.column, &l.no_cols);
    let centre = |n: usize| l.centres[l.column[n]];
    let mid = |r: Rect| r.y + r.h / 2;
    let mut routes = vec![];
    for a in &g.arrows {
        let (from, to) = (l.boxes[a.from], l.boxes[a.to]);
        let route = if a.is_back() {
            vec![]
        } else {
            let side = (from.x + from.w, mid(from));
            let (start, down) = match a.kind {
                ArrowKind::No if !l.loops.contains_key(&a.from) => {
                    (side, l.centres[l.no_cols[&a.from]])
                }
                ArrowKind::No | ArrowKind::Done => (side, l.loops[&a.from].1),
                ArrowKind::Stop => (side, l.loops[&innermost(g, a.from)].1),
                _ => ((centre(a.from), from.y + from.h), centre(a.from)),
            };
            let mut points = vec![start, (down, start.1)];
            if let Some(gap) = gap(a, &l.column, &l.no_cols) {
                let t = sets[&gap]
                    .iter()
                    .position(|&t| t == a.to)
                    .expect("its track");
                let y = l.gaps[gap] + 12 + 10 * t as i32;
                points.extend([(down, y), (centre(a.to), y)]);
            }
            points.push((centre(a.to), to.y));
            points.dedup();
            points
        };
        routes.push(route);
    }
    routes
}

#[cfg(test)]
mod tests {
    use super::super::graph::{Graph, graph};
    use super::super::layout::{Layout, layout};
    use crate::source::Source;

    fn laid(text: &str) -> (Graph, Layout) {
        let (_, checked) = crate::check_program(text);
        let checked = checked.expect("the program checks");
        let feature = checked.program().features().next().expect("a feature");
        let g = graph(feature, &Source::new(text));
        let l = layout(&g);
        (g, l)
    }

    const CHAIN: &str = "feature start\n  set n to 1\n  if n is 1\n    set n to 2\n  otherwise if n is 2\n    set n to 5\n  otherwise\n    set n to 4\n  show n\n";
    const JUMPS: &str = "feature start\n  for each i from 1 to 5\n    if i is 1\n      show 1\n    if i is 2\n      stop\n    show i\n  show 9\n";
    const OPEN: &str = "feature start\n  if 1 is 1\n    show 1\n  show 2\n";
    const REPEAT: &str =
        "feature start\n  set n to 0\n  repeat while n is less than 3\n    add 1 to n\n  show n\n";

    /// The routes of the arrows that go forward.
    fn forward(g: &Graph, l: &Layout) -> Vec<(usize, Vec<super::Point>)> {
        assert_eq!(l.routes.len(), g.arrows.len());
        let all = g.arrows.iter().zip(&l.routes);
        let all = all.enumerate().filter(|(_, (a, _))| !a.is_back());
        all.map(|(i, (_, r))| (i, r.clone())).collect()
    }

    #[test]
    fn every_segment_is_straight() {
        for text in [CHAIN, JUMPS, OPEN, REPEAT] {
            let (g, l) = laid(text);
            assert_eq!(l.routes.len(), g.arrows.len());
            for (i, r) in forward(&g, &l) {
                assert!(r.len() >= 2, "arrow {i}: {r:?}");
                assert!(
                    r.windows(2)
                        .all(|p| p[0] != p[1] && (p[0].0 == p[1].0 || p[0].1 == p[1].1))
                );
            }
        }
    }

    #[test]
    fn routes_end_on_their_target() {
        for text in [CHAIN, JUMPS, OPEN, REPEAT] {
            let (g, l) = laid(text);
            for (i, r) in forward(&g, &l) {
                let to = g.arrows[i].to;
                let top = (l.centres[l.column[to]], l.boxes[to].y);
                assert_eq!(r.last(), Some(&top), "arrow {i}");
            }
        }
    }

    #[test]
    fn a_next_arrow_in_its_column_drops_straight_down() {
        let (g, l) = laid(CHAIN);
        let a = g.arrows.iter().position(|a| (a.from, a.to) == (0, 1));
        let (x, from, to) = (l.centres[0], l.boxes[0], l.boxes[1]);
        assert_eq!(l.routes[a.unwrap()], [(x, from.y + from.h), (x, to.y)]);
    }
}
