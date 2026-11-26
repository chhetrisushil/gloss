//! Test-only checks that a finished layout is free of overlapping boxes and lines (D62, D63, D65).

use super::graph::{Arrow, ArrowKind, Graph, Region};
use super::layout::{Layout, MARGIN, Rect, layout};
use crate::ast::program::FeatureDecl;
use crate::ast::stmt::{Stmt, StmtKind};
use std::collections::{BTreeMap, BTreeSet};

/// The geometry rules of D62 and D63, checked on a finished layout.
pub(super) fn geometry(g: &Graph, l: &Layout) -> Result<(), String> {
    let right = |b: &Rect| b.x + b.w;
    let (mut lanes, mut spans) = (vec![l.give_lane], vec![]);
    for (n, a) in l.boxes.iter().enumerate() {
        if a.x < MARGIN || right(a) > l.width - MARGIN || a.y < MARGIN {
            return Err(format!("box {n} outside the margins"));
        }
        if a.y + a.h > l.height - MARGIN || a.x != l.centres[l.column[n]] - a.w / 2 {
            return Err(format!(
                "box {n} outside the height or off its column centre"
            ));
        }
        for (m, b) in l.boxes.iter().enumerate() {
            if l.column[n] < l.column[m] && right(a) >= b.x {
                return Err(format!("boxes {n} and {m} overlap in x"));
            }
        }
    }
    for r in &g.regions {
        match r {
            Region::Loop(span) => {
                let (back, done) = l.loops[&span.start];
                lanes.extend([back, done]);
                spans.push((span.clone(), back, done));
                for b in &l.boxes[span.clone()] {
                    if back >= b.x || right(b) >= done {
                        return Err(format!("{span:?} lanes do not bracket the loop"));
                    }
                }
            }
            Region::If {
                branches,
                otherwise,
            } => {
                let mut earlier = 0;
                for b in branches.iter().chain([otherwise]).filter(|b| !b.is_empty()) {
                    let left = l.boxes[b.clone()].iter().map(|r| r.x).min().unwrap();
                    if left <= earlier {
                        return Err(format!("{b:?} does not start right of the branches before"));
                    }
                    let lane = l.loops.range(b.clone()).map(|(_, v)| v.1);
                    let edge = l.boxes[b.clone()].iter().map(right).chain(lane).max();
                    earlier = earlier.max(edge.unwrap());
                }
            }
        }
    }
    if lanes.iter().any(|&x| x < MARGIN || x > l.give_lane) {
        return Err("a lane is outside the chart".into());
    }
    let mut all = lanes.clone();
    all.extend(&l.centres);
    all.sort();
    if all.windows(2).any(|w| w[0] == w[1]) {
        return Err(format!("two lanes or centres coincide: {all:?}"));
    }
    for (inner, ib, id) in &spans {
        for (outer, ob, od) in &spans {
            if outer.start < inner.start && inner.end <= outer.end && (ob >= ib || id >= od) {
                return Err(format!("{inner:?} lanes are not inside {outer:?}"));
            }
        }
        let first = l.column[inner.start];
        let last = l.column[inner.clone()].iter().max().unwrap();
        for (n, b) in l.boxes.iter().enumerate() {
            let crosses = [ib, id].iter().any(|&&x| b.x <= x && x <= right(b));
            let (c, in_span) = (l.column[n], inner.contains(&n));
            if crosses && in_span || c < first && right(b) >= *ib || c > *last && b.x <= *id {
                return Err(format!("a lane of {inner:?} is out of its gap at box {n}"));
            }
        }
    }
    Ok(())
}

/// The nodes a block makes: every statement but `note`, and one per `if` condition.
fn count(block: &[Stmt]) -> usize {
    block
        .iter()
        .map(|s| match &s.kind {
            StmtKind::Note(_) => 0,
            StmtKind::If {
                branches,
                otherwise,
            } => {
                let inner = branches.iter().map(|(_, b)| count(b)).sum::<usize>();
                branches.len() + inner + otherwise.as_deref().map_or(0, count)
            }
            StmtKind::ForEachIn { body, .. }
            | StmtKind::ForEachRange { body, .. }
            | StmtKind::RepeatWhile { body, .. } => 1 + count(body),
            _ => 1,
        })
        .sum()
}

fn overlap_1d(a: (i32, i32), b: (i32, i32)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

type Seg = ((i32, i32), (i32, i32));

fn segs(r: &[(i32, i32)]) -> impl Iterator<Item = Seg> + '_ {
    r.windows(2).map(|w| (w[0], w[1]))
}

/// A segment's x and y ranges.
fn span(s: Seg) -> ((i32, i32), (i32, i32)) {
    let ((a, b), (c, d)) = s;
    ((a.min(c), a.max(c)), (b.min(d), b.max(d)))
}

fn mid(r: &Rect) -> i32 {
    r.y + r.h / 2
}

/// Where a route starts and ends: a side of the source for `no`, `done`, `stop` and `give`, the
/// left point for `skip`, else the bottom centre; the target's left point if it goes back, else
/// its top centre (D63).
fn ends(a: &Arrow, l: &Layout) -> ((i32, i32), (i32, i32)) {
    let (b, t) = (&l.boxes[a.from], &l.boxes[a.to]);
    let start = match a.kind {
        ArrowKind::No | ArrowKind::Done | ArrowKind::Stop | ArrowKind::Give => (b.x + b.w, mid(b)),
        ArrowKind::Skip => (b.x, mid(b)),
        _ => (b.x + b.w / 2, b.y + b.h),
    };
    let end = if a.is_back() {
        (t.x, mid(t))
    } else {
        (t.x + t.w / 2, t.y)
    };
    (start, end)
}

/// Rules 2 and 3 and the route ends: a segment touches only its own boxes, only at their edges,
/// and two arrows to different targets never run along the same line.
fn routes(g: &Graph, l: &Layout) -> Result<(), String> {
    if l.routes.len() != g.arrows.len() {
        return Err("route count: one route per arrow".into());
    }
    for (i, (a, r)) in g.arrows.iter().zip(&l.routes).enumerate() {
        if (r[0], r[r.len() - 1]) != ends(a, l) {
            return Err(format!("route ends: arrow {i} {a:?} runs {r:?}"));
        }
        for s in segs(r) {
            let ((x0, x1), (y0, y1)) = span(s);
            for (m, b) in l.boxes.iter().enumerate() {
                let own = m == a.from || m == a.to;
                let (x, y) = ((b.x, b.x + b.w), (b.y, b.y + b.h));
                let hit = if own {
                    overlap_1d((x0, x1), x) && overlap_1d((y0, y1), y)
                } else {
                    x0 <= x.1 && x.0 <= x1 && y0 <= y.1 && y.0 <= y1
                };
                if hit {
                    return Err(format!("rule 2: arrow {i} {a:?} meets box {m} at {s:?}"));
                }
            }
        }
    }
    for (i, a) in g.arrows.iter().enumerate() {
        for (j, b) in g
            .arrows
            .iter()
            .enumerate()
            .skip(i + 1)
            .filter(|(_, b)| b.to != a.to)
        {
            for (s, t) in segs(&l.routes[i]).flat_map(|s| segs(&l.routes[j]).map(move |t| (s, t))) {
                let ((sx, sy), (tx, ty)) = (span(s), span(t));
                let along = (sx.0 == sx.1 && tx.0 == tx.1 && sx.0 == tx.0 && overlap_1d(sy, ty))
                    || (sy.0 == sy.1 && ty.0 == ty.1 && sy.0 == ty.0 && overlap_1d(sx, tx));
                if along {
                    return Err(format!(
                        "rule 3: arrows {i} {a:?} and {j} {b:?} share {s:?} / {t:?}"
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Every rule, naming the first one broken. Rule numbers follow the Task 5 brief; the gap rule
/// is R1 and R17.
pub(super) fn invariants(f: &FeatureDecl, g: &Graph, l: &Layout) -> Result<(), String> {
    let boxes = &l.boxes;
    for (n, a) in boxes.iter().enumerate() {
        for (m, b) in boxes.iter().enumerate().skip(n + 1) {
            if overlap_1d((a.x, a.x + a.w), (b.x, b.x + b.w))
                && overlap_1d((a.y, a.y + a.h), (b.y, b.y + b.h))
            {
                return Err(format!("rule 1: boxes {n} and {m} overlap"));
            }
        }
    }
    for (i, r) in l.routes.iter().enumerate() {
        for w in r.windows(2) {
            if w[0] == w[1] || (w[0].0 != w[1].0 && w[0].1 != w[1].1) {
                return Err(format!("rule 4: arrow {i} has a bad segment {w:?}"));
            }
        }
    }
    let want = count(&f.body) + 2;
    if g.nodes.len() != want || boxes.len() != want {
        return Err(format!("rule 5: {} nodes, want {want}", g.nodes.len()));
    }
    let inside = |&(x, y): &(i32, i32)| 0 <= x && x <= l.width && 0 <= y && y <= l.height;
    if !boxes.iter().all(|b| inside(&(b.x + b.w, b.y + b.h)))
        || !l.routes.iter().flatten().all(inside)
    {
        return Err("rule 5: the chart does not fit its width and height".into());
    }
    routes(g, l)?;
    // R1 and R17: a gap is 24 high plus 10 for each track beyond the first, and the tracks are
    // the distinct targets of the arrows that run horizontally in it.
    let mut tracks = BTreeMap::<usize, BTreeSet<usize>>::new();
    for (a, r) in g.arrows.iter().zip(&l.routes) {
        for w in r.windows(2).filter(|w| w[0].1 == w[1].1) {
            let gap = (1..boxes.len())
                .find(|&k| boxes[k - 1].y + boxes[k - 1].h < w[0].1 && w[0].1 < boxes[k].y);
            if let Some(k) = gap {
                tracks.entry(k).or_default().insert(a.to);
            }
        }
    }
    for k in 1..boxes.len() {
        let h = boxes[k].y - (boxes[k - 1].y + boxes[k - 1].h);
        let n = tracks.get(&k).map_or(0, BTreeSet::len);
        if h != 24 + 10 * (n.max(1) as i32 - 1) {
            return Err(format!("gap height: gap {k} is {h}px for {n} tracks"));
        }
    }
    geometry(g, l)
}

/// Lays out every feature of the program and checks it; returns how many it checked.
pub(super) fn program(text: &str) -> usize {
    let (_, checked) = crate::check_program(text);
    let checked = checked.expect("the program checks");
    let src = crate::source::Source::new(text);
    let mut features = 0;
    for f in checked.program().features() {
        let g = super::graph::graph(f, &src);
        let l = layout(&g);
        invariants(f, &g, &l).unwrap_or_else(|e| panic!("{e}\nfeature line {}:\n{text}", f.line));
        features += 1;
    }
    features
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::Severity;

    const TEXT: &str = "feature start\n  show 1\n  show 2\n  if 1 is 1\n    show 3\n  show 4\n";

    fn broken(rule: &str, wreck: impl FnOnce(&mut Graph, &mut Layout)) {
        let (_, checked) = crate::check_program(TEXT);
        let checked = checked.expect("the program checks");
        let f = checked.program().features().next().unwrap();
        let mut g = super::super::graph::graph(f, &crate::source::Source::new(TEXT));
        let mut l = layout(&g);
        invariants(f, &g, &l).expect("the layout starts sound");
        wreck(&mut g, &mut l);
        let e = invariants(f, &g, &l).expect_err("the broken layout passes");
        assert!(e.starts_with(rule), "{e}");
    }

    #[test]
    fn overlapping_boxes_break_rule_1() {
        broken("rule 1", |_, l| l.boxes[1] = l.boxes[0]);
    }

    #[test]
    fn a_diagonal_breaks_rule_4() {
        broken("rule 4", |_, l| l.routes[0][1].0 += 5);
    }

    #[test]
    fn a_zero_length_segment_breaks_rule_4() {
        broken("rule 4", |_, l| {
            let p = l.routes[0][0];
            l.routes[0].insert(1, p);
        });
    }

    #[test]
    fn a_missing_node_or_a_small_chart_breaks_rule_5() {
        broken("rule 5", |g, _| {
            g.nodes.pop();
        });
        broken("rule 5", |_, l| l.width -= 100);
        broken("rule 5", |_, l| l.height -= 100);
        // A route that leaves the chart while every box stays inside it.
        broken("rule 5", |_, l| {
            let p = l.routes[0][0];
            l.routes[0] = vec![(p.0, -5), p];
        });
    }

    #[test]
    fn a_wrong_gap_height_is_caught() {
        broken("gap height", |_, l| {
            let top = l.boxes[1].y;
            for b in &mut l.boxes[1..] {
                b.y += 10;
            }
            for p in l.routes.iter_mut().flatten().filter(|p| p.1 >= top) {
                p.1 += 10;
            }
            l.height += 10;
        });
    }

    #[test]
    fn a_route_through_a_box_breaks_rule_2() {
        // The first arrow drops past boxes 1 and 2 to the middle of box 3, then climbs back.
        broken("rule 2", |_, l| {
            let ((sx, sy), (ex, ey)) = (l.routes[0][0], l.routes[0][1]);
            let deep = mid(&l.boxes[3]);
            l.routes[0] = vec![
                (sx, sy),
                (sx, deep),
                (ex + 1, deep),
                (ex + 1, ey - 1),
                (ex, ey - 1),
                (ex, ey),
            ];
        });
    }

    #[test]
    fn two_targets_on_one_track_break_rule_3() {
        // Arrows 0 and 1 go to different boxes but both run along y = 64 to the right of the boxes.
        broken("rule 3", |_, l| {
            let (s0, e0) = (l.routes[0][0], l.routes[0][1]);
            let (s1, e1) = (l.routes[1][0], l.routes[1][1]);
            l.width = 1000;
            l.routes[0] = vec![s0, (s0.0, 64), (300, 64), (300, 70), (e0.0, 70), e0];
            l.routes[1] = vec![
                s1,
                (s1.0, 124),
                (400, 124),
                (400, 64),
                (200, 64),
                (200, 130),
                (e1.0, 130),
                e1,
            ];
        });
    }

    #[test]
    fn a_route_that_starts_elsewhere_is_caught() {
        broken("route ends", |_, l| l.routes[0][0].1 -= 1);
    }

    #[test]
    fn every_example_and_program_lays_out_cleanly() {
        let (mut files, mut features, mut skipped) = (0, 0, 0);
        let mut found = vec![];
        for dir in ["examples", "tests"] {
            let mut todo = vec![std::path::PathBuf::from(dir)];
            while let Some(d) = todo.pop() {
                for e in std::fs::read_dir(d).unwrap().map(|e| e.unwrap().path()) {
                    if e.is_dir() {
                        todo.push(e);
                    } else if e.extension().is_some_and(|x| x == "gloss") {
                        found.push(e);
                    }
                }
            }
        }
        found.sort();
        let examples = found.iter().filter(|p| p.starts_with("examples")).count();
        // A floor: raise it when examples are added.
        assert!(examples >= 12, "only {examples} example files");
        for p in &found {
            let text = std::fs::read_to_string(p).unwrap();
            let (diags, checked) = crate::check_program(&text);
            if checked.is_none() {
                // A file is skipped only because it has a compile error, never silently.
                assert!(diags.iter().any(|d| d.severity == Severity::Error), "{p:?}");
                skipped += 1;
                continue;
            }
            files += 1;
            features += program(&text);
        }
        eprintln!("{files} files checked, {skipped} skipped, {features} features");
        assert!(files > 0 && features > 0);
    }

    #[test]
    fn notes_do_not_count_and_a_chain_counts_each_condition() {
        let n = |t: &str| {
            let (_, c) = crate::check_program(t);
            count(&c.unwrap().program().features().next().unwrap().body)
        };
        assert_eq!(n("feature start\n  note x\n  show 1\n"), 1);
        assert_eq!(n(TEXT), 5);
    }
}
