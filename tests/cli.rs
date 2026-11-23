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

/// A test file in a folder of its own; dropping it removes the folder, even when a test fails.
struct TempFile(PathBuf);

impl std::ops::Deref for TempFile {
    type Target = std::path::Path;
    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

impl AsRef<std::path::Path> for TempFile {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let dir = self.0.parent().unwrap();
        // A test may have made the folder read-only; undo that so it can be removed.
        if let Ok(meta) = std::fs::metadata(dir) {
            let mut mode = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            mode.set_readonly(false);
            let _ = std::fs::set_permissions(dir, mode);
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn temp_file(name: &str, text: &str) -> TempFile {
    temp_named(name, &format!("{name}.gloss"), text)
}

/// Like `temp_file`, with the file name chosen separately from the folder tag.
fn temp_named(tag: &str, file_name: &str, text: &str) -> TempFile {
    let dir = std::env::temp_dir().join(format!("gloss-cli-{}-{}", tag, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(file_name);
    std::fs::write(&p, text).unwrap();
    TempFile(p)
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
}

#[test]
fn build_writes_an_executable_beside_the_source() {
    let p = temp_file("hello", "feature start\n  show \"hello\"\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "Built hello\n", ""));
    let exe = p.with_extension("");
    let mode =
        std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&exe).unwrap().permissions());
    assert_eq!(mode & 0o777, 0o755);
    assert_eq!(&std::fs::read(&exe).unwrap()[..4], b"\x7fELF");
}

#[test]
fn a_table_program_is_not_built_yet() {
    let text = "record Row\n  name as text\n\ntable rows of Row in \"rows.csv\"\n\nfeature start\n  show \"x\"\n";
    let p = temp_file("tabled", text);
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(err, "tables arrive in milestone 3.\n");
    assert!(!p.with_extension("").exists());
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
    assert_eq!(code, 3);
    assert!(
        err.starts_with("cannot read /nonexistent/x.gloss: "),
        "{err}"
    );
}

#[test]
fn build_with_only_warnings_exits_0() {
    let p = temp_file("warn", "feature start\n  set unused to 1\n  show \"hi\"\n");
    let (code, _, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(err.starts_with("line 2: warning: "), "{err}");
    assert!(p.with_extension("").exists());
}

#[test]
fn warnings_print_before_a_build_the_compiler_cannot_do_yet() {
    // A warning prints even when the build then stops on something not built yet.
    let p = temp_file("warn2", "feature start\n  set unused to list of 1\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("line 2: warning: "), "{err}");
    assert!(
        err.ends_with("this compiler cannot build this statement yet.\n"),
        "{err}"
    );
    assert!(!p.with_extension("").exists());
}

#[test]
fn format_rewrites_in_place() {
    let p = temp_file(
        "fmt",
        "feature start\n  set  t to 1\n  set t to t + 1\n  show t\n",
    );
    assert_eq!(gloss(&["format", p.to_str().unwrap()]).0, 0);
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        "feature start\n  set t to 1\n  add 1 to t\n  show t\n"
    );
    let (code, out, _) = gloss(&["format", p.to_str().unwrap()]);
    assert_eq!(
        (code, out.as_str()),
        (0, "fmt.gloss is already formatted\n")
    );
}

#[test]
fn format_leaves_unparsable_file_untouched() {
    let text = "feature start\n  set total as 0\n";
    let p = temp_file("fmtbad", text);
    let (code, out, err) = gloss(&["format", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(
        err,
        "line 2: set total as 0. set needs to. Did you mean set total to 0?\n"
    );
    assert_eq!(std::fs::read(&p).unwrap(), text.as_bytes());
}

fn leftovers(p: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(p.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into())
        .collect();
    names.sort();
    names
}

#[test]
fn format_leaves_no_temporary_file() {
    let p = temp_file("fmttmp", "feature start\n  set  t to 1\n  show t\n");
    assert_eq!(gloss(&["format", p.to_str().unwrap()]).0, 0);
    assert_eq!(leftovers(&p), ["fmttmp.gloss"]);
}

/// Root ignores permission bits, so tests that rely on them cannot fail as root.
#[cfg(unix)]
fn running_as_root() -> bool {
    let id = Command::new("id").arg("-u").output().unwrap();
    id.stdout == b"0\n"
}

#[cfg(unix)]
#[test]
fn format_keeps_the_file_mode() {
    use std::os::unix::fs::PermissionsExt;
    let p = temp_file("fmtmode", "feature start\n  set  t to 1\n  show t\n");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(gloss(&["format", p.to_str().unwrap()]).0, 0);
    let mode = std::fs::metadata(&p).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
}

#[cfg(unix)]
#[test]
fn format_through_a_symlink_formats_the_real_file() {
    let real = temp_file("fmtreal", "feature start\n  set  t to 1\n  show t\n");
    let link_dir = temp_file("fmtlink", "");
    let link = link_dir.parent().unwrap().join("link.gloss");
    std::os::unix::fs::symlink(&*real, &link).unwrap();
    assert_eq!(gloss(&["format", link.to_str().unwrap()]).0, 0);
    assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(
        std::fs::read_to_string(&*real).unwrap(),
        "feature start\n  set t to 1\n  show t\n"
    );
}

#[cfg(unix)]
#[test]
fn format_refuses_a_read_only_file() {
    use std::os::unix::fs::PermissionsExt;
    if running_as_root() {
        return;
    }
    let text = "feature start\n  set  t to 1\n  show t\n";
    let p = temp_file("fmtreadonly", text);
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o444)).unwrap();
    let (code, out, err) = gloss(&["format", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(
        err,
        format!("cannot write {}: the file is read-only\n", p.display())
    );
    assert_eq!(std::fs::read(&p).unwrap(), text.as_bytes());
    assert_eq!(leftovers(&p), ["fmtreadonly.gloss"]);
}

#[test]
fn format_handles_a_very_long_file_name() {
    let name = format!("{}.gloss", "a".repeat(240 - ".gloss".len()));
    let p = temp_named("fmtlong", &name, "feature start\n  set  t to 1\n  show t\n");
    let (code, _, err) = gloss(&["format", p.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        "feature start\n  set t to 1\n  show t\n"
    );
}

#[cfg(unix)]
#[test]
fn format_write_failure_keeps_original_and_reports_os_error() {
    use std::os::unix::fs::PermissionsExt;
    let text = "feature start\n  set  t to 1\n  show t\n";
    if running_as_root() {
        return;
    }
    let p = temp_file("fmtro", text);
    let dir = p.parent().unwrap();
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let (code, out, err) = gloss(&["format", p.to_str().unwrap()]);
    let names = leftovers(&p);
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(
        err.starts_with(&format!("cannot write {}: ", p.display())),
        "{err}"
    );
    assert_eq!(std::fs::read(&p).unwrap(), text.as_bytes());
    assert_eq!(names, ["fmtro.gloss"]);
}

#[test]
fn extra_arguments_print_the_usage() {
    let p = temp_file("extra", "feature start\n  show 1\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap(), "extra"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("usage: gloss"), "{err}");
}

#[test]
fn a_table_file_named_like_the_executable_is_refused() {
    let text = "record E\n  a as text\ntable es of E in \"clash\"\nfeature start\n  show 1\n";
    let p = temp_file("clash", text);
    let (code, _, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!(code, 3);
    assert_eq!(
        err,
        "line 3: table es of E in \"clash\". clash is the name of this program's executable; choose another file name.\n"
    );
}

#[test]
fn a_table_with_a_path_error_gets_no_executable_clash() {
    // The first table is only a clash; the second also repeats the file, which is its one error.
    let text = "record E\n  a as text\ntable es of E in \"dup\"\ntable fs of E in \"dup\"\nfeature start\n  show 1\n";
    let p = temp_file("dup", text);
    let (code, _, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!(code, 3);
    assert_eq!(
        err,
        "line 3: table es of E in \"dup\". dup is the name of this program's executable; choose another file name.\nline 4: table fs of E in \"dup\". dup is already the file of table es (line 3); one file belongs to one table.\n"
    );
}

#[test]
fn build_and_format_refuse_a_source_not_ending_in_gloss() {
    let program = "feature start\n  set  x  to 1\n  show x\n";
    for (tag, file_name) in [("noext", "prog"), ("txt", "prog.txt"), ("bare", ".gloss")] {
        let p = temp_named(tag, file_name, program);
        let path = p.to_str().unwrap();
        for command in ["build", "format"] {
            let (code, out, err) = gloss(&[command, path]);
            assert_eq!(
                (code, out.as_str(), err.as_str()),
                (3, "", "the source file must end in .gloss\n"),
                "{command} {file_name}"
            );
        }
        assert_eq!(std::fs::read_to_string(&p).unwrap(), program);
    }
}
