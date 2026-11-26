//! `gloss flowchart`: a feature's statements as a graph, then a layout and SVG text (§4.11).

mod graph;
// Task 7 draws the layout.
#[cfg_attr(not(test), expect(dead_code))]
mod layout;
