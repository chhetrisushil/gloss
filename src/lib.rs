pub mod approve;
pub mod ast;
pub mod chars;
pub mod check;
pub mod cli;
pub mod diag;
pub(crate) mod explain;
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
pub mod testrun;
pub mod trace;

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
    /// The checked program, for the narrator.
    pub fn program(&self) -> &Program {
        &self.program
    }

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

    /// Lowers and encodes the test executable `gloss test` runs: the features and a driver
    /// that runs each test in a child process of its own.
    pub fn test_executable(&self) -> Result<Vec<u8>, Diagnostic> {
        let module = self.test_module()?;
        target::x86_64_linux()
            .executable(&module)
            .map_err(Diagnostic::program)
    }

    /// The program `test_executable` encodes, before encoding.
    pub fn test_module(&self) -> Result<ir::Module, Diagnostic> {
        lower::lower_tests(&self.program, &self.types, &self.src)
    }

    /// Lowers and encodes the executable `gloss trace` runs.
    pub fn trace_executable(&self) -> Result<Vec<u8>, Diagnostic> {
        let module = self.trace_module()?;
        target::x86_64_linux()
            .executable(&module)
            .map_err(Diagnostic::program)
    }

    /// The program `trace_executable` encodes, before encoding.
    pub fn trace_module(&self) -> Result<ir::Module, Diagnostic> {
        lower::lower_trace(&self.program, &self.types, &self.src)
    }

    /// The listing `gloss explain` prints for the program, named `name`.
    pub fn explanation(&self, name: &str) -> Result<String, Diagnostic> {
        let (module, notes) = lower::lower_explained(&self.program, &self.types, &self.src)?;
        let routines = target::x86_64_linux()
            .listing(&module)
            .map_err(Diagnostic::program)?;
        Ok(explain::listing(
            name,
            &self.src,
            &self.types,
            &notes,
            &routines,
            &module.texts,
        ))
    }

    /// How many tests the program has.
    pub fn test_count(&self) -> usize {
        self.program.tests().count()
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

/// `build_executable` for the trace executable: checks, lowers and encodes the program for
/// `gloss trace`. A program with an error gives its diagnostics instead.
pub fn build_trace_executable(text: &str) -> Result<Vec<u8>, Vec<Diagnostic>> {
    match check_program(text) {
        (_, Some(checked)) => checked.trace_executable().map_err(|d| vec![d]),
        (diagnostics, None) => Err(diagnostics),
    }
}

/// `build_executable` for the test executable: checks, lowers and encodes the features and a
/// driver that runs each test. A program with an error, or one that cannot be built yet, gives
/// its diagnostics instead.
pub fn build_test_executable(text: &str) -> Result<Vec<u8>, Vec<Diagnostic>> {
    match check_program(text) {
        (_, Some(checked)) => checked.test_executable().map_err(|d| vec![d]),
        (diagnostics, None) => Err(diagnostics),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_clean_program_has_no_diagnostics() {
        assert!(super::check_source("feature start\n  show 1\n").is_empty());
    }

    const TWO: &str =
        "feature start\n  show 1\n\ntest \"a\"\n  set y to 1\n\ntest \"b\"\n  set y to 2\n";

    #[test]
    fn build_trace_executable_gives_bytes_that_are_not_the_real_ones() {
        let bytes = super::build_trace_executable(TWO).unwrap();
        assert_eq!(&bytes[..4], b"\x7fELF");
        assert_ne!(bytes, super::build_executable(TWO).unwrap());
        assert!(super::build_trace_executable("feature start\n  show nope\n").is_err());
    }

    #[test]
    fn build_test_executable_gives_bytes_for_a_program_with_tests() {
        let bytes = super::build_test_executable(TWO).unwrap();
        assert_eq!(&bytes[..4], b"\x7fELF");
        // It is not the program's own executable, which has no driver.
        assert_ne!(bytes, super::build_executable(TWO).unwrap());
        let (_, checked) = super::check_program(TWO);
        assert_eq!(checked.unwrap().test_count(), 2);
        let (_, none) = super::check_program("feature start\n  show 1\n");
        assert_eq!(none.unwrap().test_count(), 0);
    }

    #[test]
    fn build_test_executable_gives_the_usual_errors() {
        let bad = "feature start\n  show x\n\ntest \"t\"\n  set y to 1\n";
        let errors = super::build_test_executable(bad).unwrap_err();
        assert!(errors.iter().any(|d| d.is_error()));
        assert_eq!(errors, super::build_executable(bad).unwrap_err());
        // A program that loads and saves builds: a test build has pretend files.
        let files = "record Row\n  n as number\n\ntable rows of Row in \"rows.csv\"\n\n\
                     feature start\n  load rows\n  save rows\n\ntest \"t\"\n  show 1\n";
        assert!(super::build_test_executable(files).is_ok());
    }
}
