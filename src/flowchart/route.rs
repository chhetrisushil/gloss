//! Routing the arrows as horizontal and vertical segments through the gaps (D62, D63).

use super::graph::{Arrow, ArrowKind, Graph, Region};
use super::layout::{Layout, Rect};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Point = (i32, i32);

/// The column a forward arrow first runs down, or `None` if it runs down a lane.
fn drop_column(a: &Arrow, column: &[usize], no_cols: &BTreeMap<usize, usize>) -> Option<usize> {
    match a.kind {
        ArrowKind::No => no_cols.get(&a.from).copied(),
        ArrowKind::Done | ArrowKind::Stop | ArrowKind::Give | ArrowKind::Skip => None,
        _ => Some(column[a.from]),
    }
}

/// Whether the arrow only drops through, in the column it ends in: it has no horizontal run
/// and so no track (R17).
fn drops(a: &Arrow, column: &[usize], no_cols: &BTreeMap<usize, usize>) -> bool {
    !a.is_back() && drop_column(a, column, no_cols) == Some(column[a.to])
}

/// The gap (named by the row below it) that holds the arrow's track, if it has one. A back
/// arrow's is the gap below its loop's last row.
fn gap(g: &Graph, a: &Arrow, column: &[usize], no_cols: &BTreeMap<usize, usize>) -> Option<usize> {
    if a.kind == ArrowKind::Skip || drops(a, column, no_cols) {
        return None;
    }
    let end = g.regions.iter().find_map(|r| match r {
        Region::Loop(s) if s.start == a.to => Some(s.end),
        _ => None,
    });
    Some(if a.is_back() { end? } else { a.to })
}

/// For each gap, the target rows of the arrows with a track in it: one track per target.
pub(super) fn tracks(
    g: &Graph,
    column: &[usize],
    no_cols: &BTreeMap<usize, usize>,
) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut sets = BTreeMap::<usize, BTreeSet<usize>>::new();
    for a in &g.arrows {
        if let Some(gap) = gap(g, a, column, no_cols) {
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
        let side = (from.x + from.w, mid(from));
        // Where the arrow starts, and the x of its first vertical run.
        let (start, down) = match a.kind {
            ArrowKind::No if !l.loops.contains_key(&a.from) => {
                (side, l.centres[l.no_cols[&a.from]])
            }
            ArrowKind::No | ArrowKind::Done => (side, l.loops[&a.from].1),
            ArrowKind::Stop => (side, l.loops[&innermost(g, a.from)].1),
            ArrowKind::Give => (side, l.give_lane),
            ArrowKind::Skip => ((from.x, mid(from)), l.loops[&a.to].0),
            _ => ((centre(a.from), from.y + from.h), centre(a.from)),
        };
        let track = gap(g, a, &l.column, &l.no_cols).map(|gap| {
            let t = sets[&gap]
                .iter()
                .position(|&t| t == a.to)
                .expect("its track");
            l.gaps[gap] + 12 + 10 * t as i32
        });
        let mut points = vec![start, (down, start.1)];
        points.extend(track.map(|y| (down, y)));
        if a.is_back() {
            let lane = l.loops[&a.to].0;
            points.extend(track.map(|y| (lane, y)));
            points.extend([(lane, mid(to)), (to.x, mid(to))]);
        } else {
            points.extend(track.map(|y| (centre(a.to), y)));
            points.push((centre(a.to), to.y));
        }
        points.dedup();
        routes.push(points);
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

    const BACK: &str =
        "feature start\n  for each i from 1 to 3\n    show i\n    show i\n  show 0\n";
    const SKIP: &str =
        "feature start\n  for each i from 1 to 3\n    if i is 1\n      skip\n    show i\n";
    const GIVE: &str = "feature f\n  if 1 is 1\n    give 1\n  give 2\n\nfeature start\n  show f\n";
    const EMPTY: &str = "feature start\n  for each i from 1 to 3\n    note nothing\n  show 1\n";
    const INNER: &str = "feature start\n  for each i from 1 to 3\n    for each j from 1 to 3\n      show j\n  show 0\n";
    const TAIL_IF: &str =
        "feature start\n  for each i from 1 to 3\n    if i is 1\n      show i\n  show 0\n";
    const ALL: [&str; 10] = [
        CHAIN, JUMPS, OPEN, REPEAT, BACK, SKIP, GIVE, EMPTY, INNER, TAIL_IF,
    ];

    /// The point where a route ends: a decision's left point if it goes back, else a top centre.
    fn end_of(g: &Graph, l: &Layout, i: usize) -> super::Point {
        let (a, to) = (g.arrows[i], l.boxes[g.arrows[i].to]);
        if a.is_back() {
            (to.x, to.y + to.h / 2)
        } else {
            (l.centres[l.column[a.to]], to.y)
        }
    }

    #[test]
    fn every_segment_is_straight() {
        for text in ALL {
            let (g, l) = laid(text);
            assert_eq!(l.routes.len(), g.arrows.len());
            for (i, r) in l.routes.iter().enumerate() {
                assert!(r.len() >= 2, "arrow {i}: {r:?}");
                let straight = |p: &[super::Point]| p[0].0 == p[1].0 || p[0].1 == p[1].1;
                assert!(r.windows(2).all(|p| p[0] != p[1] && straight(p)), "{r:?}");
            }
        }
    }

    #[test]
    fn routes_end_on_their_target() {
        for text in ALL {
            let (g, l) = laid(text);
            for (i, r) in l.routes.iter().enumerate() {
                assert_eq!(r.last(), Some(&end_of(&g, &l, i)), "arrow {i}");
            }
        }
    }

    #[test]
    fn a_next_arrow_in_its_column_drops_straight_down() {
        let (g, l) = laid(CHAIN);
        let a = g
            .arrows
            .iter()
            .position(|a| (a.from, a.to) == (0, 1))
            .unwrap();
        let (x, from, to) = (l.centres[0], l.boxes[0], l.boxes[1]);
        assert_eq!(l.routes[a], [(x, from.y + from.h), (x, to.y)]);
    }

    fn arrow(g: &Graph, from: usize, to: usize) -> usize {
        g.arrows
            .iter()
            .position(|a| (a.from, a.to) == (from, to))
            .unwrap()
    }

    #[test]
    fn a_back_arrow_uses_the_back_lane() {
        let (g, l) = laid(BACK);
        let (b3, b1) = (l.boxes[3], l.boxes[1]);
        let (x, lane, y) = (l.centres[0], l.loops[&1].0, l.gaps[4] + 12);
        let into = (b1.x, b1.y + b1.h / 2);
        let want = [(x, b3.y + b3.h), (x, y), (lane, y), (lane, into.1), into];
        assert_eq!(l.routes[arrow(&g, 3, 1)], want);
        // An empty body gives the self arrow, which leaves from the decision's bottom centre.
        let (g, l) = laid(EMPTY);
        let (b, lane, y) = (l.boxes[1], l.loops[&1].0, l.gaps[2] + 12);
        let mid = b.y + b.h / 2;
        let want = [(x, b.y + b.h), (x, y), (lane, y), (lane, mid), (b.x, mid)];
        assert_eq!(l.routes[arrow(&g, 1, 1)], want);
    }

    #[test]
    fn a_skip_goes_left_then_up_the_back_lane() {
        let (g, l) = laid(SKIP);
        let (b3, b1) = (l.boxes[3], l.boxes[1]);
        let (lane, y3, y1) = (l.loops[&1].0, b3.y + b3.h / 2, b1.y + b1.h / 2);
        let want = [(b3.x, y3), (lane, y3), (lane, y1), (b1.x, y1)];
        assert_eq!(l.routes[arrow(&g, 3, 1)], want);
    }

    #[test]
    fn a_done_arrow_that_goes_back_leaves_by_the_inner_done_lane() {
        let (g, l) = laid(INNER);
        let r = &l.routes[arrow(&g, 2, 1)];
        assert_eq!(r[1].0, l.loops[&2].1);
        assert_eq!(r[r.len() - 3].0, l.loops[&1].0);
    }

    #[test]
    fn give_uses_the_give_lane() {
        let (g, l) = laid(GIVE);
        let (r, b) = (&l.routes[arrow(&g, 2, 4)], l.boxes[2]);
        let y = b.y + b.h / 2;
        assert_eq!(r[..2], [(b.x + b.w, y), (l.give_lane, y)]);
        assert_eq!(r[2].0, l.give_lane);
    }

    #[test]
    fn two_targets_two_tracks() {
        let (g, l) = laid(TAIL_IF);
        // The back arrows (Next and No, to the loop) and Done (past it) cross the gap above row 4.
        let (back, no, done) = (arrow(&g, 3, 1), arrow(&g, 2, 1), arrow(&g, 1, 4));
        let track = |i: usize| l.routes[i][2].1;
        assert_eq!((track(back), track(no)), (l.gaps[4] + 12, l.gaps[4] + 12));
        assert_eq!(track(done), l.gaps[4] + 22);
        let above = l.boxes[3].y + l.boxes[3].h;
        assert_eq!(l.boxes[4].y - above, 34);
    }
}
