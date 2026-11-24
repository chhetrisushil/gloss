//! Programs for the four file primitives, run in a real folder that holds the files the
//! test created (the program's current folder) and in `irsim` with the same files. Each step
//! writes its result and `b` (set beforehand, so a clobbered `b` shows); the files left
//! behind must agree too.

use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::Path;

use super::runner::{DEADLINE, TempDir, run_in, under_orb};
use super::{program, write_words};
use crate::ir::Reg::{A, B};
use crate::ir::{CWD, Data, ESCAPES, Global, Inst, LINK, Module, NOT_FOUND, OpenMode, Prim};
use crate::irsim::{self, Faults, Files};

const B_VALUE: i64 = 0x1111_2222_3333;
/// The descriptor the first successful `Open` gives (0 to 2 are standard).
const FD: i64 = 3;
const EBADF: i64 = -9;
const ENOTDIR: i64 = -20;

/// What a test puts in the folder: `(name, bytes)` files, folders, `(name, target)` links.
#[derive(Default)]
struct Setup {
    files: Vec<(&'static str, &'static str)>,
    folders: Vec<&'static str>,
    links: Vec<(&'static str, &'static str)>,
}

/// One step of a program: a primitive, its arguments, and whether to echo `out_buf` after.
struct Step {
    prim: Prim,
    args: Vec<Arg>,
    echo: usize,
}

enum Arg {
    Int(i64),
    Text(&'static str),
    OutBuf,
}

fn step(prim: Prim, args: Vec<Arg>) -> Step {
    Step {
        prim,
        args,
        echo: 0,
    }
}

fn open(mode: OpenMode, path: &'static str) -> Step {
    open_in(mode, CWD, path)
}

/// `Open` relative to the folder descriptor `folder` (or `CWD`).
fn open_in(mode: OpenMode, folder: i64, path: &'static str) -> Step {
    let mut args = vec![Arg::Int(folder), Arg::Text(path)];
    if mode == OpenMode::Create {
        args.push(Arg::Int(0o644));
    }
    step(Prim::Open(mode), args)
}

/// `Create` with the given permissions.
fn create_with(path: &'static str, permissions: i64) -> Step {
    let args = vec![Arg::Int(CWD), Arg::Text(path), Arg::Int(permissions)];
    step(Prim::Open(OpenMode::Create), args)
}

fn fd_step(prim: Prim) -> Step {
    step(prim, vec![Arg::Int(FD)])
}

fn module(steps: &[Step]) -> Module {
    let mut m = Module::default();
    let mut body = Vec::new();
    for s in steps {
        body.push(Inst::Const(B, B_VALUE));
        for arg in &s.args {
            match arg {
                Arg::Int(n) => body.push(Inst::Const(A, *n)),
                Arg::Text(t) => {
                    let mut bytes = t.as_bytes().to_vec();
                    bytes.push(0);
                    let id = m.text(&bytes);
                    body.extend([Inst::AddressOf(A, Data::Text(id)), Inst::AddImm(A, 8)]);
                }
                Arg::OutBuf => body.push(Inst::AddressOf(A, Data::Global(Global::OutBuf))),
            }
            body.push(Inst::Push(A));
        }
        body.push(Inst::System(s.prim));
        body.extend([Inst::Push(B), Inst::Push(A)]);
        body.extend(write_words(2));
        if s.echo > 0 {
            let len = i64::try_from(s.echo).unwrap();
            body.extend([
                Inst::Const(A, 1),
                Inst::Push(A),
                Inst::AddressOf(A, Data::Global(Global::OutBuf)),
                Inst::Push(A),
                Inst::Const(A, len),
                Inst::Push(A),
                Inst::System(Prim::Write),
            ]);
        }
    }
    body.extend([Inst::Const(A, 0), Inst::Push(A), Inst::System(Prim::Exit)]);
    program(&mut m, body);
    m
}

/// The files `setup` describes, as `irsim` keeps them.
fn model(setup: &Setup) -> Files {
    Files {
        files: setup
            .files
            .iter()
            .map(|(n, b)| (n.to_string(), b.as_bytes().to_vec()))
            .collect::<BTreeMap<_, _>>(),
        folders: setup.folders.iter().map(|f| f.to_string()).collect(),
        links: setup.links.iter().map(|(l, _)| l.to_string()).collect(),
        ..Files::default()
    }
}

/// The `(result, b)` words of each step in the program's standard output; `echo` bytes
/// follow the words of the step that asked for them.
fn results(out: &[u8], steps: &[Step]) -> Vec<i64> {
    let (mut at, mut found) = (0, Vec::new());
    for s in steps {
        let word =
            |i: usize| i64::from_le_bytes(out[at + 8 * i..at + 8 * i + 8].try_into().unwrap());
        assert_eq!(word(1), B_VALUE, "b did not survive {:?}", s.prim);
        found.push(word(0));
        at += 16 + s.echo;
    }
    found
}

/// Everything under `dir` except the runner's own files, as the in-memory model keeps it.
fn snapshot(dir: &Path) -> Files {
    fn walk(dir: &Path, rel: &str, out: &mut Files) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            if rel.is_empty() && name.starts_with("prog") {
                continue;
            }
            let path = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            let kind = entry.file_type().unwrap();
            if kind.is_symlink() {
                out.links.insert(path);
            } else if kind.is_dir() {
                out.folders.insert(path.clone());
                walk(&entry.path(), &path, out);
            } else {
                out.files.insert(path, std::fs::read(entry.path()).unwrap());
            }
        }
    }
    let mut out = Files::default();
    walk(dir, "", &mut out);
    out
}

/// `ENOSYS`: what Rosetta, which runs the Linux machine on a Mac, gives for `openat2`.
const ENOSYS: i64 = -38;

/// Whether the machine that runs the programs knows `openat2`.
fn has_openat2() -> Option<bool> {
    let steps = [open(OpenMode::Probe, ".")];
    let dir = TempDir::new();
    let ran = run_in(&module(&steps), b"", DEADLINE, &dir)?;
    Some(results(&ran.expect("the program ends").1, &steps) != [ENOSYS])
}

/// Runs the program in `irsim` and, unless `real` is false, for real; both must give
/// `wants` as the step results, and the same output and files.
fn check(name: &str, setup: &Setup, steps: &[Step], wants: &[i64], real: bool) {
    let m = module(steps);
    let mut files = model(setup);
    let want = irsim::run_with(&m, "_start", b"", &[], Faults::default(), &mut files);
    assert_eq!(results(&want.stdout, steps), wants, "{name}: irsim");
    if !real {
        return;
    }
    let dir = TempDir::new();
    for f in &setup.folders {
        std::fs::create_dir_all(dir.0.join(f)).unwrap();
    }
    for (n, b) in &setup.files {
        std::fs::write(dir.0.join(n), b).unwrap();
    }
    for (l, target) in &setup.links {
        symlink(target, dir.0.join(l)).unwrap();
    }
    let Some(got) = run_in(&m, b"", DEADLINE, &dir) else {
        return;
    };
    let got = got.expect("the program ends");
    assert_eq!(got, (want.status, want.stdout, want.stderr), "{name}");
    let (real, model) = (snapshot(&dir.0), (files.files, files.folders, files.links));
    assert_eq!(
        (real.files, real.folders, real.links),
        model,
        "{name}: files"
    );
}

/// Runs every file program. Under `orb` on Rosetta, which lacks `openat2`, the real runs are
/// skipped, visibly, and only `irsim` is checked against the expected results; ENOSYS on any
/// other runner is a failure.
pub fn programs() {
    let real = match has_openat2() {
        Some(true) => true,
        // Rosetta, which `orb` may use on a Mac, has no openat2: skip, visibly.
        Some(false) if under_orb() => false,
        Some(false) => panic!("openat2 gives ENOSYS on a native runner"),
        None => false,
    };
    if !real {
        let note = "difftest files skipped: no runner, or `orb` on Rosetta lacks openat2\n";
        let _ = std::io::stderr().write_all(note.as_bytes());
    }
    let a = Setup {
        files: vec![("a.txt", "hello")],
        ..Setup::default()
    };
    let mut read = step(Prim::Read, vec![Arg::Int(FD), Arg::OutBuf, Arg::Int(5)]);
    read.echo = 5;
    let steps = [
        open(OpenMode::Read, "a.txt"),
        read,
        fd_step(Prim::Close),
        fd_step(Prim::Close),
    ];
    check("open read", &a, &steps, &[FD, 5, 0, EBADF], real);
    let missing = [open(OpenMode::Read, "nope")];
    check("missing", &a, &missing, &[NOT_FOUND], real);
    let escapes = [open(OpenMode::Read, "../x")];
    check("escapes", &a, &escapes, &[ESCAPES], real);
    let link = Setup {
        files: vec![("a.txt", "hello"), ("real/x", "in")],
        folders: vec!["real"],
        links: vec![("ln", "a.txt"), ("link", "real")],
    };
    let steps = [open(OpenMode::Read, "ln"), open(OpenMode::Folder, "ln")];
    check("link final", &link, &steps, &[LINK, LINK], real);
    // A link in the middle of the path, not the last component, must be refused too.
    let steps = [
        open(OpenMode::Read, "link/x"),
        open(OpenMode::Create, "link/y"),
        open(OpenMode::Probe, "link/x"),
        open(OpenMode::Read, "real/x"),
    ];
    check("link inside", &link, &steps, &[LINK, LINK, LINK, FD], real);

    let steps = [
        open(OpenMode::Probe, "a.txt"),
        fd_step(Prim::Close),
        open(OpenMode::Probe, "nope"),
    ];
    check("probe", &a, &steps, &[FD, 0, NOT_FOUND], real);
    let old = Setup {
        files: vec![("t", "old bytes")],
        ..Setup::default()
    };
    let steps = [open(OpenMode::Create, "t"), fd_step(Prim::Close)];
    check("create truncates", &old, &steps, &[FD, 0], real);
    let rename = |from, to| {
        let names = vec![Arg::Int(FD), Arg::Text(from), Arg::Text(to)];
        step(Prim::Rename, names)
    };
    let steps = [
        open(OpenMode::Create, "t.new"),
        step(
            Prim::Write,
            vec![Arg::Int(FD), Arg::Text("data!"), Arg::Int(5)],
        ),
        fd_step(Prim::Sync),
        fd_step(Prim::Close),
        open(OpenMode::Folder, "."),
        rename("t.new", "t"),
        rename("t", "u"),
        fd_step(Prim::Sync),
        fd_step(Prim::Close),
    ];
    check(
        "create and rename",
        &old,
        &steps,
        &[FD, 5, 0, 0, FD, 0, 0, 0, 0],
        real,
    );
    let steps = [
        open(OpenMode::Folder, "."),
        fd_step(Prim::Sync),
        open(OpenMode::Folder, "a.txt"),
        step(Prim::Sync, vec![Arg::Int(77)]),
    ];
    check("sync folder", &a, &steps, &[FD, 0, ENOTDIR, EBADF], real);
    folder_cases(real);
    mode_cases(real);
}

/// `Mode` reads the permission bits through any kind of descriptor, a probe included.
fn mode_cases(real: bool) {
    let setup = Setup {
        files: vec![("a.txt", "x"), ("sub/x", "in")],
        folders: vec!["sub"],
        ..Setup::default()
    };
    let steps = [
        open(OpenMode::Probe, "a.txt"),
        fd_step(Prim::Mode),
        fd_step(Prim::Close),
        open(OpenMode::Folder, "sub"),
        fd_step(Prim::Mode),
        step(Prim::Mode, vec![Arg::Int(77)]),
    ];
    check(
        "mode",
        &setup,
        &steps,
        &[FD, 0o644, 0, FD, 0o755, EBADF],
        real,
    );
    // A created file gets the permissions asked for, less the umask's; a file that exists
    // keeps its own.
    let steps = [
        create_with("private", 0o600),
        fd_step(Prim::Mode),
        create_with("group", 0o640),
        step(Prim::Mode, vec![Arg::Int(FD + 1)]),
        create_with("wide", 0o666),
        step(Prim::Mode, vec![Arg::Int(FD + 2)]),
        create_with("a.txt", 0o600),
        step(Prim::Mode, vec![Arg::Int(FD + 3)]),
    ];
    let wants = [FD, 0o600, FD + 1, 0o640, FD + 2, 0o644, FD + 3, 0o644];
    check("created modes", &setup, &steps, &wants, real);
}

/// Opens and renames relative to a folder descriptor, as `save` does: "beneath" is the
/// descriptor's folder, not the current one.
fn folder_cases(real: bool) {
    let setup = Setup {
        files: vec![("a.txt", "top"), ("sub/x", "in")],
        folders: vec!["sub"],
        ..Setup::default()
    };
    let steps = [
        open(OpenMode::Folder, "sub"),
        open_in(OpenMode::Read, FD, "x"),
        open_in(OpenMode::Read, FD, "../a.txt"),
        open_in(OpenMode::Create, FD, "y"),
        open(OpenMode::Read, "/a.txt"),
        open(OpenMode::Probe, "sub"),
    ];
    let wants = [FD, FD + 1, ESCAPES, FD + 2, ESCAPES, FD + 3];
    check("beneath a folder", &setup, &steps, &wants, real);
    let steps = [
        open(OpenMode::Folder, "sub"),
        step(
            Prim::Rename,
            vec![Arg::Int(FD), Arg::Text("x"), Arg::Text("z")],
        ),
        fd_step(Prim::Sync),
        fd_step(Prim::Close),
    ];
    check("rename in a folder", &setup, &steps, &[FD, 0, 0, 0], real);
}
