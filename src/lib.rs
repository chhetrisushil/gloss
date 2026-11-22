pub mod ast;
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
