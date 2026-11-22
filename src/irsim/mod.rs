//! `irsim`: a test-only interpreter for the Gloss IR. It follows the IR's documented
//! semantics exactly, so runtime routines written in IR can be checked against the Rust
//! `Number` reference and whole programs can run on any host.

mod memory;
