pub mod ast;
pub mod chars;
pub mod check;
pub mod cli;
pub mod diag;
pub mod format;
pub mod ir;
#[cfg(test)]
mod irsim;
pub mod lexer;
pub mod lower;
pub mod number;
pub mod parser;
pub mod permissions;
pub mod runtime;
pub mod source;
pub mod target;
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

/// A program that checked without errors, ready to build.
pub struct Checked {
    program: Program,
    src: Source,
    types: check::TypeInfo,
}

impl Checked {
    /// The table files this program can touch, as a person approves them at build time.
    pub fn permissions(&self) -> Vec<permissions::Entry> {
        permissions::list(&self.program)
    }

    /// Lowers and encodes the program as an x86-64 Linux executable.
    pub fn executable(&self) -> Result<Vec<u8>, Diagnostic> {
        let module = lower::lower(&self.program, &self.types, &self.src)?;
        target::x86_64_linux()
            .executable(&module)
            .map_err(Diagnostic::program)
    }
}

/// Parses and checks a program once: the diagnostics, and the program when none is an error.
pub fn check_program(text: &str) -> (Vec<Diagnostic>, Option<Checked>) {
    let (program, src) = match front_end(&Source::new(text)) {
        Ok(parsed) => parsed,
        Err(d) => return (vec![d], None),
    };
    let checked = check::check(&program, &src);
    let clean = !checked.diagnostics.iter().any(Diagnostic::is_error);
    let built = checked.types.filter(|_| clean).map(|types| Checked {
        program,
        src,
        types,
    });
    (checked.diagnostics, built)
}

/// Checks, lowers and encodes a program as an x86-64 Linux executable. A program with an
/// error, or one that cannot be built yet, gives its diagnostics instead.
pub fn build_executable(text: &str) -> Result<Vec<u8>, Vec<Diagnostic>> {
    match check_program(text) {
        (_, Some(checked)) => checked.executable().map_err(|d| vec![d]),
        (diagnostics, None) => Err(diagnostics),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_clean_program_has_no_diagnostics() {
        assert!(super::check_source("feature start\n  show 1\n").is_empty());
    }
}
