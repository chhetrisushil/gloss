//! A permanent differential test: whole modules run in `irsim` and as real executables
//! must give the same standard output, standard error and exit status. It needs a Linux
//! runner and says so on standard error when there is none.

mod calls;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ir::Reg::{A, B};
use crate::ir::{Function, Inst, Module, Prim};
use crate::irsim;
use crate::target::x86_64_linux;

/// What a run did: status, standard output and standard error.
type Ran = (i32, Vec<u8>, Vec<u8>);

/// Mirrors `tests/run_support`: `GLOSS_RUNNER` (split on spaces), else `orb -m gloss-amd64`
/// on macOS, else direct on x86-64 Linux; `None` (after a visible skip) otherwise.
fn real(m: &Module, stdin: &[u8]) -> Option<Ran> {
    let prefix: Vec<String> = match std::env::var("GLOSS_RUNNER") {
        Ok(r) => r
            .split(' ')
            .filter(|w| !w.is_empty())
            .map(String::from)
            .collect(),
        Err(_) if cfg!(target_os = "macos") => {
            ["orb", "-m", "gloss-amd64"].map(String::from).to_vec()
        }
        Err(_) if cfg!(all(target_os = "linux", target_arch = "x86_64")) => Vec::new(),
        Err(_) => {
            let _ = std::io::stderr().write_all(b"difftest skipped: no x86-64 Linux runner\n");
            return None;
        }
    };
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let name = format!(
        "difftest-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::SeqCst)
    );
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/tmp")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prog");
    std::fs::write(&path, x86_64_linux().executable(m).expect("builds")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut cmd = match prefix.split_first() {
        Some((program, args)) => {
            let mut c = Command::new(program);
            c.args(args).arg(&path);
            c
        }
        None => Command::new(&path),
    };
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("runs");
    // A program may exit without reading its input: a broken pipe is not an error.
    let _ = child.stdin.take().expect("stdin").write_all(stdin);
    let out = child.wait_with_output().expect("finishes");
    std::fs::remove_dir_all(&dir).unwrap();
    Some((out.status.code().expect("exits"), out.stdout, out.stderr))
}

fn simulated(m: &Module, stdin: &[u8]) -> Ran {
    let o = irsim::run(m, "_start", stdin);
    (o.status, o.stdout, o.stderr)
}

/// A module whose `_start` calls each case in turn, writing its result as 8 bytes.
fn value_module(helpers: Vec<Function>, cases: &[Function]) -> Module {
    let mut start = vec![Inst::Enter(0)];
    for c in cases {
        start.extend([
            Inst::Call(c.name.clone()),
            Inst::Push(A),
            Inst::StackPointer(B),
            Inst::Const(A, 1),
            Inst::Push(A),
            Inst::Push(B),
            Inst::Const(A, 8),
            Inst::Push(A),
            Inst::System(Prim::Write),
            Inst::Pop(A),
        ]);
    }
    start.extend([Inst::Const(A, 0), Inst::Push(A), Inst::System(Prim::Exit)]);
    let mut m = Module::default();
    m.functions.push(Function {
        name: "_start".into(),
        params: 0,
        slots: 0,
        body: start,
    });
    m.functions.extend(helpers);
    m.functions.extend(cases.iter().cloned());
    m
}

/// Compares the real run of `m` with the simulated one; `names` label the 8-byte results.
fn agree(m: &Module, names: &[String], stdin: &[u8]) {
    let want = simulated(m, stdin);
    let Some(got) = real(m, stdin) else { return };
    let words = |b: &[u8]| b.chunks(8).map(<[u8]>::to_vec).collect::<Vec<_>>();
    for (i, (g, w)) in words(&got.1).iter().zip(words(&want.1)).enumerate() {
        let name = names.get(i).map_or("?", String::as_str);
        assert_eq!(*g, w, "{name}: real {g:?} but irsim {w:?}");
    }
    assert_eq!(got, want);
}

#[test]
fn value_cases_agree() {
    let cases = calls::cases();
    let names: Vec<String> = cases.iter().map(|c| c.name.clone()).collect();
    agree(&value_module(calls::helpers(), &cases), &names, b"");
}
