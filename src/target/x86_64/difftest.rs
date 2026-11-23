//! A permanent differential test: whole modules run in `irsim` and as real executables
//! must give the same standard output, standard error and exit status. It needs a Linux
//! runner and says so on standard error when there is none.

mod calls;
mod memory;
mod num;
mod ops;
mod runner;
mod sys;

use runner::{DEADLINE, run_program};

use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Inst, Module, Prim};
use crate::irsim;

/// What a run did: status, standard output and standard error.
type Ran = (i32, Vec<u8>, Vec<u8>);

/// Runs `m` for real, with `stdin`; `None` without a runner. A hang or a signal is a failure.
fn real(m: &Module, stdin: &[u8]) -> Option<Ran> {
    run_program(m, stdin, DEADLINE).map(|ran| ran.expect("the program ends"))
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
    let mut cases = calls::cases();
    cases.extend(ops::cases());
    cases.extend(memory::cases());
    let names: Vec<String> = cases.iter().map(|c| c.name.clone()).collect();
    agree(&value_module(calls::helpers(), &cases), &names, b"");
}

/// Writes the `words` 8-byte words on top of the stack to standard output, then pops them.
/// It uses `b` as scratch.
fn write_words(words: i64) -> Vec<Inst> {
    vec![
        Inst::StackPointer(B),
        Inst::Const(A, 1),
        Inst::Push(A),
        Inst::Push(B),
        Inst::Const(A, 8 * words),
        Inst::Push(A),
        Inst::System(Prim::Write),
    ]
}

/// Writes the text constant `text` (bytes only) to standard output, then clobbers `a`.
fn write_text(text: u32, len: i64) -> Vec<Inst> {
    vec![
        Inst::Const(A, 1),
        Inst::Push(A),
        Inst::AddressOf(A, Data::Text(text)),
        Inst::AddImm(A, 8),
        Inst::Push(A),
        Inst::Const(A, len),
        Inst::Push(A),
        Inst::System(Prim::Write),
    ]
}

/// A program: `_start` has the body `body`, which must end by exiting.
fn program(m: &mut Module, body: Vec<Inst>) {
    let mut all = vec![Inst::Enter(0)];
    all.extend(body);
    m.functions.push(Function {
        name: "_start".into(),
        params: 0,
        slots: 0,
        body: all,
    });
}

/// Runs the whole program in both machines and compares everything it did.
fn agree_program(name: &str, m: &Module, stdin: &[u8]) {
    let want = simulated(m, stdin);
    if let Some(got) = real(m, stdin) {
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn number_programs_agree() {
    num::programs(&mut |name, m| agree_program(name, m, b""));
}

#[test]
fn system_programs_agree() {
    sys::programs(&mut |name, m, stdin| agree_program(name, m, stdin));
}

#[test]
fn a_program_that_never_ends_times_out() {
    let mut m = Module::default();
    program(
        &mut m,
        vec![
            Inst::Mark(crate::ir::Label(1)),
            Inst::Jump(crate::ir::Label(1)),
        ],
    );
    if let Some(ran) = run_program(&m, b"", std::time::Duration::from_millis(500)) {
        assert!(ran.unwrap_err().starts_with("timed out"));
    }
}
