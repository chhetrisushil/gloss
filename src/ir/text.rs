//! The text form of the IR, which lowering tests compare against.

use super::{Cond, Data, Function, Inst, IntOp, Label, Module, OpenMode, Prim, Reg, Shift};

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
        Data::Global(g) => g.name().to_string(),
    }
}

/// The word that follows `system open`.
fn open_mode(mode: &OpenMode) -> &'static str {
    match mode {
        OpenMode::Read => "read",
        OpenMode::Folder => "folder",
        OpenMode::Create => "create",
        OpenMode::Probe => "probe",
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

pub fn inst_text(inst: &Inst) -> String {
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
        Inst::System(Prim::Open(mode)) => format!("  system open {}", open_mode(mode)),
        Inst::System(Prim::Close) => "  system close".to_string(),
        Inst::System(Prim::Sync) => "  system sync".to_string(),
        Inst::System(Prim::Mode) => "  system mode".to_string(),
        Inst::System(Prim::Rename) => "  system rename".to_string(),
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
            Inst::System(Prim::Open(OpenMode::Read)),
            Inst::System(Prim::Open(OpenMode::Folder)),
            Inst::System(Prim::Open(OpenMode::Create)),
            Inst::System(Prim::Open(OpenMode::Probe)),
            Inst::System(Prim::Close),
            Inst::System(Prim::Sync),
            Inst::System(Prim::Mode),
            Inst::System(Prim::Rename),
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
  system open read
  system open folder
  system open create
  system open probe
  system close
  system sync
  system mode
  system rename
";

    #[test]
    fn a_function_prints_one_line_per_instruction() {
        assert_eq!(function_text(&sample()), SAMPLE);
    }

    fn every_instruction() -> Function {
        let l3 = Label(3);
        let l4 = Label(4);
        let body = vec![
            Inst::Line(4),
            Inst::Const(A, 42),
            Inst::LoadSlot(A, 2),
            Inst::StoreSlot(2, A),
            Inst::LoadArg(B, 1),
            Inst::Move { to: B, from: A },
            Inst::Push(A),
            Inst::Pop(B),
            Inst::AddressOf(A, Data::Text(3)),
            Inst::AddressOf(A, Data::Desc(2)),
            Inst::AddressOf(A, Data::Global(Global::OutBuf)),
            Inst::AddressOf(A, Data::Global(Global::HeapNext)),
            Inst::AddressOf(A, Data::Global(Global::HeapEnd)),
            Inst::AddressOf(A, Data::Global(Global::StackLimit)),
            Inst::AddressOf(A, Data::Global(Global::OutLen)),
            Inst::AddressOf(A, Data::Global(Global::InLen)),
            Inst::AddressOf(A, Data::Global(Global::InPos)),
            Inst::AddressOf(A, Data::Global(Global::InBuf)),
            Inst::AddressOf(A, Data::Global(Global::NumBuf)),
            Inst::StackPointer(A),
            Inst::Load {
                to: A,
                base: B,
                offset: 8,
            },
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
            Inst::Store {
                base: B,
                offset: 8,
                from: A,
            },
            Inst::LoadByte {
                to: A,
                base: B,
                offset: 8,
            },
            Inst::StoreByte {
                base: B,
                offset: 8,
                from: A,
            },
            Inst::NumAdd { range: l3 },
            Inst::NumSub { range: l3 },
            Inst::NumMul { range: l3 },
            Inst::NumNeg { range: l3 },
            Inst::NumDiv {
                zero: l4,
                range: l3,
            },
            Inst::NumRem { zero: l4 },
            Inst::NumWhole,
            Inst::Int(IntOp::Add),
            Inst::Int(IntOp::Sub),
            Inst::Int(IntOp::Mul),
            Inst::Int(IntOp::Div),
            Inst::Int(IntOp::Rem),
            Inst::Int(IntOp::And),
            Inst::Int(IntOp::Or),
            Inst::AddImm(A, -8),
            Inst::AddImm(B, 8),
            Inst::Shift(Shift::Left, 3),
            Inst::Shift(Shift::Right, 3),
            Inst::Shift(Shift::RightLogical, 3),
            Inst::Compare,
            Inst::JumpIf(Cond::Eq, l3),
            Inst::JumpIf(Cond::Ne, l3),
            Inst::JumpIf(Cond::Lt, l3),
            Inst::JumpIf(Cond::Le, l3),
            Inst::JumpIf(Cond::Gt, l3),
            Inst::JumpIf(Cond::Ge, l3),
            Inst::SetIf(Cond::Ge),
            Inst::Jump(l3),
            Inst::Mark(l3),
            Inst::Call("show_number".to_string()),
            Inst::Return,
            Inst::Enter(3),
            Inst::StackCheck {
                bytes: 48,
                fail: Label(5),
            },
            Inst::System(Prim::Write),
            Inst::System(Prim::Read),
            Inst::System(Prim::Map),
            Inst::System(Prim::Exit),
            Inst::System(Prim::StackSize),
            Inst::System(Prim::Open(OpenMode::Read)),
            Inst::System(Prim::Open(OpenMode::Folder)),
            Inst::System(Prim::Open(OpenMode::Create)),
            Inst::System(Prim::Open(OpenMode::Probe)),
            Inst::System(Prim::Close),
            Inst::System(Prim::Sync),
            Inst::System(Prim::Mode),
            Inst::System(Prim::Rename),
        ];
        Function {
            name: "f".to_string(),
            params: 2,
            slots: 3,
            body,
        }
    }

    const EXPECTED: &str = "\
function f, 2 params, 3 slots
line 4
  a = 42
  a = slot 2
  slot 2 = a
  b = arg 1
  b = a
  push a
  pop b
  a = address of text 3
  a = address of desc 2
  a = address of out_buf
  a = address of heap_next
  a = address of heap_end
  a = address of stack_limit
  a = address of out_len
  a = address of in_len
  a = address of in_pos
  a = address of in_buf
  a = address of num_buf
  a = stack pointer
  a = [b + 8]
  a = [b]
  a = [b - 8]
  [b + 8] = a
  a = byte [b + 8]
  byte [b + 8] = a
  a = num_add a, b; out of range L3
  a = num_sub a, b; out of range L3
  a = num_mul a, b; out of range L3
  a = num_neg a; out of range L3
  a = num_div a, b; zero L4; out of range L3
  a = num_rem a, b; zero L4
  a = num_whole a
  a = a + b
  a = a - b
  a = a * b
  a = a div b
  a = a mod b
  a = a and b
  a = a or b
  a = a - 8
  b = b + 8
  a = a << 3
  a = a >> 3
  a = a >>> 3
  compare a, b
  jump if eq L3
  jump if ne L3
  jump if lt L3
  jump if le L3
  jump if gt L3
  jump if ge L3
  a = 1 if ge
  jump L3
L3:
  call show_number
  return
  enter 3
  stack check 48; too deep L5
  system write
  system read
  system map
  system exit
  system stack_size
  system open read
  system open folder
  system open create
  system open probe
  system close
  system sync
  system mode
  system rename
";

    #[test]
    fn a_function_with_every_instruction_prints_exactly() {
        assert_eq!(function_text(&every_instruction()), EXPECTED);
    }

    #[test]
    fn a_module_separates_functions_with_a_blank_line() {
        let mut g = sample();
        g.name = "g".to_string();
        let functions = vec![sample(), g];
        let want = format!("{SAMPLE}\n{}", SAMPLE.replace("function f", "function g"));
        assert_eq!(
            module_text(&Module {
                functions,
                ..Module::default()
            }),
            want
        );
    }
}
