use super::inst::{AluOp, Cc, Mem, R, Sym, X, text};
use crate::ir::{Data, Function, Global, Inst, Reg, function_text};

fn local(d: i32) -> Mem {
    Mem::Base(R::Rbp, d)
}

fn field() -> Mem {
    Mem::Base(R::Rcx, 8)
}

#[test]
fn moves_print_in_intel_syntax() {
    assert_eq!(text(&X::MovRR(R::Rax, R::R9)), "mov rax, r9");
    assert_eq!(text(&X::MovRM(R::Rax, local(-16))), "mov rax, [rbp-16]");
    assert_eq!(text(&X::MovMR(local(-16), R::Rax)), "mov [rbp-16], rax");
    assert_eq!(text(&X::MovRI(R::Rax, 42)), "movabs rax, 42");
    assert_eq!(text(&X::MovRI(R::R15, -7)), "movabs r15, -7");
    assert_eq!(
        text(&X::MovzxRM8(R::Rax, field())),
        "movzx eax, byte [rcx+8]"
    );
    assert_eq!(
        text(&X::MovzxRM8(R::R10, field())),
        "movzx r10d, byte [rcx+8]"
    );
    assert_eq!(text(&X::MovM8R(field(), R::Rax)), "mov byte [rcx+8], al");
    assert_eq!(text(&X::MovM8R(field(), R::Rsi)), "mov byte [rcx+8], sil");
    assert_eq!(text(&X::MovM8R(field(), R::R12)), "mov byte [rcx+8], r12b");
}

#[test]
fn control_flow_and_markers_print() {
    assert_eq!(text(&X::Jmp("L3".to_string())), "jmp L3");
    assert_eq!(
        text(&X::Call("show_number".to_string())),
        "call show_number"
    );
    assert_eq!(text(&X::Ret), "ret");
    assert_eq!(text(&X::Syscall), "syscall");
    assert_eq!(text(&X::Label("L3".to_string())), "L3:");
    assert_eq!(text(&X::Comment("line 4".to_string())), "; line 4");
}

#[test]
fn memory_operands_and_symbols_print() {
    assert_eq!(
        text(&X::Lea(R::Rax, Mem::Rip(Sym::Text(3)))),
        "lea rax, [rip+text_3]"
    );
    assert_eq!(
        text(&X::Lea(R::Rax, Mem::Rip(Sym::Desc(2)))),
        "lea rax, [rip+desc_2]"
    );
    let out = Mem::Rip(Sym::Global(Global::OutBuf));
    assert_eq!(text(&X::Lea(R::Rdi, out)), "lea rdi, [rip+out_buf]");
    let l = Mem::Rip(Sym::Label("L1".to_string()));
    assert_eq!(text(&X::MovRM(R::Rax, l)), "mov rax, [rip+L1]");
    assert_eq!(
        text(&X::MovRM(R::Rax, Mem::Base(R::Rsp, 0))),
        "mov rax, [rsp+0]"
    );
    assert_eq!(
        text(&X::MovRM(R::Rax, Mem::Base(R::R13, 24))),
        "mov rax, [r13+24]"
    );
    let low = Mem::Base(R::Rbp, i32::MIN);
    assert_eq!(text(&X::MovRM(R::Rax, low)), "mov rax, [rbp-2147483648]");
}

#[test]
fn every_global_prints_its_name() {
    let names = [
        (Global::HeapNext, "heap_next"),
        (Global::HeapEnd, "heap_end"),
        (Global::StackLimit, "stack_limit"),
        (Global::OutLen, "out_len"),
        (Global::OutBuf, "out_buf"),
        (Global::InLen, "in_len"),
        (Global::InPos, "in_pos"),
        (Global::InBuf, "in_buf"),
        (Global::NumBuf, "num_buf"),
        (Global::EntrySp, "entry_sp"),
        (Global::Tables, "tables"),
    ];
    for (g, name) in names {
        let x = X::Lea(R::Rax, Mem::Rip(Sym::Global(g)));
        assert_eq!(text(&x), format!("lea rax, [rip+{name}]"));
        let f = Function {
            name: "f".to_string(),
            params: 0,
            slots: 0,
            body: vec![Inst::AddressOf(Reg::A, Data::Global(g))],
        };
        assert_eq!(
            function_text(&f).lines().nth(1),
            Some(&*format!("  a = address of {name}"))
        );
    }
}

#[test]
fn arithmetic_prints() {
    assert_eq!(text(&X::Alu(AluOp::Add, R::Rax, R::Rcx)), "add rax, rcx");
    assert_eq!(text(&X::Alu(AluOp::Cmp, R::Rax, R::Rcx)), "cmp rax, rcx");
    assert_eq!(text(&X::AluI(AluOp::Sub, R::Rsp, 24)), "sub rsp, 24");
    assert_eq!(text(&X::AluI(AluOp::And, R::Rax, -8)), "and rax, -8");
    assert_eq!(text(&X::Alu(AluOp::Or, R::Rax, R::Rdx)), "or rax, rdx");
    assert_eq!(text(&X::Alu(AluOp::Xor, R::Rdx, R::Rdx)), "xor rdx, rdx");
    assert_eq!(text(&X::Imul(R::Rax, R::Rcx)), "imul rax, rcx");
    assert_eq!(text(&X::ImulWide(R::Rcx)), "imul rcx");
    assert_eq!(text(&X::Mul(R::Rcx)), "mul rcx");
    assert_eq!(text(&X::Idiv(R::Rcx)), "idiv rcx");
    assert_eq!(text(&X::Div(R::Rcx)), "div rcx");
    assert_eq!(text(&X::Cqo), "cqo");
    assert_eq!(text(&X::Neg(R::Rax)), "neg rax");
    assert_eq!(text(&X::Test(R::Rax, R::Rax)), "test rax, rax");
    assert_eq!(text(&X::Test(R::Rax, R::Rcx)), "test rax, rcx");
    assert_eq!(text(&X::Shl(R::Rax, 3)), "shl rax, 3");
    assert_eq!(text(&X::Sar(R::Rax, 63)), "sar rax, 63");
    assert_eq!(text(&X::Shr(R::Rax, 1)), "shr rax, 1");
    assert_eq!(text(&X::Push(R::Rbp)), "push rbp");
    assert_eq!(text(&X::Pop(R::R14)), "pop r14");
}

#[test]
fn every_register_prints_in_each_width() {
    let regs = [
        (R::Rax, "rax", "eax", "al"),
        (R::Rcx, "rcx", "ecx", "cl"),
        (R::Rdx, "rdx", "edx", "dl"),
        (R::Rbx, "rbx", "ebx", "bl"),
        (R::Rsp, "rsp", "esp", "spl"),
        (R::Rbp, "rbp", "ebp", "bpl"),
        (R::Rsi, "rsi", "esi", "sil"),
        (R::Rdi, "rdi", "edi", "dil"),
        (R::R8, "r8", "r8d", "r8b"),
        (R::R9, "r9", "r9d", "r9b"),
        (R::R10, "r10", "r10d", "r10b"),
        (R::R11, "r11", "r11d", "r11b"),
        (R::R12, "r12", "r12d", "r12b"),
        (R::R13, "r13", "r13d", "r13b"),
        (R::R14, "r14", "r14d", "r14b"),
        (R::R15, "r15", "r15d", "r15b"),
    ];
    for (r, wide, dword, byte) in regs {
        assert_eq!(text(&X::Push(r)), format!("push {wide}"));
        assert_eq!(
            text(&X::MovzxRM8(r, field())),
            format!("movzx {dword}, byte [rcx+8]")
        );
        assert_eq!(text(&X::Setcc(Cc::E, r)), format!("sete {byte}"));
    }
}

#[test]
fn conditions_print() {
    let conds = [
        (Cc::E, "e"),
        (Cc::Ne, "ne"),
        (Cc::L, "l"),
        (Cc::Le, "le"),
        (Cc::G, "g"),
        (Cc::Ge, "ge"),
        (Cc::B, "b"),
        (Cc::Ae, "ae"),
        (Cc::O, "o"),
    ];
    for (c, name) in conds {
        assert_eq!(text(&X::Setcc(c, R::Rax)), format!("set{name} al"));
        assert_eq!(text(&X::Jcc(c, "L3".to_string())), format!("j{name} L3"));
    }
}
