use crate::source::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub line: Option<u32>,
    pub source_line: String,
    pub problem: String,
    pub suggestion: Option<String>,
}

impl Diagnostic {
    fn at(severity: Severity, src: &Source, line: u32, problem: impl Into<String>) -> Self {
        Diagnostic {
            severity,
            line: Some(line),
            source_line: src.line(line).trim().to_string(),
            problem: problem.into(),
            suggestion: None,
        }
    }

    pub fn error(src: &Source, line: u32, problem: impl Into<String>) -> Self {
        Self::at(Severity::Error, src, line, problem)
    }

    pub fn warning(src: &Source, line: u32, problem: impl Into<String>) -> Self {
        Self::at(Severity::Warning, src, line, problem)
    }

    pub fn program(problem: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            line: None,
            source_line: String::new(),
            problem: problem.into(),
            suggestion: None,
        }
    }

    pub fn suggest(mut self, form: impl Into<String>) -> Self {
        self.suggestion = Some(form.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    pub fn render(&self) -> String {
        // A problem that already ends in a question mark takes no full stop.
        let stop = if self.problem.ends_with('?') { "" } else { "." };
        let mut out = match self.line {
            Some(n) => {
                let tag = if self.severity == Severity::Warning {
                    "warning: "
                } else {
                    ""
                };
                format!(
                    "line {n}: {tag}{}. {}{stop}",
                    self.source_line, self.problem
                )
            }
            None => format!("{}{stop}", self.problem),
        };
        if let Some(s) = &self.suggestion {
            out.push_str(&format!(" Did you mean {s}?"));
        }
        out
    }
}

/// Program-level first, then by line, errors before warnings on the same line.
pub fn sort(diags: &mut [Diagnostic]) {
    diags.sort_by_key(|d| (d.line.map_or(0, |n| n as u64 + 1), d.severity));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Source;

    #[test]
    fn renders_spec_example_exactly() {
        let src = Source::new("feature start\n  x\n  set total as 0\n");
        let d = Diagnostic::error(&src, 3, "set needs to").suggest("set total to 0");
        assert_eq!(
            d.render(),
            "line 3: set total as 0. set needs to. Did you mean set total to 0?"
        );
    }

    #[test]
    fn renders_warning_and_program_level() {
        let src = Source::new("a\nb\nc\n  set totl to 5\n");
        let w = Diagnostic::warning(&src, 4, "totl is set but never used");
        assert_eq!(
            w.render(),
            "line 4: warning: set totl to 5. totl is set but never used."
        );
        let p =
            Diagnostic::program("This program has no feature start, so it has nowhere to begin");
        assert_eq!(
            p.render(),
            "This program has no feature start, so it has nowhere to begin."
        );
    }

    #[test]
    fn sorts_program_first_then_line_then_errors_first() {
        let src = Source::new("a\nb\nc\n");
        let mut v = vec![
            Diagnostic::warning(&src, 2, "w2"),
            Diagnostic::error(&src, 3, "e3"),
            Diagnostic::error(&src, 2, "e2"),
            Diagnostic::program("p"),
        ];
        sort(&mut v);
        let order: Vec<&str> = v.iter().map(|d| d.problem.as_str()).collect();
        assert_eq!(order, ["p", "e2", "w2", "e3"]);
    }

    #[test]
    fn a_question_takes_no_full_stop() {
        let src = Source::new("a\n  set totl to 5\n");
        let w = Diagnostic::warning(&src, 2, "new name totl; did you mean total?");
        assert_eq!(
            w.render(),
            "line 2: warning: set totl to 5. new name totl; did you mean total?"
        );
    }
}
