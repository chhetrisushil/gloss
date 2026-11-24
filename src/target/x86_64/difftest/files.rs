//! Programs for the four file primitives, run in a real folder that holds the files the
//! test created (the program's current folder) and in `irsim` with the same files. Each step
//! writes its result and `b` (set beforehand, so a clobbered `b` shows); the files left
//! behind must agree too.

use std::collections::BTreeMap;

use super::{program, write_words};
use crate::ir::Reg::{A, B};
use crate::ir::{CWD, Data, Global, Inst, Module, OpenMode, Prim};
use crate::irsim::Files;

const B_VALUE: i64 = 0x1111_2222_3333;
/// The descriptor the first successful `Open` gives (0 to 2 are standard).
const FD: i64 = 3;
const EBADF: i64 = -9;

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
    step(Prim::Open(mode), vec![Arg::Int(folder), Arg::Text(path)])
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
        synced: Vec::new(),
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

#[test]
fn irsim_runs_an_open_and_a_read() {
    let setup = Setup {
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
    let out = crate::irsim::run_with(
        &module(&steps),
        "_start",
        b"",
        &[],
        crate::irsim::Faults::default(),
        &mut model(&setup),
    );
    assert_eq!(results(&out.stdout, &steps), [FD, 5, 0, EBADF]);
    assert!(out.stdout.windows(5).any(|w| w == b"hello"));
}
