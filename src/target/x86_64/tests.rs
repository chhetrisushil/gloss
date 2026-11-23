use super::inst::{Mem, R, X, text};

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
    assert_eq!(text(&X::MovRI(R::Rax, 42)), "mov rax, 42");
    assert_eq!(text(&X::MovRI(R::R15, -7)), "mov r15, -7");
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
