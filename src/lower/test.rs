//! Test bodies: each test is lowered to a function `t_<i>` of no inputs, in source order.
use super::Lowerer;
use crate::ast::program::{Program, TestDecl};
use crate::diag::Diagnostic;
use crate::ir::{Function, Inst, Module};
use crate::source::Source;

/// The name of the IR function for test `i`.
pub(super) fn test_name(i: usize) -> String {
    format!("t_{i}")
}

/// Lowers test `i`, finished as a feature is: its failure blocks, its frame and the routines
/// it calls.
pub(super) fn lower_test(
    module: &mut Module,
    context: (&Source, &Program, &crate::check::TypeInfo),
    i: usize,
    test: &TestDecl,
    called: &mut Vec<String>,
) -> Result<Function, Diagnostic> {
    let info = &context.2.tests[i];
    let mut l = Lowerer::new(module, context, (&info.vars, &info.loops), Vec::new());
    l.stmts(&test.body)?;
    l.emit(Inst::Return);
    let (slots, body) = l.finish(called);
    Ok(Function {
        name: test_name(i),
        params: 0,
        slots,
        body,
    })
}
