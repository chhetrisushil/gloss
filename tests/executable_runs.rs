//! Whole IR modules become executables that run on Linux.

mod run_support;

use gloss::ir::Reg::A;
use gloss::ir::{Function, Inst, Module, Prim};
use gloss::target::x86_64_linux;
use run_support::{TempDir, run};
use std::os::unix::fs::PermissionsExt;

fn start(body: Vec<Inst>) -> Function {
    Function {
        name: "_start".to_string(),
        params: 0,
        slots: 0,
        body,
    }
}

/// Builds `m`, runs it with no input, and returns what it did; `None` without a runner.
fn build_and_run(m: &Module, name: &str) -> Option<run_support::Ran> {
    let bytes = x86_64_linux().executable(m).expect("the module builds");
    let dir = TempDir::new(name);
    let path = dir.0.join("prog");
    std::fs::write(&path, bytes).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    run(&path, b"")
}

#[test]
fn a_module_writing_hi_prints_it_and_exits_zero() {
    let mut m = Module::default();
    let hi = m.text(b"hi\n");
    m.functions.push(start(vec![
        Inst::Enter(0),
        Inst::Const(A, 1),
        Inst::Push(A),
        Inst::AddressOf(A, gloss::ir::Data::Text(hi)),
        Inst::AddImm(A, 8),
        Inst::Push(A),
        Inst::Const(A, 3),
        Inst::Push(A),
        Inst::System(Prim::Write),
        Inst::Const(A, 0),
        Inst::Push(A),
        Inst::System(Prim::Exit),
    ]));
    if let Some(ran) = build_and_run(&m, "hi") {
        assert_eq!(ran.unwrap(), (0, b"hi\n".to_vec(), Vec::new()));
    }
}

#[test]
fn a_module_without_start_is_refused() {
    let err = x86_64_linux().executable(&Module::default()).unwrap_err();
    assert_eq!(err, "the module has no _start function");
}
