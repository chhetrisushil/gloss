//! Test-only checks that a finished layout is free of overlapping boxes and lines (D62, D63, D65).

use super::graph::{Graph, Region};
use super::layout::{Layout, MARGIN, Rect};
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

#[cfg(test)]
mod tests {
    use super::super::layout::layout;
    use super::*;

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
            for b in &mut l.boxes[1..] {
                b.y += 10;
            }
            l.height += 10;
        });
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
