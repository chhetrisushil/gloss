//! `gloss flowchart`: a feature's statements as a graph, then a layout and SVG text (§4.11).

#[cfg(test)]
mod check;
mod graph;
mod layout;
mod route;
mod svg;

use crate::ast::program::Program;
use crate::source::Source;

/// One `(file name, svg)` pair per feature, in source order; the stem is `name` without `.gloss`.
// Task 7 calls this from the CLI.
#[cfg_attr(not(test), expect(dead_code))]
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn charts(program: &Program, src: &Source, name: &str) -> Vec<(String, String)> {
    let stem = name.strip_suffix(".gloss").unwrap_or(name);
    program
        .features()
        .map(|f| {
            let (g, title) = (graph::graph(f, src), format!("{name}: feature {}", f.name));
            let svg = svg::svg(&title, &g, &layout::layout(&g));
            (format!("{stem}.{}.svg", f.name), svg)
        })
        .collect()
}
