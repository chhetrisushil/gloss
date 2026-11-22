//! The text form of the IR, which lowering tests compare against.

use super::{Cond, Data, Function, Global, Inst, IntOp, Label, Module, Prim, Reg, Shift};

/// Prints a function: a header line, then one line per instruction.
pub fn function_text(f: &Function) -> String {
    let mut out = format!(
        "function {}, {} params, {} slots\n",
        f.name, f.params, f.slots
    );
    for inst in &f.body {
        out.push_str(&inst_text(inst));
        out.push('\n');
    }
    out
}

/// Prints every function in order, separated by a blank line.
pub fn module_text(m: &Module) -> String {
    let functions: Vec<String> = m.functions.iter().map(function_text).collect();
    functions.join("\n")
}

fn reg(r: &Reg) -> &'static str {
    match r {
        Reg::A => "a",
        Reg::B => "b",
    }
}

/// `[base]`, `[base + 8]` or `[base - 8]`.
fn at(base: &Reg, offset: &i32) -> String {
    let base = reg(base);
    match *offset {
        0 => format!("[{base}]"),
        o if o > 0 => format!("[{base} + {o}]"),
        o => format!("[{base} - {}]", -i64::from(o)),
    }
}

fn data(d: &Data) -> String {
    match d {
        Data::Text(n) => format!("text {n}"),
        Data::Desc(n) => format!("desc {n}"),
        Data::Global(Global::HeapNext) => "heap_next".to_string(),
        Data::Global(Global::HeapEnd) => "heap_end".to_string(),
        Data::Global(Global::StackLimit) => "stack_limit".to_string(),
        Data::Global(Global::OutLen) => "out_len".to_string(),
        Data::Global(Global::OutBuf) => "out_buf".to_string(),
        Data::Global(Global::InLen) => "in_len".to_string(),
        Data::Global(Global::InPos) => "in_pos".to_string(),
        Data::Global(Global::InBuf) => "in_buf".to_string(),
    }
}

fn cond(c: &Cond) -> &'static str {
    match c {
        Cond::Eq => "eq",
        Cond::Ne => "ne",
        Cond::Lt => "lt",
        Cond::Le => "le",
        Cond::Gt => "gt",
        Cond::Ge => "ge",
    }
}

fn inst_text(inst: &Inst) -> String {
    match inst {
        Inst::Line(n) => format!("line {n}"),
        Inst::Const(r, n) => format!("  {} = {n}", reg(r)),
        Inst::LoadSlot(r, n) => format!("  {} = slot {n}", reg(r)),
        Inst::StoreSlot(n, r) => format!("  slot {n} = {}", reg(r)),
        Inst::LoadArg(r, n) => format!("  {} = arg {n}", reg(r)),
        Inst::Move { to, from } => format!("  {} = {}", reg(to), reg(from)),
        Inst::Push(r) => format!("  push {}", reg(r)),
        Inst::Pop(r) => format!("  pop {}", reg(r)),
        Inst::AddressOf(r, d) => format!("  {} = address of {}", reg(r), data(d)),
        Inst::StackPointer(r) => format!("  {} = stack pointer", reg(r)),
        Inst::Load { to, base, offset } => format!("  {} = {}", reg(to), at(base, offset)),
        Inst::Store { base, offset, from } => format!("  {} = {}", at(base, offset), reg(from)),
        Inst::LoadByte { to, base, offset } => format!("  {} = byte {}", reg(to), at(base, offset)),
        Inst::StoreByte { base, offset, from } => {
            format!("  byte {} = {}", at(base, offset), reg(from))
        }
        Inst::NumAdd { range: Label(r) } => format!("  a = num_add a, b; out of range L{r}"),
        Inst::NumSub { range: Label(r) } => format!("  a = num_sub a, b; out of range L{r}"),
        Inst::NumMul { range: Label(r) } => format!("  a = num_mul a, b; out of range L{r}"),
        Inst::NumNeg { range: Label(r) } => format!("  a = num_neg a; out of range L{r}"),
        Inst::NumDiv { zero, range } => {
            format!(
                "  a = num_div a, b; zero L{}; out of range L{}",
                zero.0, range.0
            )
        }
        Inst::NumRem { zero: Label(z) } => format!("  a = num_rem a, b; zero L{z}"),
        Inst::NumWhole => "  a = num_whole a".to_string(),
        Inst::Int(IntOp::Add) => "  a = a + b".to_string(),
        Inst::Int(IntOp::Sub) => "  a = a - b".to_string(),
        Inst::Int(IntOp::Mul) => "  a = a * b".to_string(),
        Inst::Int(IntOp::Div) => "  a = a div b".to_string(),
        Inst::Int(IntOp::Rem) => "  a = a mod b".to_string(),
        Inst::Int(IntOp::And) => "  a = a and b".to_string(),
        Inst::Int(IntOp::Or) => "  a = a or b".to_string(),
        Inst::AddImm(r, n) => {
            let sign = if *n < 0 { '-' } else { '+' };
            format!("  {0} = {0} {sign} {1}", reg(r), i64::from(*n).abs())
        }
        Inst::Shift(Shift::Left, n) => format!("  a = a << {n}"),
        Inst::Shift(Shift::Right, n) => format!("  a = a >> {n}"),
        Inst::Shift(Shift::RightLogical, n) => format!("  a = a >>> {n}"),
        Inst::Compare => "  compare a, b".to_string(),
        Inst::JumpIf(c, Label(l)) => format!("  jump if {} L{l}", cond(c)),
        Inst::SetIf(c) => format!("  a = 1 if {}", cond(c)),
        Inst::Jump(Label(l)) => format!("  jump L{l}"),
        Inst::Mark(Label(l)) => format!("L{l}:"),
        Inst::Call(name) => format!("  call {name}"),
        Inst::Return => "  return".to_string(),
        Inst::Enter(n) => format!("  enter {n}"),
        Inst::StackCheck { bytes, fail } => format!("  stack check {bytes}; too deep L{}", fail.0),
        Inst::System(Prim::Write) => "  system write".to_string(),
        Inst::System(Prim::Read) => "  system read".to_string(),
        Inst::System(Prim::Map) => "  system map".to_string(),
        Inst::System(Prim::Exit) => "  system exit".to_string(),
        Inst::System(Prim::StackSize) => "  system stack_size".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::Reg::{A, B};
    use super::super::*;
    use super::*;

    fn sample() -> Function {
        let body = vec![
            Inst::Line(4),
            Inst::Const(A, 42),
            Inst::LoadSlot(A, 2),
            Inst::Load {
                to: A,
                base: B,
                offset: 0,
            },
            Inst::Load {
                to: A,
                base: B,
                offset: -8,
            },
            Inst::AddressOf(A, Data::Global(Global::OutBuf)),
            Inst::NumDiv {
                zero: Label(4),
                range: Label(3),
            },
            Inst::Int(IntOp::Div),
            Inst::AddImm(A, -8),
            Inst::JumpIf(Cond::Lt, Label(3)),
            Inst::Mark(Label(3)),
            Inst::StackCheck {
                bytes: 48,
                fail: Label(5),
            },
            Inst::System(Prim::StackSize),
        ];
        Function {
            name: "f".to_string(),
            params: 2,
            slots: 3,
            body,
        }
    }

    const SAMPLE: &str = "\
function f, 2 params, 3 slots
line 4
  a = 42
  a = slot 2
  a = [b]
  a = [b - 8]
  a = address of out_buf
  a = num_div a, b; zero L4; out of range L3
  a = a div b
  a = a - 8
  jump if lt L3
L3:
  stack check 48; too deep L5
  system stack_size
";

    #[test]
    fn a_function_prints_one_line_per_instruction() {
        assert_eq!(function_text(&sample()), SAMPLE);
    }
}
