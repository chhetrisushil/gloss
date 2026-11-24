use std::collections::{HashMap, HashSet};

use super::encode::encode;
use super::inst::{Mem, R, X, text};
use super::lower::lower_function;
use crate::ir::Reg::{A, B};
use crate::ir::{Cond, Data, Function, Global, Inst, IntOp, Label, OpenMode, Prim, Shift};

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
        lines(&func(2, 2, vec![Inst::Enter(2)])),
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
fn calls_drop_their_arguments() {
    assert_eq!(
        one(Inst::Call("two".to_string())),
        ["call two", "add rsp, 16"]
    );
    assert_eq!(one(Inst::Call("none".to_string())), ["call none"]);
    let err = lower_function(&func(0, 0, vec![Inst::Call("x".into())]), &known());
    assert!(err.is_err());
}

#[test]
fn comparisons_jump_and_set() {
    let body = vec![Inst::Compare, Inst::JumpIf(Cond::Le, Label(3))];
    assert_eq!(lines(&func(0, 0, body)), ["cmp rax, rcx", "jle Lf_3"]);
    let body = vec![Inst::Mark(Label(3)), Inst::Compare, Inst::SetIf(Cond::Ne)];
    assert_eq!(
        lines(&func(0, 0, body)),
        ["Lf_3:", "cmp rax, rcx", "setne al", "and rax, 255"]
    );
    let body = vec![Inst::Jump(Label(1)), Inst::Line(7), Inst::Mark(Label(1))];
    assert_eq!(lines(&func(0, 0, body)), ["jmp Lf_1", "; line 7", "Lf_1:"]);
}

#[test]
fn every_condition_has_a_signed_code() {
    let all = [
        (Cond::Eq, "je"),
        (Cond::Ne, "jne"),
        (Cond::Lt, "jl"),
        (Cond::Le, "jle"),
        (Cond::Gt, "jg"),
        (Cond::Ge, "jge"),
    ];
    for (c, j) in all {
        let body = vec![
            Inst::Compare,
            Inst::JumpIf(c, Label(0)),
            Inst::Mark(Label(0)),
        ];
        assert_eq!(lines(&func(0, 0, body))[1], format!("{j} Lf_0"));
    }
}

#[test]
fn a_stack_check_compares_against_the_limit() {
    let body = vec![
        Inst::StackCheck {
            bytes: 64,
            fail: Label(9),
        },
        Inst::Mark(Label(9)),
    ];
    assert_eq!(
        lines(&func(0, 0, body)),
        [
            "lea rdx, [rsp-64]",
            "mov rsi, [rip+stack_limit]",
            "cmp rdx, rsi",
            "jb Lf_9",
            "Lf_9:"
        ]
    );
}

#[test]
fn integer_operations_and_shifts() {
    assert_eq!(one(Inst::Int(IntOp::Add)), ["add rax, rcx"]);
    assert_eq!(one(Inst::Int(IntOp::Sub)), ["sub rax, rcx"]);
    assert_eq!(one(Inst::Int(IntOp::And)), ["and rax, rcx"]);
    assert_eq!(one(Inst::Int(IntOp::Or)), ["or rax, rcx"]);
    assert_eq!(one(Inst::Int(IntOp::Mul)), ["imul rax, rcx"]);
    assert_eq!(one(Inst::Int(IntOp::Div)), ["cqo", "idiv rcx"]);
    assert_eq!(
        one(Inst::Int(IntOp::Rem)),
        ["cqo", "idiv rcx", "mov rax, rdx"]
    );
    assert_eq!(one(Inst::Shift(Shift::Left, 3)), ["shl rax, 3"]);
    assert_eq!(one(Inst::Shift(Shift::Right, 63)), ["sar rax, 63"]);
    assert_eq!(one(Inst::Shift(Shift::RightLogical, 1)), ["shr rax, 1"]);
    assert_eq!(one(Inst::AddImm(B, -8)), ["add rcx, -8"]);
}

#[test]
fn checked_numbers_jump_to_range_on_overflow() {
    let range = Label(1);
    assert_eq!(one(Inst::NumAdd { range }), ["add rax, rcx", "jo Lf_1"]);
    assert_eq!(one(Inst::NumSub { range }), ["sub rax, rcx", "jo Lf_1"]);
    assert_eq!(one(Inst::NumNeg { range }), ["neg rax", "jo Lf_1"]);
    assert_eq!(
        one(Inst::NumWhole),
        ["cqo", "movabs rsi, 10000", "idiv rsi", "imul rax, rsi"]
    );
}

#[test]
fn system_primitives_pop_their_arguments_in_reverse_and_keep_b() {
    let transfer = |n: &str| {
        let movs = [
            "pop rdx", "pop rsi", "pop rdi", n, "push rcx", "syscall", "pop rcx",
        ];
        movs.map(String::from).to_vec()
    };
    assert_eq!(one(Inst::System(Prim::Write)), transfer("movabs rax, 1"));
    assert_eq!(one(Inst::System(Prim::Read)), transfer("movabs rax, 0"));
    assert_eq!(
        one(Inst::System(Prim::Exit)),
        ["pop rdi", "movabs rax, 231", "syscall"]
    );
}

#[test]
fn open_builds_open_how_on_the_stack_and_keeps_b() {
    let how = |flags: i64, mode: i64| {
        [
            "pop rsi".to_string(),
            "pop rdi".to_string(),
            "movabs r11, 12".to_string(),
            "push r11".to_string(),
            format!("movabs r11, {mode}"),
            "push r11".to_string(),
            format!("movabs r11, {flags}"),
            "push r11".to_string(),
            "mov rdx, rsp".to_string(),
            "movabs r10, 24".to_string(),
            "movabs rax, 437".to_string(),
            "push rcx".to_string(),
            "syscall".to_string(),
            "pop rcx".to_string(),
            "add rsp, 24".to_string(),
        ]
    };
    // `Create` pops its permissions first and passes them on in `open_how.mode`.
    let create_how = |flags: i64| {
        let mut words = how(flags, 0).to_vec();
        words.insert(0, "pop r8".to_string());
        words.splice(5..7, ["push r8".to_string()]);
        words
    };
    let open = |mode| one(Inst::System(Prim::Open(mode)));
    assert_eq!(open(OpenMode::Read), how(0o2000000, 0)); // O_RDONLY | O_CLOEXEC
    // O_RDONLY | O_DIRECTORY | O_CLOEXEC: not O_PATH, so the folder can be fsynced (R8).
    assert_eq!(open(OpenMode::Folder), how(0o2200000, 0));
    // O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC
    assert_eq!(open(OpenMode::Create), create_how(0o2001101));
    assert_eq!(open(OpenMode::Probe), how(0o12000000, 0)); // O_PATH | O_CLOEXEC
}

#[test]
fn close_sync_and_rename_pop_their_arguments_in_reverse_and_keep_b() {
    let call = |number: &str| {
        ["movabs rax, N", "push rcx", "syscall", "pop rcx"].map(|s| s.replace("N", number))
    };
    let close = ["pop rdi".to_string()].into_iter().chain(call("3"));
    assert_eq!(one(Inst::System(Prim::Close)), close.collect::<Vec<_>>());
    let sync = ["pop rdi".to_string()].into_iter().chain(call("74"));
    assert_eq!(one(Inst::System(Prim::Sync)), sync.collect::<Vec<_>>());
    // fchmod(fd, permissions)
    let set_mode = ["pop rsi", "pop rdi"].map(String::from);
    let set_mode = set_mode.into_iter().chain(call("91"));
    assert_eq!(
        one(Inst::System(Prim::SetMode)),
        set_mode.collect::<Vec<_>>()
    );
    // renameat(folder, from, folder, to): rdx repeats the folder in rdi.
    let rename = ["pop r10", "pop rsi", "pop rdi", "mov rdx, rdi"].map(String::from);
    let rename = rename.into_iter().chain(call("264"));
    assert_eq!(one(Inst::System(Prim::Rename)), rename.collect::<Vec<_>>());
}

#[test]
fn mode_reads_the_low_permission_bits_of_st_mode_from_fstat() {
    let mode = one(Inst::System(Prim::Mode));
    assert_eq!(
        mode[..3],
        ["pop rdi", "sub rsp, 144", "mov rsi, rsp"].map(String::from)
    );
    assert!(mode.contains(&"movabs rax, 5".to_string()));
    assert!(mode.contains(&"cmp rax, -4096".to_string()));
    assert!(mode.contains(&"mov rax, [rsp+24]".to_string()), "{mode:?}");
    assert!(mode.contains(&"and rax, 511".to_string()));
    assert_eq!(mode.last().unwrap(), "add rsp, 144");
}

#[test]
fn map_turns_kernel_errors_into_zero_and_stack_size_reads_the_limit() {
    let map = one(Inst::System(Prim::Map));
    assert_eq!(map[0], "pop rsi");
    assert!(map.contains(&"movabs rax, 9".to_string()));
    assert!(map.contains(&"movabs r10, 34".to_string()));
    assert!(map.contains(&"cmp rax, -4096".to_string()));
    assert!(map.contains(&"xor rax, rax".to_string()));
    assert_eq!(
        one(Inst::System(Prim::StackSize)),
        [
            "push rcx",
            "sub rsp, 16",
            "movabs rdi, 3",
            "mov rsi, rsp",
            "movabs rax, 97",
            "syscall",
            "mov rax, [rsp+0]",
            "add rsp, 16",
            "pop rcx"
        ]
    );
}

#[test]
fn mul_div_and_rem_call_their_helpers_and_dispatch_on_the_status() {
    let (zero, range) = (Label(1), Label(2));
    assert_eq!(
        one(Inst::NumMul { range }),
        ["call x86_64.multiply_number", "test rdx, rdx", "jne Lf_2"]
    );
    assert_eq!(
        one(Inst::NumDiv { zero, range }),
        [
            "call x86_64.divide_number",
            "cmp rdx, 2",
            "je Lf_1",
            "test rdx, rdx",
            "jne Lf_2"
        ]
    );
    assert_eq!(
        one(Inst::NumRem { zero }),
        ["call x86_64.remainder_number", "test rdx, rdx", "jne Lf_1"]
    );
}

#[test]
fn only_the_mapped_registers_appear() {
    let allowed = [
        R::Rax,
        R::Rcx,
        R::Rdx,
        R::Rsi,
        R::Rdi,
        R::R8,
        R::R9,
        R::R10,
        R::R11,
        R::Rsp,
        R::Rbp,
    ];
    // `cqo` and `idiv` implicitly use rdx, which is an allowed scratch register.
    let mut body = vec![Inst::Line(1), Inst::Enter(2)];
    for r in [A, B] {
        body.extend([
            Inst::Const(r, 7),
            Inst::LoadSlot(r, 1),
            Inst::StoreSlot(2, r),
            Inst::LoadArg(r, 1),
            Inst::Push(r),
            Inst::Pop(r),
            Inst::AddImm(r, -3),
            Inst::StackPointer(r),
            Inst::AddressOf(r, Data::Text(0)),
            Inst::AddressOf(r, Data::Desc(0)),
            Inst::AddressOf(r, Data::Global(Global::OutBuf)),
        ]);
        for base in [A, B] {
            body.extend([
                Inst::Load {
                    to: r,
                    base,
                    offset: 8,
                },
                Inst::Store {
                    base,
                    offset: 8,
                    from: r,
                },
                Inst::LoadByte {
                    to: r,
                    base,
                    offset: 1,
                },
                Inst::StoreByte {
                    base,
                    offset: 1,
                    from: r,
                },
            ]);
        }
        body.push(Inst::Move {
            to: r,
            from: if r == A { B } else { A },
        });
    }
    for op in [
        IntOp::Add,
        IntOp::Sub,
        IntOp::Mul,
        IntOp::Div,
        IntOp::Rem,
        IntOp::And,
        IntOp::Or,
    ] {
        body.push(Inst::Int(op));
    }
    for s in [Shift::Left, Shift::Right, Shift::RightLogical] {
        body.push(Inst::Shift(s, 5));
    }
    body.extend([
        Inst::Compare,
        Inst::SetIf(Cond::Lt),
        Inst::Compare,
        Inst::JumpIf(Cond::Ge, Label(1)),
        Inst::StackCheck {
            bytes: 16,
            fail: Label(1),
        },
        Inst::Call("two".to_string()),
        Inst::Jump(Label(1)),
        Inst::Mark(Label(1)),
        Inst::Return,
    ]);
    let xs = lower_function(&func(2, 2, body), &known()).unwrap();
    for x in &xs {
        for r in registers(x) {
            assert!(allowed.contains(&r), "{x:?} uses {r:?}");
        }
    }
}

fn registers(x: &X) -> Vec<R> {
    let mem = |m: &Mem| match m {
        Mem::Base(r, _) => vec![*r],
        Mem::Rip(_) => vec![],
    };
    match x {
        X::MovRR(a, b) | X::Alu(_, a, b) | X::Imul(a, b) | X::Test(a, b) => vec![*a, *b],
        X::MovRI(r, _)
        | X::Push(r)
        | X::Pop(r)
        | X::AluI(_, r, _)
        | X::ImulWide(r)
        | X::Mul(r)
        | X::Idiv(r)
        | X::Div(r)
        | X::Neg(r)
        | X::Shl(r, _)
        | X::Sar(r, _)
        | X::Shr(r, _)
        | X::Setcc(_, r) => vec![*r],
        X::MovRM(r, m) | X::MovMR(m, r) | X::MovzxRM8(r, m) | X::MovM8R(m, r) | X::Lea(r, m) => {
            let mut v = mem(m);
            v.push(*r);
            v
        }
        X::Cqo
        | X::Jcc(..)
        | X::Jmp(_)
        | X::Call(_)
        | X::Ret
        | X::Syscall
        | X::Label(_)
        | X::Comment(_) => vec![],
    }
}

#[test]
fn malformed_frames_and_sizes_are_refused() {
    let err = |f: Function| lower_function(&f, &known()).unwrap_err();
    assert!(err(func(0, 2, vec![Inst::Enter(3)])).contains("enter 3"));
    for k in [0, 3] {
        assert!(err(func(0, 2, vec![Inst::LoadSlot(A, k)])).contains("slot"));
        assert!(err(func(0, 2, vec![Inst::StoreSlot(k, A)])).contains("slot"));
        assert!(err(func(2, 0, vec![Inst::LoadArg(A, k)])).contains("argument"));
    }
    let far = Inst::StackCheck {
        bytes: 1 << 31,
        fail: Label(0),
    };
    assert!(err(func(0, 0, vec![far])).contains("out of range"));
    let big = func(0, 1 << 28, vec![Inst::Enter(1 << 28)]);
    assert!(err(big).contains("out of range"));
    let call = Function {
        name: "f".into(),
        params: 0,
        slots: 0,
        body: vec![Inst::Call("huge".into())],
    };
    let huge = HashMap::from([("huge".to_string(), 1u32 << 29)]);
    assert!(
        lower_function(&call, &huge)
            .unwrap_err()
            .contains("out of range")
    );
}

#[test]
fn entering_with_no_slots_zeroes_nothing() {
    let f = func(0, 0, vec![Inst::Enter(0)]);
    assert_eq!(
        lines(&f),
        ["push rbp", "mov rbp, rsp", "sub rsp, 8", "mov [rbp-8], rcx"]
    );
}

#[test]
#[should_panic(expected = "arrive in task 3")]
fn fork_and_wait_are_not_lowered_yet() {
    one(Inst::System(Prim::Fork));
}
