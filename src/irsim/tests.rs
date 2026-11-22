use super::call;
use crate::ir::{Function, Inst, Module, Reg};
use Inst::*;
use Reg::A;

fn function(name: &str, params: u32, slots: u32, body: Vec<Inst>) -> Function {
    let mut all = vec![Enter(slots)];
    all.extend(body);
    Function {
        name: name.to_string(),
        params,
        slots,
        body: all,
    }
}

fn single(body: Vec<Inst>) -> Module {
    Module {
        functions: vec![function("f", 0, 0, body)],
        ..Module::default()
    }
}

#[test]
fn a_function_returns_a() {
    let m = single(vec![Line(1), Const(A, 42), Return]);
    assert_eq!(call(&m, "f", &[]), 42);
}
