use std::collections::{HashMap, HashSet};

use super::encode::encode;
use super::inst::{X, text};
use super::lower::lower_function;
use crate::ir::Reg::{A, B};
use crate::ir::{Data, Function, Global, Inst, Prim};

fn func(params: u32, slots: u32, body: Vec<Inst>) -> Function {
    Function {
        name: "f".to_string(),
        params,
        slots,
        body,
    }
}

fn known() -> HashMap<String, u32> {
    HashMap::from([("two".to_string(), 2), ("none".to_string(), 0)])
}

/// Lowers `body` (after the label), checks it encodes, and returns its text.
fn lines(f: &Function) -> Vec<String> {
    let xs = lower_function(f, &known()).unwrap();
    let mut all = xs.clone();
    let mut wanted = HashSet::new();
    for x in &xs {
        if let X::Jcc(_, l) | X::Jmp(l) | X::Call(l) = x {
            wanted.insert(l.clone());
        }
    }
    for x in &xs {
        if let X::Label(l) = x {
            wanted.remove(l);
        }
    }
    for l in wanted {
        all.push(X::Label(l));
    }
    encode(&all).unwrap();
    xs.iter().skip(1).map(text).collect()
}

fn one(inst: Inst) -> Vec<String> {
    lines(&func(2, 3, vec![inst]))
}

#[test]
fn the_output_starts_with_the_function_label() {
    let xs = lower_function(&func(0, 0, vec![]), &known()).unwrap();
    assert_eq!(xs, vec![X::Label("f".to_string())]);
}

#[test]
fn enter_builds_the_frame_and_zeroes_the_slots() {
    assert_eq!(
        one(Inst::Enter(2)),
        [
            "push rbp",
            "mov rbp, rsp",
            "sub rsp, 24",
            "mov [rbp-24], rcx",
            "xor rdx, rdx",
            "mov [rbp-8], rdx",
            "mov [rbp-16], rdx",
        ]
    );
}

#[test]
fn return_restores_b_and_the_frame() {
    let f = func(0, 2, vec![Inst::Return]);
    assert_eq!(
        lines(&f),
        ["mov rcx, [rbp-24]", "mov rsp, rbp", "pop rbp", "ret"]
    );
}

#[test]
fn slots_and_arguments_have_frame_addresses() {
    assert_eq!(one(Inst::LoadSlot(A, 2)), ["mov rax, [rbp-16]"]);
    assert_eq!(one(Inst::StoreSlot(1, B)), ["mov [rbp-8], rcx"]);
    assert_eq!(one(Inst::LoadArg(B, 1)), ["mov rcx, [rbp+24]"]);
    assert_eq!(one(Inst::LoadArg(A, 2)), ["mov rax, [rbp+16]"]);
}

#[test]
fn moves() {
    assert_eq!(one(Inst::Const(B, -5)), ["movabs rcx, -5"]);
    assert_eq!(one(Inst::Move { to: A, from: B }), ["mov rax, rcx"]);
    assert_eq!(one(Inst::Push(A)), ["push rax"]);
    assert_eq!(one(Inst::Pop(B)), ["pop rcx"]);
}

#[test]
fn memory() {
    assert_eq!(one(Inst::StackPointer(A)), ["mov rax, rsp"]);
    assert_eq!(
        one(Inst::AddressOf(A, Data::Text(2))),
        ["lea rax, [rip+text_2]"]
    );
    assert_eq!(
        one(Inst::AddressOf(B, Data::Desc(1))),
        ["lea rcx, [rip+desc_1]"]
    );
    assert_eq!(
        one(Inst::AddressOf(A, Data::Global(Global::HeapNext))),
        ["lea rax, [rip+heap_next]"]
    );
    let load = Inst::Load {
        to: A,
        base: B,
        offset: 8,
    };
    assert_eq!(one(load), ["mov rax, [rcx+8]"]);
    let store = Inst::Store {
        base: A,
        offset: -8,
        from: B,
    };
    assert_eq!(one(store), ["mov [rax-8], rcx"]);
    let lb = Inst::LoadByte {
        to: B,
        base: A,
        offset: 1,
    };
    assert_eq!(one(lb), ["movzx ecx, byte [rax+1]"]);
    let sb = Inst::StoreByte {
        base: B,
        offset: 0,
        from: A,
    };
    assert_eq!(one(sb), ["mov byte [rcx+0], al"]);
}

#[test]
fn numeric_and_system_instructions_wait_for_8b() {
    let err = lower_function(&func(0, 0, vec![Inst::NumWhole]), &known()).unwrap_err();
    assert_eq!(err, "cannot lower a = num_whole a yet");
    let err = lower_function(&func(0, 0, vec![Inst::System(Prim::Exit)]), &known());
    assert_eq!(err.unwrap_err(), "cannot lower system exit yet");
}
