//! Cases for memory access, `AddImm`, `SetIf` and `StackCheck`, ported from the
//! reviewer's 8a harness.

use super::calls::l;
use crate::ir::Reg::{A, B};
use crate::ir::{Cond, Data, Function, Global, Inst, IntOp};
use Inst::*;

fn f(name: &str, body: Vec<Inst>) -> Function {
    Function {
        name: name.to_string(),
        params: 0,
        slots: 0,
        body,
    }
}

/// Sets `stack_limit` to `sp + delta` (or an absolute value), then `StackCheck bytes`: the
/// result is 2 if the check is taken, else 1.
fn stack(name: &str, delta: Option<i32>, abs: i64, bytes: u32) -> Function {
    let mut b = vec![Enter(0)];
    match delta {
        Some(d) => b.extend([StackPointer(A), AddImm(A, d)]),
        None => b.push(Const(A, abs)),
    }
    b.extend([
        AddressOf(B, Data::Global(Global::StackLimit)),
        Store {
            base: B,
            offset: 0,
            from: A,
        },
        StackCheck { bytes, fail: l(1) },
        Const(A, 1),
        Return,
        Mark(l(1)),
        Const(A, 2),
        Return,
    ]);
    f(name, b)
}

pub fn cases() -> Vec<Function> {
    let mut c = vec![
        f(
            "mem_word",
            vec![
                Enter(0),
                AddressOf(B, Data::Global(Global::OutBuf)),
                Const(A, -2),
                Store {
                    base: B,
                    offset: 8,
                    from: A,
                },
                Const(A, 0x1234),
                StoreByte {
                    base: B,
                    offset: 8,
                    from: A,
                },
                Load {
                    to: A,
                    base: B,
                    offset: 8,
                },
                Return,
            ],
        ),
        f(
            "mem_byte",
            vec![
                Enter(0),
                AddressOf(B, Data::Global(Global::OutBuf)),
                Const(A, -2),
                Store {
                    base: B,
                    offset: 8,
                    from: A,
                },
                LoadByte {
                    to: A,
                    base: B,
                    offset: 9,
                },
                Return,
            ],
        ),
        f(
            "mem_base_a",
            vec![
                Enter(0),
                AddressOf(A, Data::Global(Global::InLen)),
                Const(B, i64::MIN + 5),
                Store {
                    base: A,
                    offset: -8,
                    from: B,
                },
                Const(B, 0),
                LoadByte {
                    to: B,
                    base: A,
                    offset: -1,
                },
                Load {
                    to: A,
                    base: A,
                    offset: -8,
                },
                Int(IntOp::Add),
                Return,
            ],
        ),
        f(
            "mem_heap",
            vec![
                Enter(0),
                AddressOf(A, Data::Global(Global::HeapNext)),
                Const(B, 0x7766554433221100),
                Store {
                    base: A,
                    offset: 0,
                    from: B,
                },
                Const(B, 0xAB),
                StoreByte {
                    base: A,
                    offset: 7,
                    from: B,
                },
                Load {
                    to: A,
                    base: A,
                    offset: 0,
                },
                Return,
            ],
        ),
        f(
            "stack_ptr",
            vec![
                Enter(0),
                StackPointer(A),
                Push(A),
                StackPointer(B),
                Pop(A),
                Int(IntOp::Sub),
                Return,
            ],
        ),
    ];
    c.extend([
        f(
            "addimm",
            vec![
                Enter(0),
                Const(A, 10),
                AddImm(A, i32::MIN),
                Const(B, -3),
                AddImm(B, i32::MAX),
                Int(IntOp::Sub),
                Return,
            ],
        ),
        f(
            "setif_upper",
            vec![
                Enter(0),
                Const(A, -5),
                Const(B, -5),
                Compare,
                SetIf(Cond::Eq),
                Return,
            ],
        ),
    ]);
    c.extend([
        stack("sc_below4096_nottaken", Some(-4096), 0, 64),
        stack("sc_limit_eq_sp_taken", Some(0), 0, 8),
        stack("sc_exact_nottaken", Some(-64), 0, 64),
        stack("sc_exact_plus1_taken", Some(-64), 0, 65),
        stack("sc_neg1_unsigned_taken", None, -1, 0),
        stack("sc_zero_nottaken", None, 0, 1 << 20),
        stack("sc_above_bytes0_taken", Some(8), 0, 0),
        stack("sc_minint_taken", None, i64::MIN, 0),
    ]);
    c
}
