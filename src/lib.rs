pub mod ast;
pub mod check;
pub mod diag;
pub mod lexer;
pub mod number;
pub mod parser;
pub mod source;

use ast::program::Program;
use diag::Diagnostic;
use source::Source;

/// Lexes and parses a whole program.
pub fn parse_source(src: &Source) -> Result<Program, Diagnostic> {
    parser::parse(&lexer::lex(src)?, src)
}

/// Parses and checks a program; returns the parse error alone if parsing
/// fails, else the sorted check diagnostics.
pub fn check_source(text: &str) -> Vec<Diagnostic> {
    let src = Source::new(text);
    match parse_source(&src) {
        Ok(program) => check::check(&program, &src).diagnostics,
        Err(d) => vec![d],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_clean_program_has_no_diagnostics() {
        assert!(super::check_source("feature start\n  show 1\n").is_empty());
    }
}
