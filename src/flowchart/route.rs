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
    use super::super::graph::{ArrowKind::*, Graph, graph};
    use super::super::layout::{Layout, layout};
    use crate::source::Source;

    fn laid(text: &str) -> (Graph, Layout) {
        super::super::check::program(text);
        let (_, checked) = crate::check_program(text);
        let checked = checked.expect("the program checks");
        let feature = checked.program().features().next().expect("a feature");
        let g = graph(feature, &Source::new(text), &checked.types);
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

    const NESTED: &str =
        "feature start\n  for each i from 1 to 3\n    for each j from 1 to 3\n      show j\n";
    const STOP_LAST: &str = "feature start\n  for each i from 1 to 3\n    for each j from 1 to 3\n      stop\n  show 0\n";
    const STOP_AFTER: &str = "feature start\n  for each i from 1 to 3\n    for each j from 1 to 3\n      show j\n    stop\n  show 0\n";
    const LOOP_FIRST: &str = "feature start\n  if 1 is 1\n    for each i from 1 to 3\n      show i\n  otherwise\n    show 1\n  show 2\n";
    const PLAIN: &str = "feature start\n  show 1\n  show 2\n  show 3\n";
    const MORE: [&str; 6] = [
        NESTED,
        STOP_LAST,
        STOP_AFTER,
        LOOP_FIRST,
        PLAIN,
        "feature start\n  for each i from 1 to 3\n    if i is 1\n      skip\n    otherwise if i is 2\n      stop\n    otherwise\n      show i\n  show 0\n",
    ];

    fn gap_above(l: &Layout, row: usize) -> i32 {
        l.boxes[row].y - (l.boxes[row - 1].y + l.boxes[row - 1].h)
    }

    fn stop_arrow(g: &Graph) -> usize {
        g.arrows.iter().position(|a| a.kind == Stop).unwrap()
    }

    #[test]
    fn a_no_arrow_leaves_from_the_right_point() {
        let (g, l) = laid(CHAIN);
        let a = g
            .arrows
            .iter()
            .position(|a| (a.from, a.to, a.kind) == (2, 4, No))
            .unwrap();
        let b = l.boxes[2];
        let y = b.y + b.h / 2;
        assert_eq!(l.routes[a][..2], [(b.x + b.w, y), (l.centres[1], y)]);
        // With no otherwise, it runs to the empty column to the right of the branch.
        let (g, l) = laid(OPEN);
        let a = g.arrows.iter().position(|a| a.kind == No).unwrap();
        let b = l.boxes[1];
        assert_eq!(l.routes[a][1], (l.centres[1], b.y + b.h / 2));
    }

    #[test]
    fn stop_and_done_share_the_done_lane() {
        let (g, l) = laid(JUMPS);
        let lane = l.loops[&1].1;
        for kind in [Done, Stop] {
            let a = g.arrows.iter().position(|a| a.kind == kind).unwrap();
            let r = &l.routes[a];
            let b = l.boxes[g.arrows[a].from];
            assert_eq!(r[..2], [(b.x + b.w, b.y + b.h / 2), (lane, b.y + b.h / 2)]);
            assert_eq!(r[2].0, lane);
        }
    }

    #[test]
    fn arrows_to_one_target_share_a_track() {
        let (g, l) = laid(CHAIN);
        let into_end: Vec<usize> = (0..g.arrows.len())
            .filter(|&i| g.arrows[i].to == 7)
            .collect();
        assert_eq!(into_end.len(), 3);
        let tracks: Vec<i32> = into_end
            .iter()
            .filter(|&&i| l.routes[i].len() > 2)
            .map(|&i| l.routes[i][l.routes[i].len() - 2].1)
            .collect();
        assert!(!tracks.is_empty() && tracks.iter().all(|&y| y == l.gaps[7] + 12));
    }

    #[test]
    fn a_stop_leaves_by_the_innermost_loops_lane() {
        // Inside the inner loop, after it, and as a back arrow that ends both loops.
        let text = "feature start\n  for each i from 1 to 3\n    for each j from 1 to 3\n      if j is 2\n        stop\n      show j\n    show i\n  show 0\n";
        let (g, l) = laid(text);
        assert_eq!(l.routes[stop_arrow(&g)][1].0, l.loops[&2].1);
        let (g, l) = laid(STOP_LAST);
        let a = stop_arrow(&g);
        assert!(g.arrows[a].is_back());
        assert_eq!(l.routes[a][1].0, l.loops[&2].1);
        assert_ne!(l.routes[a][1].0, l.loops[&1].1);
        // A stop on the row just after the inner loop is outside it.
        let (g, l) = laid(STOP_AFTER);
        assert_eq!(l.routes[stop_arrow(&g)][1].0, l.loops[&1].1);
    }

    #[test]
    fn nested_loops_ending_together_get_separate_back_tracks() {
        let (g, l) = laid(NESTED);
        let track = |i: usize| l.routes[i][l.routes[i].len() - 3].1;
        // The outer loop's back arrow sits above the inner one's (R14), 10px apart.
        let (outer, inner) = (arrow(&g, 2, 1), arrow(&g, 3, 2));
        assert_eq!(
            (track(outer), track(inner)),
            (l.gaps[4] + 12, l.gaps[4] + 22)
        );
        // The outer `Done` past both loops runs forward below them: three targets, 44px.
        let past = &l.routes[arrow(&g, 1, 4)];
        assert_eq!(past[past.len() - 2].1, l.gaps[4] + 32);
        assert_eq!(gap_above(&l, 4), 44);
    }

    fn span(s: (super::Point, super::Point)) -> (bool, i32, i32, i32) {
        let (a, b) = s;
        if a.1 == b.1 {
            (true, a.1, a.0.min(b.0), a.0.max(b.0))
        } else {
            (false, a.0, a.1.min(b.1), a.1.max(b.1))
        }
    }

    #[test]
    fn arrows_with_different_targets_never_share_a_line() {
        for text in ALL.into_iter().chain(MORE) {
            let (g, l) = laid(text);
            let segs: Vec<Vec<_>> = l
                .routes
                .iter()
                .map(|r| r.windows(2).map(|w| span((w[0], w[1]))).collect())
                .collect();
            for (i, si) in segs.iter().enumerate() {
                for (j, sj) in segs
                    .iter()
                    .enumerate()
                    .skip(i + 1)
                    .filter(|(j, _)| g.arrows[*j].to != g.arrows[i].to)
                {
                    for (a, b) in si.iter().flat_map(|a| sj.iter().map(move |b| (a, b))) {
                        let shared = a.0 == b.0 && a.1 == b.1 && a.3.min(b.3) > a.2.max(b.2);
                        assert!(!shared, "arrows {i} and {j} share {a:?} {b:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn every_arrow_starts_where_its_kind_does() {
        for text in ALL.into_iter().chain(MORE) {
            let (g, l) = laid(text);
            for (i, (a, r)) in g.arrows.iter().zip(&l.routes).enumerate() {
                let b = l.boxes[a.from];
                let mid = b.y + b.h / 2;
                let want = match a.kind {
                    No | Done | Stop | Give => (b.x + b.w, mid),
                    Skip => (b.x, mid),
                    _ => (l.centres[l.column[a.from]], b.y + b.h),
                };
                assert_eq!(r[0], want, "arrow {i} {a:?} in {text}");
                // A side start runs sideways first; a bottom start drops first.
                let sideways = r[1].1 == r[0].1;
                assert_eq!(
                    sideways,
                    want != (l.centres[l.column[a.from]], b.y + b.h),
                    "{i} {r:?}"
                );
            }
        }
    }

    #[test]
    fn a_drop_through_adds_no_track() {
        // `No` into the `otherwise` column drops straight; only the back arrow runs in the gap.
        let (g, l) = laid(LOOP_FIRST);
        assert_eq!(gap_above(&l, 4), 24);
        let no = g
            .arrows
            .iter()
            .position(|a| a.kind == No && a.from == 1)
            .unwrap();
        assert_eq!(l.routes[no].len(), 3);
        // A gap that nothing runs along is 24px as well.
        let (_, l) = laid(PLAIN);
        assert!((1..4).all(|row| gap_above(&l, row) == 24));
    }
}
