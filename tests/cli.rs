use std::path::PathBuf;
use std::process::Command;

fn gloss(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_gloss"))
        .args(args)
        .output()
        .unwrap();
    let text = |b| String::from_utf8(b).unwrap();
    (o.status.code().unwrap(), text(o.stdout), text(o.stderr))
}

/// Writes `text` into a folder of its own; the caller removes the folder.
fn temp_file(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gloss-cli-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.gloss"));
    std::fs::write(&p, text).unwrap();
    p
}

fn cleanup(p: &std::path::Path) {
    std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
}

#[test]
fn build_reports_errors_with_status_3() {
    let p = temp_file("bad", "feature start\n  set total as 0\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(
        err,
        "line 2: set total as 0. set needs to. Did you mean set total to 0?\n"
    );
    cleanup(&p);
}

#[test]
fn build_checks_clean_program() {
    let p = temp_file("good", "feature start\n  show \"hello\"\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "Checked good.gloss: no errors (code generation arrives in milestone 2)\n"
    );
    cleanup(&p);
}

#[test]
fn usage_and_unbuilt_commands_exit_3() {
    assert_eq!(gloss(&[]).0, 3);
    assert_eq!(gloss(&["frobnicate", "x.gloss"]).0, 3);
    let (code, _, err) = gloss(&["test", "x.gloss"]);
    assert_eq!(
        (code, err.as_str()),
        (
            3,
            "gloss test is not built yet; it arrives in milestone 4\n"
        )
    );
    let (code, _, err) = gloss(&["build", "/nonexistent/x.gloss"]);
    assert_eq!(
        (code, err.as_str()),
        (3, "cannot read /nonexistent/x.gloss\n")
    );
}

#[test]
fn build_with_only_warnings_exits_0() {
    // An unused variable is the simplest warning; see check for the exact text.
    let p = temp_file("warn", "feature start\n  set unused to 1\n  show \"hi\"\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(err.starts_with("line 2: warning: "), "{err}");
    assert_eq!(
        out,
        "Checked warn.gloss: no errors (code generation arrives in milestone 2)\n"
    );
    cleanup(&p);
}
