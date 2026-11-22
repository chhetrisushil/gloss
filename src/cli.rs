//! The `gloss` command line.

use std::io::Write;
use std::path::Path;

use crate::{check_source, format_source};

const USAGE: &str = "usage: gloss <command> <file.gloss>\ncommands: build, test, format, trace, explain, flowchart\n";

/// Runs one command; returns the exit status (0 success, 3 anything else).
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (Some(command), Some(file)) = (args.first(), args.get(1)) else {
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
    let diagnostics = check_source(text);
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

/// Replaces `path` with `text` by writing a temporary file beside it and renaming it over
/// the original, so an interrupted write cannot truncate the user's source.
fn write_atomically(path: &str, text: &str) -> std::io::Result<()> {
    let temp = format!("{path}.gloss-format-tmp");
    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
