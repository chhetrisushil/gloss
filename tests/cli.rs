use std::path::PathBuf;
use std::process::Command;

mod run_support;
#[cfg(unix)]
use run_support::running_as_root;

fn gloss(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_gloss"))
        .args(args)
        // The test command makes its private folder under the temporary folder.
        .env("TMPDIR", env!("CARGO_TARGET_TMPDIR"))
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
fn a_table_program_builds() {
    let text = "record Row\n  name as text\n\ntable rows of Row in \"rows.csv\"\n\nfeature start\n  show \"x\"\n";
    let p = temp_file("tabled", text);
    let (code, out, err) = gloss(&["build", p.to_str().unwrap(), "--allow", "rows.csv"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.ends_with("Approval saved in tabled.allow\nBuilt tabled\n"),
        "{out}"
    );
    assert_eq!(err, "");
    assert!(p.with_extension("").exists());
}

#[test]
fn usage_and_unknown_commands_exit_3() {
    assert_eq!(gloss(&[]).0, 3);
    let (code, out, err) = gloss(&["frobnicate", "x.gloss"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("usage: gloss <command>"), "{err}");
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
fn warnings_print_for_a_table_program_that_builds() {
    // The warning prints, and the build goes on.
    let p = temp_file(
        "warn2",
        "record Row\n  name as text\n\ntable rows of Row in \"rows.csv\"\n\nfeature start\n  set xs to 1\n",
    );
    let (code, _, err) = gloss(&["build", p.to_str().unwrap(), "--allow", "rows.csv"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.starts_with("line 7: warning: "), "{err}");
    assert!(p.with_extension("").exists());
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

#[test]
fn the_usage_lists_the_options() {
    let (code, _, err) = gloss(&[]);
    assert_eq!(code, 3);
    assert!(
        err.ends_with("\noptions: --allow <file> (build, trace)\n"),
        "{err}"
    );
}

#[test]
fn allow_needs_a_file_name() {
    let p = temp_file("allowname", "feature start\n  show 1\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap(), "--allow"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("usage: gloss"), "{err}");
}

#[test]
fn format_refuses_allow() {
    let p = temp_file("fmtallow", "feature start\n  show 1\n");
    let (code, out, err) = gloss(&["format", p.to_str().unwrap(), "--allow", "a.csv"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("usage: gloss"), "{err}");
}

#[test]
fn allow_may_repeat_after_the_file() {
    let p = temp_file("allowtwice", "feature start\n  show 1\n");
    let a = [
        "build",
        p.to_str().unwrap(),
        "--allow",
        "a.csv",
        "--allow",
        "b.csv",
    ];
    let (_, _, err) = gloss(&a);
    assert!(!err.contains("usage:"), "{err}");
    let (code, _, err) = gloss(&["build", p.to_str().unwrap(), "--other", "a.csv"]);
    assert_eq!(code, 3);
    assert!(err.starts_with("usage: gloss"), "{err}");
}

const E_HEAD: &str = "record E\n  amount as number\ntable es of E in \"e.csv\"\n";
const LISTED: &str = "This program can:\n  read only          e.csv\n  nothing else\n";

fn allow_body(lines: &str) -> String {
    format!("note Files this program may touch; approved for t.gloss.\n{lines}")
}

fn tabled(tag: &str, body: &str) -> TempFile {
    temp_named(tag, "t.gloss", &format!("{E_HEAD}{body}"))
}

#[test]
fn a_table_program_without_approval_exits_4() {
    let p = tabled("noapproval", "feature start\n  load es\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (4, LISTED));
    assert_eq!(
        err,
        "Not approved: e.csv. Build in a terminal to answer, or add --allow e.csv.\n"
    );
    assert!(!p.with_extension("").exists() && !p.with_extension("allow").exists());
}

#[test]
fn allow_writes_the_approval() {
    let p = tabled("writes", "feature start\n  load es\n  save es\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap(), "--allow", "e.csv"]);
    let list = "This program can:\n  read and replace   e.csv\n  nothing else\n";
    assert_eq!(code, 0);
    assert_eq!(out, format!("{list}Approval saved in t.allow\nBuilt t\n"));
    assert_eq!(err, "");
    let want = allow_body("  read and replace   e.csv\n");
    assert_eq!(
        std::fs::read_to_string(p.with_extension("allow")).unwrap(),
        want
    );
    let mode = std::fs::metadata(p.with_extension("allow")).unwrap();
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(&mode.permissions()) & 0o777,
        0o644
    );
}

#[test]
fn a_recorded_approval_asks_nothing() {
    let p = tabled("recorded", "feature start\n  load es\n");
    gloss(&["build", p.to_str().unwrap(), "--allow", "e.csv"]);
    let before = std::fs::metadata(p.with_extension("allow"))
        .unwrap()
        .modified()
        .unwrap();
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    let built = format!("{LISTED}Built t\n");
    assert_eq!((code, out.as_str(), err.as_str()), (0, built.as_str(), ""));
    let after = std::fs::metadata(p.with_extension("allow"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn read_only_to_replace_needs_approval_again() {
    let p = tabled("replace", "feature start\n  load es\n  save es\n");
    let old = allow_body("  read only          e.csv\n");
    std::fs::write(p.with_extension("allow"), &old).unwrap();
    let (code, _, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!(code, 4, "{err}");
    assert_eq!(
        std::fs::read_to_string(p.with_extension("allow")).unwrap(),
        old
    );
    assert!(!p.with_extension("").exists());
    assert!(err.starts_with("Not approved: e.csv."), "{err}");
}

#[test]
fn a_dropped_table_shrinks_allow() {
    let p = tabled("shrink", "feature start\n  load es\n");
    let old = allow_body("  read only          e.csv\n  read only          f.csv\n");
    std::fs::write(p.with_extension("allow"), old).unwrap();
    let (code, out, _) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(
        out.ends_with("Approval saved in t.allow\nBuilt t\n"),
        "{out}"
    );
    let want = allow_body("  read only          e.csv\n");
    assert_eq!(
        std::fs::read_to_string(p.with_extension("allow")).unwrap(),
        want
    );
}

#[test]
fn a_broken_allow_exits_3() {
    let p = tabled("broken", "feature start\n  load es\n");
    std::fs::write(p.with_extension("allow"), "note x\nnonsense\n").unwrap();
    let (code, _, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!(code, 3);
    assert_eq!(
        err,
        "t.allow line 2: this line is not an approval; expected read only or read and replace and a file name.\n"
    );
}

#[test]
fn an_allow_for_an_unknown_file_exits_3() {
    let p = tabled("unknown", "feature start\n  load es\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap(), "--allow", "x.csv"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(err, "--allow x.csv: this program has no table in x.csv.\n");
    assert!(!p.with_extension("allow").exists());
}

#[test]
fn a_program_without_tables_prints_no_list() {
    let p = temp_named("notables", "x.gloss", "feature start\n  show 1\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "Built x\n", ""));
    assert!(!p.with_extension("allow").exists());
    let (code, _, err) = gloss(&["build", p.to_str().unwrap(), "--allow", "x.csv"]);
    assert_eq!(
        (code, err.as_str()),
        (3, "--allow x.csv: this program has no table in x.csv.\n")
    );
}

#[test]
fn test_reports_errors_with_status_3() {
    // Errors come before the platform refusal, so a Mac user sees them, not the Linux message.
    let p = temp_file(
        "testerr",
        "feature start\n  show nope\n\ntest \"t\"\n  expect 1 is 1\n",
    );
    let (code, out, err) = gloss(&["test", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("line 2: show nope"), "{err}");
    assert!(!err.contains("Linux only"), "{err}");
}

#[test]
fn test_with_no_tests_says_so() {
    let p = temp_file("notests", "feature start\n  show 1\n");
    let dir = p.parent().unwrap().to_path_buf();
    let (code, out, err) = gloss(&["test", p.to_str().unwrap()]);
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (0, "No tests in notests.gloss\n", "")
    );
    assert_eq!(std::fs::read_dir(dir).unwrap().count(), 1);
}

/// Passes before `test` is built (the parser already refuses `--allow` on it); kept as a
/// regression guard so that stays true.
#[test]
fn test_refuses_allow() {
    let p = temp_file("testallow", "feature start\n  show 1\n");
    let (code, out, err) = gloss(&["test", p.to_str().unwrap(), "--allow", "a.csv"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("usage: gloss"), "{err}");
}

/// Runs `gloss test` on a program that has a table, `load`, `save` and a test, with an `.allow`
/// file beside it: the listing and the `.allow` bytes must be the same afterwards.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn test_never_touches_allow_or_beside_the_source() {
    let text = "record Row\n  name as text\n\ntable rows of Row in \"rows.csv\"\n\nfeature start\n  load rows\n  save rows\n\ntest \"saves and loads\"\n  append Row with name \"a\" to rows\n  save rows\n  load rows\n  expect count of rows is 1\n";
    let p = temp_file("testkeep", text);
    let allow = p.with_extension("allow");
    std::fs::write(&allow, "note keep me\n").unwrap();
    let listing = |d: &std::path::Path| {
        let mut names: Vec<_> = std::fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    let before = listing(p.parent().unwrap());
    let (code, out, err) = gloss(&["test", p.to_str().unwrap()]);
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (0, "pass  saves and loads\n\n1 passed, 0 failed\n", "")
    );
    assert_eq!(listing(p.parent().unwrap()), before);
    assert_eq!(std::fs::read(&allow).unwrap(), b"note keep me\n");
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
#[test]
fn test_refuses_off_linux() {
    let p = temp_file(
        "offlinux",
        "feature start\n  show 1\n\ntest \"t\"\n  expect 1 is 1\n",
    );
    let (code, out, err) = gloss(&["test", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(err, "gloss test runs tests on Linux only\n");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn a_passing_program_exits_0() {
    let p = temp_file(
        "testpass",
        "feature start\n  show 1\n\ntest \"t\"\n  expect 1 is 1\n",
    );
    let (code, out, err) = gloss(&["test", p.to_str().unwrap()]);
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (0, "pass  t\n\n1 passed, 0 failed\n", "")
    );
    assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn a_failing_program_exits_1() {
    let text = "feature start\n  show 1\n\ntest \"t\"\n  expect 1 is 2\n";
    let p = temp_file("testfail", text);
    let (code, out, _) = gloss(&["test", p.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(out.starts_with("fail  t\n"), "{out}");
    assert!(out.ends_with("0 passed, 1 failed\n"), "{out}");
}

/// The program of the trace tests: a call, a set, an if and a show.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const TRACED: &str = "feature double\n  needs n as number\n  give n * 2\n\nfeature start\n  set r to double with 21\n  if r is greater than 5\n    show \"big\"\n";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn trace_narrates_and_writes_the_trace_file() {
    let p = temp_file("tracenarrates", TRACED);
    let (code, out, err) = gloss(&["trace", p.to_str().unwrap()]);
    let narration = "line 6: calling double with n = 21\nline 6: double gives 42\nline 6: r is 42\nline 7: 42 is greater than 5, so the block runs\nbig\n";
    assert_eq!((code, out.as_str(), err.as_str()), (0, narration, ""));
    let events = concat!(
        "{\"line\":6,\"event\":\"enter\",\"feature\":\"double\",\"inputs\":{\"n\":{\"number\":\"21\"}}}\n",
        "{\"line\":6,\"event\":\"leave\",\"feature\":\"double\",\"result\":{\"number\":\"42\"}}\n",
        "{\"line\":6,\"event\":\"set\",\"name\":\"r\",\"to\":{\"number\":\"42\"}}\n",
        "{\"line\":7,\"event\":\"check\",\"condition\":\"r is greater than 5\",\"values\":{\"r\":{\"number\":\"42\"}},\"result\":\"yes\"}\n",
        "{\"line\":8,\"event\":\"show\",\"text\":\"big\"}\n",
    );
    assert_eq!(
        std::fs::read_to_string(p.with_extension("trace")).unwrap(),
        events
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn trace_of_a_failure_exits_2() {
    let text = "feature start\n  set n to 0\n  set x to 10\n  divide x by n\n";
    let p = temp_file("tracefail", text);
    let (code, out, err) = gloss(&["trace", p.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert_eq!(out, "line 2: n is 0\nline 3: x is 10\n");
    assert_eq!(err, "line 4: failure 1: dividing by zero (n is 0)\n");
    // The buffered file is complete: it ends with the failure event and its newline.
    let events = std::fs::read_to_string(p.with_extension("trace")).unwrap();
    assert_eq!(events.lines().count(), 3, "{events}");
    assert!(
        events
            .lines()
            .last()
            .unwrap()
            .contains("\"event\":\"fail\"")
    );
    assert!(events.ends_with("}\n"));
}

/// Approval comes before the host check, so this runs on every host.
#[test]
fn trace_with_a_table_asks_for_approval() {
    let p = tabled("tracenoapproval", "feature start\n  load es\n");
    let (code, out, err) = gloss(&["trace", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (4, LISTED));
    assert_eq!(
        err,
        "Not approved: e.csv. Trace in a terminal to answer, or add --allow e.csv.\n"
    );
    assert!(!p.with_extension("trace").exists() && !p.with_extension("allow").exists());
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn trace_with_allow_runs() {
    let p = tabled("traceallow", "feature start\n  show 1\n  load es\n");
    let (code, out, _) = gloss(&["trace", p.to_str().unwrap(), "--allow", "e.csv"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Approval saved in t.allow\n"), "{out}");
    assert!(out.contains("\n1\n"), "{out}");
    assert!(p.with_extension("allow").exists());
    assert!(p.with_extension("trace").exists());
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
#[test]
fn trace_refuses_off_linux() {
    let p = temp_file("traceoff", "feature start\n  show 1\n");
    let (code, out, err) = gloss(&["trace", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(err, "gloss trace runs programs on Linux only\n");
    assert!(!p.with_extension("trace").exists());
}

/// Off Linux the approval is still recorded before the refusal.
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
#[test]
fn trace_records_the_approval_before_refusing_off_linux() {
    let p = tabled("traceoffallow", "feature start\n  load es\n");
    let (code, _, err) = gloss(&["trace", p.to_str().unwrap(), "--allow", "e.csv"]);
    assert_eq!(
        (code, err.as_str()),
        (3, "gloss trace runs programs on Linux only\n")
    );
    assert!(p.with_extension("allow").exists());
}

/// A folder where the private folder for a run cannot be made, so setup fails.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn gloss_with_no_temp_folder(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_gloss"))
        .args(args)
        .env("TMPDIR", "/gloss-no-such-folder/tmp")
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8(o.stderr).unwrap(),
    )
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn trace_setup_failures_do_not_say_test() {
    let p = temp_file("tracesetup", TRACED);
    let (code, err) = gloss_with_no_temp_folder(&["trace", p.to_str().unwrap()]);
    assert_eq!(code, 3);
    assert!(
        err.starts_with("cannot make a private folder for the traced run: "),
        "{err}"
    );
    assert!(!err.contains("test"), "{err}");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn test_setup_failures_keep_the_test_wording() {
    let text = "feature start\n  show 1\n\ntest \"t\"\n  expect 1 is 1\n";
    let p = temp_file("testsetup", text);
    let (code, err) = gloss_with_no_temp_folder(&["test", p.to_str().unwrap()]);
    assert_eq!(code, 3);
    assert!(
        err.starts_with("cannot make a private folder for the test run: "),
        "{err}"
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn trace_file_that_cannot_be_created_says_why() {
    let p = temp_file("tracecreate", TRACED);
    std::fs::create_dir(p.with_extension("trace")).unwrap();
    let (code, out, err) = gloss(&["trace", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(
        err,
        "tracecreate.trace cannot be written: Is a directory (os error 21)\n"
    );
}

#[test]
fn explain_prints_the_listing_and_exits_0() {
    let p = temp_file("x", "feature start\n  show 1\n");
    let dir = p.parent().unwrap().to_path_buf();
    let (code, out, err) = gloss(&["explain", p.to_str().unwrap()]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(
        out.starts_with("Explain x.gloss for x86-64 Linux\n\nfeature start (line 1)\n"),
        "{out}"
    );
    assert!(out.contains("\nruntime\n"), "{out}");
    let names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["x.gloss"]);
}

#[test]
fn explain_of_a_program_with_errors_exits_3() {
    let p = temp_file("experr", "feature start\n  show nope\n");
    let (code, out, err) = gloss(&["explain", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("line 2: show nope"), "{err}");
}

#[test]
fn explain_with_allow_is_a_usage_error() {
    let p = temp_file("expallow", "feature start\n  show 1\n");
    let (code, out, err) = gloss(&["explain", p.to_str().unwrap(), "--allow", "a.csv"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("usage: gloss"), "{err}");
}

const TWO_FEATURES: &str = "feature start\n  show 1\n\nfeature helper\n  show 2\n";

fn beside(p: &std::path::Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(p.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn flowchart_writes_one_svg_per_feature() {
    use std::os::unix::fs::PermissionsExt;
    let p = temp_named("flowtwo", "x.gloss", TWO_FEATURES);
    let (code, out, err) = gloss(&["flowchart", p.to_str().unwrap()]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, "Wrote x.start.svg\nWrote x.helper.svg\n");
    for name in ["x.start.svg", "x.helper.svg"] {
        let svg = std::fs::read_to_string(p.with_file_name(name)).unwrap();
        assert!(svg.starts_with("<svg "), "{svg}");
        let mode = std::fs::metadata(p.with_file_name(name)).unwrap();
        assert_eq!(mode.permissions().mode() & 0o777, 0o644);
    }
}

#[test]
fn flowchart_of_a_program_with_errors_writes_nothing() {
    let p = temp_named("flowerr", "x.gloss", "feature start\n  show nope\n");
    let (code, out, err) = gloss(&["flowchart", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("line 2: show nope"), "{err}");
    assert_eq!(beside(&p), ["x.gloss"]);
}

#[test]
fn flowchart_with_allow_is_a_usage_error() {
    let p = temp_named("flowallow", "x.gloss", TWO_FEATURES);
    let (code, out, err) = gloss(&["flowchart", p.to_str().unwrap(), "--allow", "a.csv"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("usage: gloss"), "{err}");
    assert_eq!(beside(&p), ["x.gloss"]);
}

#[test]
fn flowchart_reports_a_file_it_cannot_write() {
    let p = temp_named("flowfail", "x.gloss", TWO_FEATURES);
    std::fs::create_dir(p.with_file_name("x.start.svg")).unwrap();
    let (code, out, err) = gloss(&["flowchart", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.starts_with("x.start.svg cannot be written: "), "{err}");
    assert!(
        err.trim_end().len() > "x.start.svg cannot be written: ".len(),
        "{err}"
    );
    assert!(!p.with_file_name("x.helper.svg").exists());
}

#[test]
fn flowchart_keeps_the_files_written_before_a_failure() {
    let p = temp_named("flowsecond", "x.gloss", TWO_FEATURES);
    std::fs::create_dir(p.with_file_name("x.helper.svg")).unwrap();
    let (code, out, err) = gloss(&["flowchart", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, "Wrote x.start.svg\n"));
    let prefix = "x.helper.svg cannot be written: ";
    assert!(err.starts_with(prefix), "{err}");
    assert!(err.trim_end().len() > prefix.len(), "{err}");
    assert!(p.with_file_name("x.start.svg").is_file());
}

#[test]
fn flowchart_of_a_bare_file_name_writes_in_the_current_folder() {
    let p = temp_named("flowbare", "x.gloss", TWO_FEATURES);
    let o = Command::new(env!("CARGO_BIN_EXE_gloss"))
        .args(["flowchart", "x.gloss"])
        .current_dir(p.parent().unwrap())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(o.stdout).unwrap(),
        "Wrote x.start.svg\nWrote x.helper.svg\n"
    );
    assert!(p.with_file_name("x.start.svg").is_file());
    assert!(p.with_file_name("x.helper.svg").is_file());
}

#[test]
fn flowchart_of_a_program_with_only_warnings_still_writes() {
    let p = temp_named(
        "flowwarn",
        "x.gloss",
        "feature helper\n  give 1\n  show 2\n\nfeature start\n  show 1\n",
    );
    let (code, out, err) = gloss(&["flowchart", p.to_str().unwrap()]);
    assert_eq!(
        (code, out.as_str()),
        (0, "Wrote x.helper.svg\nWrote x.start.svg\n"),
        "{err}"
    );
    assert!(err.contains("this line never runs"), "{err}");
    assert!(p.with_file_name("x.start.svg").is_file());
}
