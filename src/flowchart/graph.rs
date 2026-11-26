//! A feature as flowchart nodes labelled with its source lines (D60, D61).

use crate::ast::program::FeatureDecl;
use crate::ast::stmt::{Stmt, StmtKind};
use crate::ast::{Expr, ExprKind, walk};
use crate::lower::spell_invisible;
use crate::source::Source;
use std::ops::Range;

// Task 3 lays out the boxes by shape.
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Start,
    End,
    Step,
    Call,
    Decision,
}

// Task 3 sizes the boxes from the labels.
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Node {
    pub shape: Shape,
    pub label: Vec<String>,
    pub line: u32,
}

/// How an arrow leaves its node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArrowKind {
    Next,
    Yes,
    No,
    Each,
    Done,
    Skip,
    Stop,
    Give,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Arrow {
    pub from: usize,
    pub to: usize,
    pub kind: ArrowKind,
}

impl ArrowKind {
    // Task 4 draws the labels.
    #[cfg_attr(not(test), expect(dead_code))]
    /// The word written on the arrow, if it has one.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Yes => Some("yes"),
            Self::No => Some("no"),
            Self::Each => Some("next"),
            Self::Done => Some("done"),
            _ => None,
        }
    }
}

impl Arrow {
    // Task 4 routes the back arrows.
    #[cfg_attr(not(test), expect(dead_code))]
    /// Whether the arrow goes back to an earlier (or the same) node, as a loop does.
    pub fn is_back(&self) -> bool {
        self.to <= self.from
    }
}

/// A block of the feature, as ranges of node indices, for placing nodes in columns.
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Region {
    /// A loop: its decision and the nodes of its body.
    Loop(Range<usize>),
    /// An if chain: each condition with its body, then the `otherwise` body (empty if none).
    If {
        branches: Vec<Range<usize>>,
        otherwise: Range<usize>,
    },
}

impl Region {
    /// The index of the first node of the block.
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn start(&self) -> usize {
        match self {
            Self::Loop(span) => span.start,
            Self::If { branches, .. } => branches[0].start,
        }
    }
}

// Task 3 lays the graph out.
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Graph {
    pub nodes: Vec<Node>,
    pub arrows: Vec<Arrow>,
    pub regions: Vec<Region>,
}

// Task 7 builds the graph for each feature.
#[cfg_attr(not(test), expect(dead_code))]
pub(crate) fn graph(feature: &FeatureDecl, src: &Source) -> Graph {
    let (mut label, mut end) = label_at(src, feature.line);
    for input in &feature.needs {
        if input.line > end {
            let (more, last) = label_at(src, input.line);
            end = last;
            label.extend(more);
        }
    }
    let start = Node {
        shape: Shape::Start,
        label,
        line: feature.line,
    };
    let mut builder = Builder {
        src,
        nodes: vec![start],
        end,
        arrows: vec![],
        open: vec![(0, ArrowKind::Next)],
        loops: vec![],
        gives: vec![],
        regions: vec![],
    };
    builder.block(&feature.body);
    let line = builder.body_end(src.line(feature.line));
    let end_node = builder.add(Node {
        shape: Shape::End,
        label: vec!["end".to_string()],
        line,
    });
    for from in std::mem::take(&mut builder.gives) {
        builder.arrow(from, end_node, ArrowKind::Give);
    }
    Graph {
        nodes: builder.nodes,
        arrows: builder.arrows,
        regions: builder.regions,
    }
}

/// A loop being built: its decision, and the `stop` exits that leave it.
struct Loop {
    decision: usize,
    stops: Vec<(usize, ArrowKind)>,
}

struct Builder<'a> {
    src: &'a Source,
    nodes: Vec<Node>,
    /// The last line of the last node pushed.
    end: u32,
    arrows: Vec<Arrow>,
    /// The arrows that still need a target: the next node pushed takes them.
    open: Vec<(usize, ArrowKind)>,
    loops: Vec<Loop>,
    /// The `give` nodes, which get an arrow to the end node.
    gives: Vec<usize>,
    regions: Vec<Region>,
}

impl Builder<'_> {
    fn block(&mut self, block: &[Stmt]) {
        for stmt in block {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Note(_) => {}
            StmtKind::If {
                branches,
                otherwise,
            } => {
                let mut out = Vec::new();
                let mut ranges = Vec::new();
                for (i, (cond, body)) in branches.iter().enumerate() {
                    let first = self.nodes.len();
                    let c = self.push(Shape::Decision, if i == 0 { stmt.line } else { cond.line });
                    self.open = vec![(c, ArrowKind::Yes)];
                    self.block(body);
                    out.append(&mut self.open);
                    self.open = vec![(c, ArrowKind::No)];
                    ranges.push(first..self.nodes.len());
                }
                let first = self.nodes.len();
                if let Some(body) = otherwise {
                    self.block(body);
                }
                self.regions.push(Region::If {
                    branches: ranges,
                    otherwise: first..self.nodes.len(),
                });
                out.append(&mut self.open);
                self.open = out;
            }
            StmtKind::ForEachIn { body, .. }
            | StmtKind::ForEachRange { body, .. }
            | StmtKind::RepeatWhile { body, .. } => {
                let d = self.push(Shape::Decision, stmt.line);
                let repeat = matches!(stmt.kind, StmtKind::RepeatWhile { .. });
                let (enter, leave) = if repeat {
                    (ArrowKind::Yes, ArrowKind::No)
                } else {
                    (ArrowKind::Each, ArrowKind::Done)
                };
                self.open = vec![(d, enter)];
                self.loops.push(Loop {
                    decision: d,
                    stops: vec![],
                });
                self.block(body);
                // An empty body gives the arrow (d, d, Each): a back arrow that Task 4 routes.
                for (from, kind) in std::mem::take(&mut self.open) {
                    self.arrow(from, d, kind);
                }
                self.regions.push(Region::Loop(d..self.nodes.len()));
                let lp = self.loops.pop().expect("the loop pushed above");
                self.open = vec![(d, leave)];
                self.open.extend(lp.stops);
            }
            kind => {
                let mut exprs = Vec::new();
                walk::parts(kind, &mut exprs, &mut Vec::new());
                let calls = matches!(kind, StmtKind::Run { .. }) || exprs.into_iter().any(has_call);
                let n = self.push(if calls { Shape::Call } else { Shape::Step }, stmt.line);
                match kind {
                    StmtKind::Skip => {
                        self.open.clear();
                        // The checker rejects a skip outside a loop, so one always encloses it.
                        let lp = self.loops.last();
                        debug_assert!(lp.is_some(), "skip outside a loop");
                        if let Some(lp) = lp {
                            self.arrow(n, lp.decision, ArrowKind::Skip);
                        }
                    }
                    StmtKind::Stop => {
                        self.open.clear();
                        // The checker rejects a stop outside a loop, so one always encloses it.
                        let lp = self.loops.last_mut();
                        debug_assert!(lp.is_some(), "stop outside a loop");
                        if let Some(lp) = lp {
                            lp.stops.push((n, ArrowKind::Stop));
                        }
                    }
                    StmtKind::Give(_) => {
                        self.open.clear();
                        self.gives.push(n);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Adds a node for the statement at `line`, takes the open exits, and returns its index.
    fn push(&mut self, shape: Shape, line: u32) -> usize {
        let (label, last) = label_at(self.src, line);
        self.end = last;
        self.add(Node { shape, label, line })
    }

    fn add(&mut self, node: Node) -> usize {
        let n = self.nodes.len();
        self.nodes.push(node);
        for (from, kind) in std::mem::take(&mut self.open) {
            self.arrow(from, n, kind);
        }
        self.open = vec![(n, ArrowKind::Next)];
        n
    }

    fn arrow(&mut self, from: usize, to: usize, kind: ArrowKind) {
        self.arrows.push(Arrow { from, to, kind });
    }

    /// The last line of the feature's body: the last non-blank line before the next line
    /// written at the feature's own level, or the end of the file.
    fn body_end(&self, feature_line: &str) -> u32 {
        let base = indent(feature_line);
        let mut end = self.end;
        for n in self.end + 1..=self.src.line_count() {
            let text = self.src.line(n);
            if text.trim().is_empty() {
                continue;
            }
            if indent(text) <= base {
                break;
            }
            end = n;
        }
        end
    }
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The source lines of the statement written from `line`, and the number of its last line:
/// each trimmed and with invisible characters spelled, and the next line too while one ends
/// in a comma (F3). Blank lines are ignored (spec §3), so they are not part of a label.
fn label_at(src: &Source, line: u32) -> (Vec<String>, u32) {
    let mut label = Vec::new();
    let mut n = line;
    loop {
        let text = src.line(n).trim();
        if !text.is_empty() {
            label.push(spell_invisible(text));
        }
        if !(text.is_empty() || text.ends_with(',')) || n >= src.line_count() {
            return (label, n);
        }
        n += 1;
    }
}

fn has_call(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Call { .. }) || walk::children(e).into_iter().any(has_call)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ArrowKind::*;
    use Shape::*;

    fn graph_of(text: &str) -> Graph {
        super::super::check::program(text);
        let (_, checked) = crate::check_program(text);
        let checked = checked.expect("the program checks");
        let feature = checked.program().features().next().expect("a feature");
        graph(feature, &Source::new(text))
    }

    fn firsts(g: &Graph) -> Vec<&str> {
        g.nodes.iter().map(|n| n.label[0].as_str()).collect()
    }

    fn shapes(g: &Graph) -> Vec<Shape> {
        g.nodes.iter().map(|n| n.shape).collect()
    }

    #[test]
    fn notes_are_not_nodes() {
        let g = graph_of(
            "feature start\n  note one line\n  set n to 1\n  note\n    a block\n    of words\n  show n\n",
        );
        assert_eq!(shapes(&g), [Start, Step, Step, End]);
        assert_eq!(g.nodes[3].line, 7);
    }

    #[test]
    fn the_start_box_holds_the_needs_lines() {
        let g = graph_of(
            "feature total_of\n  needs prices as list of number\n  give 1\n\nfeature start\n  show 1\n",
        );
        assert_eq!(
            g.nodes[0].label,
            ["feature total_of", "needs prices as list of number"]
        );
    }

    #[test]
    fn nodes_are_in_source_order() {
        let g = graph_of(
            "feature start\n  set n to 3\n  if n is 1\n    set n to 2\n  otherwise if n is 2\n    set n to 5\n  otherwise\n    set n to 4\n  for each i from 1 to n\n    show i\n  show n\n",
        );
        assert_eq!(
            shapes(&g),
            [
                Start, Step, Decision, Step, Decision, Step, Step, Decision, Step, Step, End
            ]
        );
        assert_eq!(
            firsts(&g),
            [
                "feature start",
                "set n to 3",
                "if n is 1",
                "set n to 2",
                "otherwise if n is 2",
                "set n to 5",
                "set n to 4",
                "for each i from 1 to n",
                "show i",
                "show n",
                "end"
            ]
        );
        assert_eq!(g.nodes[1].line, 2);
        assert_eq!(g.nodes[4].line, 5);
        assert_eq!(g.nodes[10].line, 11);
    }

    #[test]
    fn a_continued_line_is_one_node() {
        let g = graph_of("feature start\n  show 1,\n    2\n");
        assert_eq!(shapes(&g), [Start, Step, End]);
        assert_eq!(g.nodes[1].label, ["show 1,", "2"]);
        assert_eq!(g.nodes[2].line, 3);
        // A header with no trailing comma is not continued by its deeper body line.
        let g = graph_of("feature start\n  if 1 is 1\n    show 1\n");
        assert_eq!(g.nodes[1].label, ["if 1 is 1"]);
    }

    fn end_line(text: &str) -> u32 {
        graph_of(text).nodes.last().expect("an end node").line
    }

    #[test]
    fn the_end_line_takes_in_trailing_notes() {
        assert_eq!(end_line("feature start\n  show 1\n  note tail\n"), 3);
        let nested = "feature start\n  if 1 is 1\n    show 1\n    note n\n";
        assert_eq!(end_line(nested), 4);
        let block = "feature start\n  show 1\n  note\n    a\n\n    b\n";
        assert_eq!(end_line(block), 6);
        let nested_block = "feature start\n  if 1 is 1\n    show 1\n    note\n      a\n\n      b\n";
        assert_eq!(end_line(nested_block), 7);
        let two = "feature start\n  show 1\n  note t\n\nfeature other\n  show 2\n";
        assert_eq!(end_line(two), 3);
    }

    #[test]
    fn needs_lines_are_listed_once_each() {
        let label = |text: &str| {
            graph_of(&format!("{text}\nfeature start\n  show 1\n")).nodes[0]
                .label
                .clone()
        };
        let one_line = "feature f\n  needs x as number, y as number\n  show x, y\n";
        assert_eq!(
            label(one_line),
            ["feature f", "needs x as number, y as number"]
        );
        let continued = "feature f\n  needs x as number,\n    y as number\n  show x, y\n";
        assert_eq!(
            label(continued),
            ["feature f", "needs x as number,", "y as number"]
        );
        let noted = "feature f\n  note a\n  needs x as number\n  show x\n";
        assert_eq!(label(noted), ["feature f", "needs x as number"]);
        // A blank line after a trailing comma is ignored, in a needs line and in a statement.
        let gap = "feature f\n  needs x as number,\n\n    y as number\n  show x,\n\n    y\n";
        let g = graph_of(&format!("{gap}\nfeature start\n  show 1\n"));
        assert_eq!(
            g.nodes[0].label,
            ["feature f", "needs x as number,", "y as number"]
        );
        assert_eq!(g.nodes[1].label, ["show x,", "y"]);
        assert_eq!(g.nodes[2].line, 7);
    }

    #[test]
    fn every_loop_form_is_a_decision_and_blocks_nest() {
        let g = graph_of(
            "feature start\n  set xs to list of 1, 2\n  for each x in xs\n    show x\n  for each i from 1 to 3\n    show i\n  set n to 0\n  repeat while n is less than 3\n    add 1 to n\n",
        );
        assert_eq!(
            shapes(&g),
            [
                Start, Step, Decision, Step, Decision, Step, Step, Decision, Step, End
            ]
        );
        assert_eq!(
            firsts(&g),
            [
                "feature start",
                "set xs to list of 1, 2",
                "for each x in xs",
                "show x",
                "for each i from 1 to 3",
                "show i",
                "set n to 0",
                "repeat while n is less than 3",
                "add 1 to n",
                "end"
            ]
        );
        let deep = graph_of(
            "feature start\n  set n to 0\n  for each i from 1 to 3\n    if i is 2\n      repeat while n is less than 2\n        add 1 to n\n",
        );
        assert_eq!(
            shapes(&deep),
            [Start, Step, Decision, Decision, Decision, Step, End]
        );
        assert_eq!(
            firsts(&deep)[2..6],
            [
                "for each i from 1 to 3",
                "if i is 2",
                "repeat while n is less than 2",
                "add 1 to n"
            ]
        );
    }

    #[test]
    fn a_call_inside_an_expression_makes_the_statement_a_call() {
        let g = graph_of(
            "feature start\n  set xs to list of 1, 2\n  show 1 + (total_of with xs)\n\nfeature total_of\n  needs xs as list of number\n  give 1\n",
        );
        assert_eq!(shapes(&g), [Start, Step, Call, End]);
    }

    #[test]
    fn calls_have_their_own_shape() {
        let g = graph_of(
            "feature start\n  set n to 3\n  set xs to list of 1, 2\n  run report\n  set t to total_of with xs\n  if is_big with n\n    show n\n\nfeature report\n  show 1\n\nfeature is_big\n  needs n as number\n  give n is greater than 5\n\nfeature total_of\n  needs xs as list of number\n  give 1\n",
        );
        assert_eq!(
            shapes(&g),
            [Start, Step, Step, Call, Call, Decision, Step, End]
        );
    }

    #[test]
    fn an_arrow_to_an_earlier_or_the_same_node_is_back() {
        for (from, to, back) in [(1, 2, false), (2, 1, true), (2, 2, true)] {
            let kind = ArrowKind::Next;
            assert_eq!(Arrow { from, to, kind }.is_back(), back);
        }
    }

    fn arrows_of(text: &str) -> Vec<(usize, usize, ArrowKind)> {
        let g = graph_of(text);
        g.arrows.iter().map(|a| (a.from, a.to, a.kind)).collect()
    }

    #[test]
    fn straight_line_arrows() {
        assert_eq!(
            arrows_of("feature start\n  set n to 1\n  show n\n"),
            [(0, 1, Next), (1, 2, Next), (2, 3, Next)]
        );
    }

    #[test]
    fn an_if_chain_joins_below() {
        let text = "feature start\n  set n to 1\n  if n is 1\n    set n to 2\n  otherwise if n is 2\n    set n to 5\n  otherwise\n    set n to 4\n  show n\n";
        assert_eq!(
            arrows_of(text),
            [
                (0, 1, Next),
                (1, 2, Next),
                (2, 3, Yes),
                (2, 4, No),
                (4, 5, Yes),
                (4, 6, No),
                (3, 7, Next),
                (5, 7, Next),
                (6, 7, Next),
                (7, 8, Next)
            ]
        );
    }

    #[test]
    fn an_if_without_otherwise_falls_through_on_no() {
        let text = "feature start\n  if 1 is 1\n    show 1\n  show 2\n";
        assert_eq!(
            arrows_of(text),
            [
                (0, 1, Next),
                (1, 2, Yes),
                (2, 3, Next),
                (1, 3, No),
                (3, 4, Next)
            ]
        );
    }

    #[test]
    fn a_loop_goes_back_and_out() {
        let each = "feature start\n  for each i from 1 to 3\n    show i\n    show i\n  show 0\n";
        assert_eq!(
            arrows_of(each),
            [
                (0, 1, Next),
                (1, 2, Each),
                (2, 3, Next),
                (3, 1, Next),
                (1, 4, Done),
                (4, 5, Next)
            ]
        );
        assert!(graph_of(each).arrows[3].is_back());
        let repeat = "feature start\n  set n to 0\n  repeat while n is less than 3\n    add 1 to n\n    add 1 to n\n  show n\n";
        assert_eq!(
            arrows_of(repeat),
            [
                (0, 1, Next),
                (1, 2, Next),
                (2, 3, Yes),
                (3, 4, Next),
                (4, 2, Next),
                (2, 5, No),
                (5, 6, Next)
            ]
        );
        // An if left open at the end of a body goes back on its No.
        let open = "feature start\n  for each i from 1 to 3\n    if i is 1\n      show i\n";
        assert_eq!(
            arrows_of(open),
            [
                (0, 1, Next),
                (1, 2, Each),
                (2, 3, Yes),
                (3, 1, Next),
                (2, 1, No),
                (1, 4, Done)
            ]
        );
    }

    #[test]
    fn jumps() {
        let text = "feature f\n  for each i from 1 to 5\n    if i is 1\n      skip\n    if i is 2\n      stop\n    show i\n  show 9\n  give 1\n  show 8\n\nfeature start\n  show f\n";
        let arrows = arrows_of(text);
        assert_eq!(
            arrows,
            [
                (0, 1, Next),
                (1, 2, Each),
                (2, 3, Yes),
                (3, 1, Skip),
                (2, 4, No),
                (4, 5, Yes),
                (4, 6, No),
                (6, 1, Next),
                (1, 7, Done),
                (5, 7, Stop),
                (7, 8, Next),
                (9, 10, Next),
                (8, 10, Give)
            ]
        );
        assert!(arrows.iter().all(|&(_, to, _)| to != 9));
    }

    #[test]
    fn nested_loops() {
        let text = "feature start\n  for each i from 1 to 3\n    for each j from 1 to 3\n      if j is 2\n        stop\n      show j\n    show i\n  show 0\n";
        assert_eq!(
            arrows_of(text),
            [
                (0, 1, Next),
                (1, 2, Each),
                (2, 3, Each),
                (3, 4, Yes),
                (3, 5, No),
                (5, 2, Next),
                (2, 6, Done),
                (4, 6, Stop),
                (6, 1, Next),
                (1, 7, Done),
                (7, 8, Next)
            ]
        );
    }

    #[test]
    fn a_skip_goes_to_the_inner_loop() {
        let text = "feature start\n  for each i from 1 to 3\n    for each j from 1 to 3\n      if j is 2\n        skip\n      show j\n    show i\n";
        assert_eq!(
            arrows_of(text),
            [
                (0, 1, Next),
                (1, 2, Each),
                (2, 3, Each),
                (3, 4, Yes),
                (4, 2, Skip),
                (3, 5, No),
                (5, 2, Next),
                (2, 6, Done),
                (6, 1, Next),
                (1, 7, Done)
            ]
        );
    }

    #[test]
    fn a_stop_ends_a_body_with_no_back_arrow() {
        let text = "feature start\n  for each i from 1 to 3\n    show i\n    stop\n  show 0\n";
        assert_eq!(
            arrows_of(text),
            [
                (0, 1, Next),
                (1, 2, Each),
                (2, 3, Next),
                (1, 4, Done),
                (3, 4, Stop),
                (4, 5, Next)
            ]
        );
    }

    #[test]
    fn give_in_a_loop_and_an_if_goes_to_the_end() {
        let text = "feature f\n  for each i from 1 to 3\n    if i is 2\n      give i\n    show i\n  give 0\n\nfeature start\n  show f\n";
        assert_eq!(
            arrows_of(text),
            [
                (0, 1, Next),
                (1, 2, Each),
                (2, 3, Yes),
                (2, 4, No),
                (4, 1, Next),
                (1, 5, Done),
                (3, 6, Give),
                (5, 6, Give)
            ]
        );
    }

    #[test]
    fn empty_bodies_leave_their_exits_open() {
        let text = "feature start\n  if 1 is 1\n    note nothing\n  show 1\n";
        assert_eq!(
            arrows_of(text),
            [(0, 1, Next), (1, 2, Yes), (1, 2, No), (2, 3, Next)]
        );
        // The empty loop body makes a self arrow, a back arrow.
        let text = "feature start\n  for each i from 1 to 3\n    note nothing\n  show 1\n";
        assert_eq!(
            arrows_of(text),
            [(0, 1, Next), (1, 1, Each), (1, 2, Done), (2, 3, Next)]
        );
    }

    #[test]
    fn an_if_whose_branches_all_jump_has_no_way_into_the_next_node() {
        let text = "feature start\n  for each i from 1 to 3\n    if i is 1\n      skip\n    otherwise\n      stop\n    show i\n  show 0\n";
        let arrows = arrows_of(text);
        assert_eq!(
            arrows,
            [
                (0, 1, Next),
                (1, 2, Each),
                (2, 3, Yes),
                (3, 1, Skip),
                (2, 4, No),
                (5, 1, Next),
                (1, 6, Done),
                (4, 6, Stop),
                (6, 7, Next)
            ]
        );
        assert!(arrows.iter().all(|&(_, to, _)| to != 5));
    }

    #[test]
    fn arrows_are_labelled_by_kind() {
        use ArrowKind as K;
        let labels = [
            K::Next,
            K::Yes,
            K::No,
            K::Each,
            K::Done,
            K::Skip,
            K::Stop,
            K::Give,
        ]
        .map(K::label);
        assert_eq!(
            labels,
            [
                None,
                Some("yes"),
                Some("no"),
                Some("next"),
                Some("done"),
                None,
                None,
                None
            ]
        );
    }

    #[test]
    fn invisible_characters_are_spelled() {
        let g = graph_of("feature start\n  show \"a\u{200B}b\"\n");
        assert_eq!(g.nodes[1].label, ["show \"a\\u{200B}b\""]);
    }
}
