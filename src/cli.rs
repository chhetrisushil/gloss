//! The `gloss` command line.

use std::io::Write;
use std::path::Path;

use crate::check_source;

const USAGE: &str = "usage: gloss <command> <file.gloss>\ncommands: build, test, format, trace, explain, flowchart\n";

/// Runs one command; returns the exit status (0 success, 3 anything else).
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (Some(command), Some(file)) = (args.first(), args.get(1)) else {
        let _ = err.write_all(USAGE.as_bytes());
        return 3;
    };
    let milestone = match command.as_str() {
        "build" => 0,
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
    let Ok(text) = std::fs::read_to_string(file) else {
        let _ = writeln!(err, "cannot read {file}");
        return 3;
    };
    let name = Path::new(file)
        .file_name()
        .map_or(file.clone(), |n| n.to_string_lossy().into());
    build(&name, &text, out, err)
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
