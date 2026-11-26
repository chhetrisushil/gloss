//! `gloss flowchart`: a feature's statements as a graph, then a layout and SVG text (§4.11).

#[cfg(test)]
mod check;
mod graph;
// Task 7 draws the layout.
#[cfg_attr(not(test), expect(dead_code))]
mod layout;
// Task 7 draws the routes.
#[cfg_attr(not(test), expect(dead_code))]
mod route;
