//! A feature as flowchart nodes labelled with its source lines (D60, D61).

use crate::ast::program::FeatureDecl;
use crate::ast::stmt::StmtKind;
use crate::ast::{Expr, ExprKind, walk};
use crate::lower::spell_invisible;
use crate::source::Source;

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

// Task 2 builds the arrows; no variant but Next is built until then.
#[allow(dead_code)]
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

// Task 2 builds the arrows.
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Arrow {
    pub from: usize,
    pub to: usize,
    pub kind: ArrowKind,
}

impl Arrow {
    // Task 4 routes the back arrows.
    #[cfg_attr(not(test), expect(dead_code))]
    /// Whether the arrow goes back to an earlier (or the same) node, as a loop does.
    pub fn is_back(&self) -> bool {
        self.to <= self.from
    }
}

// Task 2 fills the arrows, and Task 3 lays the graph out.
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Graph {
    pub nodes: Vec<Node>,
    pub arrows: Vec<Arrow>,
}

// Task 7 builds the graph for each feature.
#[cfg_attr(not(test), expect(dead_code))]
pub(crate) fn graph(feature: &FeatureDecl, src: &Source) -> Graph {
    let mut label = label_at(src, feature.line);
    let mut end = feature.line + label.len() as u32 - 1;
    for input in &feature.needs {
        if input.line > end {
            let more = label_at(src, input.line);
            end = input.line + more.len() as u32 - 1;
            label.extend(more);
        }
    }
    let mut nodes = vec![Node {
        shape: Shape::Start,
        label,
        line: feature.line,
    }];
    for stmt in &feature.body {
        let mut exprs = Vec::new();
        walk::parts(&stmt.kind, &mut exprs, &mut Vec::new());
        let shape = match &stmt.kind {
            StmtKind::Note(_) => continue,
            StmtKind::If { .. }
            | StmtKind::ForEachIn { .. }
            | StmtKind::ForEachRange { .. }
            | StmtKind::RepeatWhile { .. } => Shape::Decision,
            StmtKind::Run { .. } => Shape::Call,
            _ if exprs.into_iter().any(has_call) => Shape::Call,
            _ => Shape::Step,
        };
        let label = label_at(src, stmt.line);
        end = stmt.line + label.len() as u32 - 1;
        nodes.push(Node {
            shape,
            label,
            line: stmt.line,
        });
    }
    nodes.push(Node {
        shape: Shape::End,
        label: vec!["end".to_string()],
        line: end,
    });
    Graph {
        nodes,
        arrows: vec![],
    }
}

/// The source lines of the statement written from `line`: each trimmed and with invisible
/// characters spelled, and the next line too while one ends in a comma (F3).
fn label_at(src: &Source, line: u32) -> Vec<String> {
    let mut label = Vec::new();
    let mut n = line;
    loop {
        let text = src.line(n).trim();
        label.push(spell_invisible(text));
        if !text.ends_with(',') || n >= src.line_count() {
            return label;
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
    use Shape::*;

    fn graph_of(text: &str) -> Graph {
        let (_, checked) = crate::check_program(text);
        let checked = checked.expect("the program checks");
        let feature = checked.program().features().next().expect("a feature");
        graph(feature, &Source::new(text))
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
    fn calls_have_their_own_shape() {
        let g = graph_of(
            "feature start\n  set n to 3\n  set xs to list of 1, 2\n  run report\n  set t to total_of with xs\n  show n\n\nfeature report\n  show 1\n\nfeature total_of\n  needs xs as list of number\n  give 1\n",
        );
        assert_eq!(shapes(&g), [Start, Step, Step, Call, Call, Step, End]);
    }

    #[test]
    fn an_arrow_to_an_earlier_or_the_same_node_is_back() {
        for (from, to, back) in [(1, 2, false), (2, 1, true), (2, 2, true)] {
            let kind = ArrowKind::Next;
            assert_eq!(Arrow { from, to, kind }.is_back(), back);
        }
    }

    #[test]
    fn invisible_characters_are_spelled() {
        let g = graph_of("feature start\n  show \"a\u{200B}b\"\n");
        assert_eq!(g.nodes[1].label, ["show \"a\\u{200B}b\""]);
    }
}
