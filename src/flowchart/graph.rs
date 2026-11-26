//! A feature as flowchart nodes labelled with its source lines (D60, D61).

use crate::ast::program::FeatureDecl;
use crate::ast::stmt::{Stmt, StmtKind};
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
    let start = Node {
        shape: Shape::Start,
        label,
        line: feature.line,
    };
    let mut builder = Builder {
        src,
        nodes: vec![start],
        end,
    };
    builder.block(&feature.body);
    let end = builder.body_end(src.line(feature.line));
    builder.nodes.push(Node {
        shape: Shape::End,
        label: vec!["end".to_string()],
        line: end,
    });
    Graph {
        nodes: builder.nodes,
        arrows: vec![],
    }
}

struct Builder<'a> {
    src: &'a Source,
    nodes: Vec<Node>,
    /// The last line of the last node pushed.
    end: u32,
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
                for (i, (cond, body)) in branches.iter().enumerate() {
                    self.push(Shape::Decision, if i == 0 { stmt.line } else { cond.line });
                    self.block(body);
                }
                if let Some(body) = otherwise {
                    self.block(body);
                }
            }
            StmtKind::ForEachIn { body, .. }
            | StmtKind::ForEachRange { body, .. }
            | StmtKind::RepeatWhile { body, .. } => {
                self.push(Shape::Decision, stmt.line);
                self.block(body);
            }
            kind => {
                let mut exprs = Vec::new();
                walk::parts(kind, &mut exprs, &mut Vec::new());
                let calls = matches!(kind, StmtKind::Run { .. }) || exprs.into_iter().any(has_call);
                self.push(if calls { Shape::Call } else { Shape::Step }, stmt.line);
            }
        }
    }

    fn push(&mut self, shape: Shape, line: u32) {
        let label = label_at(self.src, line);
        self.end = line + label.len() as u32 - 1;
        self.nodes.push(Node { shape, label, line });
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
        assert!(g.arrows.is_empty());
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

    #[test]
    fn invisible_characters_are_spelled() {
        let g = graph_of("feature start\n  show \"a\u{200B}b\"\n");
        assert_eq!(g.nodes[1].label, ["show \"a\\u{200B}b\""]);
    }
}
