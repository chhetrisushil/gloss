//! The `gloss` command line.

use std::io::Write;
use std::path::Path;

use crate::diag::Diagnostic;
use crate::source::Source;
use crate::{check_source, format_source, parse_source};

const USAGE: &str = "usage: gloss <command> <file.gloss>\ncommands: build, test, format, trace, explain, flowchart\n";

/// Runs one command; returns the exit status (0 success, 3 anything else).
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (Some(command), Some(file), None) = (args.first(), args.get(1), args.get(2)) else {
        let _ = err.write_all(USAGE.as_bytes());
        return 3;
    };
    let milestone = match command.as_str() {
        "build" | "format" => 0,
        "test" => 4,
        "trace" => 5,
        "explain" => 6,
        "flowchart" => 7,
        _ => {
            let _ = err.write_all(USAGE.as_bytes());
            return 3;
        }
    };
    if milestone != 0 {
        let _ = writeln!(
            err,
            "gloss {command} is not built yet; it arrives in milestone {milestone}"
        );
        return 3;
    }
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) => {
            let _ = writeln!(err, "cannot read {file}: {e}");
            return 3;
        }
    };
    let name = Path::new(file)
        .file_name()
        .map_or(file.clone(), |n| n.to_string_lossy().into());
    if command == "build" {
        build(&name, &text, out, err)
    } else {
        format(file, &name, &text, out, err)
    }
}

fn build(name: &str, text: &str, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut diagnostics = check_source(text);
    diagnostics.extend(executable_clashes(name, text));
    crate::diag::sort(&mut diagnostics);
    for d in &diagnostics {
        let _ = writeln!(err, "{}", d.render());
    }
    if diagnostics.iter().any(|d| d.is_error()) {
        return 3;
    }
    let _ = writeln!(
        out,
        "Checked {name}: no errors (code generation arrives in milestone 2)"
    );
    0
}

/// A table file named like the program's executable (the source name without `.gloss`)
/// would be overwritten by the build, so it is refused on the table's line.
fn executable_clashes(name: &str, text: &str) -> Vec<Diagnostic> {
    let source = Source::new(text);
    let Ok(program) = parse_source(&source) else {
        return Vec::new();
    };
    let executable = name.strip_suffix(".gloss").unwrap_or(name);
    let clashing = program.tables().filter(|t| t.path == executable);
    clashing
        .map(|t| {
            let msg = format!(
                "{} is the name of this program's executable; choose another file name",
                t.path
            );
            Diagnostic::error(&source, t.line, msg)
        })
        .collect()
}

fn format(path: &str, name: &str, text: &str, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let formatted = match format_source(text) {
        Ok(f) => f,
        Err(d) => {
            let _ = writeln!(err, "{}", d.render());
            return 3;
        }
    };
    if formatted == text {
        let _ = writeln!(out, "{name} is already formatted");
    } else if let Err(e) = write_atomically(path, &formatted) {
        let _ = writeln!(err, "cannot write {path}: {e}");
        return 3;
    } else {
        let _ = writeln!(out, "Formatted {name}");
    }
    0
}

/// Replaces the file at `path` with `text` by writing a short-named temporary file beside it
/// and renaming it over the original, so an interrupted write cannot truncate the user's
/// source. A symlink is followed (the real file is replaced, the link stays a link), a
/// read-only file is refused, and the original's permissions are kept.
fn write_atomically(path: &str, text: &str) -> std::io::Result<()> {
    let real = std::fs::canonicalize(path)?;
    let permissions = std::fs::metadata(&real)?.permissions();
    if permissions.readonly() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the file is read-only",
        ));
    }
    let temp = real.with_file_name(format!(".gloss-format-{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        file.set_permissions(permissions)?;
        file.sync_all()?;
        std::fs::rename(&temp, &real)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
