pub mod ast;
pub mod check;
pub mod cli;
pub mod diag;
pub mod format;
pub mod lexer;
pub mod number;
pub mod parser;
pub mod source;
#[cfg(test)]
mod testing;

use ast::program::Program;
use diag::Diagnostic;
use source::Source;

/// Lexes and parses a whole program. The source returned with it reads a statement written
/// over several lines as one line, which is how diagnostics echo it.
fn front_end(src: &Source) -> Result<(Program, Source), Diagnostic> {
    let toks = lexer::lex(src)?;
    let src = src.with_statements(&lexer::statement_spans(&toks));
    Ok((parser::parse(&toks, &src)?, src))
}

/// Lexes and parses a whole program.
pub fn parse_source(src: &Source) -> Result<Program, Diagnostic> {
    front_end(src).map(|(program, _)| program)
}

/// Rewrites a program in the canonical layout.
pub fn format_source(text: &str) -> Result<String, Diagnostic> {
    let program = parse_source(&Source::new(text))?;
    format::verified(&program, format::format_program(&program))
}

/// Parses and checks a program; returns the parse error alone if parsing
/// fails, else the sorted check diagnostics.
pub fn check_source(text: &str) -> Vec<Diagnostic> {
    match front_end(&Source::new(text)) {
        Ok((program, src)) => check::check(&program, &src).diagnostics,
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
