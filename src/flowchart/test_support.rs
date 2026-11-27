//! Helpers shared by the flowchart unit tests.

use super::graph::{Graph, graph};
use crate::source::Source;

/// An if chain with a join below it: three bodies, the last an `otherwise`.
pub(super) const CHAIN: &str = "feature start\n  set n to 1\n  if n is 1\n    set n to 2\n  otherwise if n is 2\n    set n to 5\n  otherwise\n    set n to 4\n  show n\n";

/// The graph of the program's first feature, after every feature of it has passed the full
/// invariants and the SVGs are well formed.
pub(super) fn graph_of(text: &str) -> Graph {
    super::check::program(text);
    let (_, checked) = crate::check_program(text);
    let checked = checked.expect("the program checks");
    let feature = checked.program().features().next().expect("a feature");
    graph(feature, &Source::new(text), &checked.types)
}
