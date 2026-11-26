//! Test-only checks of a finished layout against the geometry rules (D62, D63).

use super::graph::{Graph, Region};
use super::layout::{Layout, MARGIN, Rect};

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
